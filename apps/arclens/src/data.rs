//! Loading the catalog: cache first, network refresh when stale.

use crate::paths::Paths;
use anyhow::Context as _;
use arclens_data::download::download_raidtheory;
use arclens_data::raidtheory::RaidTheoryDir;
use arclens_data::{Catalog, DiskCache};
use std::sync::Arc;
use std::time::Duration;

/// Upstream data changes with game patches, not hourly.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Returns the cached catalog if fresh; otherwise downloads the dataset,
/// rebuilds and caches it. A stale cache is still returned if the refresh
/// fails, so the app keeps working offline.
pub async fn load(paths: Paths) -> Result<Arc<Catalog>, String> {
    load_inner(paths)
        .await
        .map(Arc::new)
        .map_err(|e| format!("{e:#}"))
}

/// Set to a local checkout of RaidTheory/arcraiders-data to skip the cache
/// and network entirely (offline development, testing dataset changes).
pub const DATA_DIR_ENV: &str = "ARCLENS_RAIDTHEORY_DIR";

async fn load_inner(paths: Paths) -> anyhow::Result<Catalog> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        tracing::info!(dir = %std::path::Path::new(&dir).display(), "loading dataset from {DATA_DIR_ENV}");
        return Ok(tokio::task::spawn_blocking(move || RaidTheoryDir::new(dir).load()).await??);
    }

    let cache = DiskCache::new(paths.catalog_cache());
    let cached = {
        let cache = cache.clone();
        tokio::task::spawn_blocking(move || cache.load())
            .await?
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "ignoring unreadable catalog cache");
                None
            })
    };

    if let Some(catalog) = &cached
        && !catalog.is_stale(MAX_AGE)
    {
        tracing::info!(items = catalog.items.len(), "using cached catalog");
        return Ok(cached.unwrap_or_else(|| unreachable!()));
    }

    match refresh(&paths, &cache).await {
        Ok(catalog) => Ok(catalog),
        Err(error) => match cached {
            Some(stale) => {
                tracing::warn!(
                    error = format!("{error:#}"),
                    "refresh failed; using stale cache"
                );
                Ok(stale)
            }
            None => Err(error),
        },
    }
}

/// HTTP client for all upstream requests (identifies us to data providers).
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("ARClens/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

async fn refresh(paths: &Paths, cache: &DiskCache) -> anyhow::Result<Catalog> {
    let client = http_client();
    let dir = paths.raidtheory_dir();
    download_raidtheory(&client, &dir)
        .await
        .context("downloading RaidTheory dataset")?;

    let cache = cache.clone();
    tokio::task::spawn_blocking(move || {
        let catalog = RaidTheoryDir::new(dir).load()?;
        cache.store(&catalog)?;
        tracing::info!(items = catalog.items.len(), "catalog refreshed");
        Ok(catalog)
    })
    .await?
}

/// Load a saved `events-schedule` response instead of fetching (offline dev).
pub const EVENTS_FILE_ENV: &str = "ARCLENS_EVENTS_FILE";
/// Events are refetched after this long.
pub const EVENTS_MAX_AGE: Duration = Duration::from_secs(30 * 60);

/// The condition schedule for `region` (`None`: MetaForge's default):
/// fetched and cached per region; the cache is used when offline.
pub async fn load_events(
    paths: Paths,
    region: Option<String>,
) -> Result<arclens_data::metaforge::Schedule, String> {
    use arclens_data::metaforge;
    if let Some(file) = std::env::var_os(EVENTS_FILE_ENV) {
        let bytes = tokio::fs::read(&file).await.map_err(|e| e.to_string())?;
        return metaforge::parse_schedule(&bytes).map_err(|e| e.to_string());
    }
    let name = region.as_deref().unwrap_or("auto");
    let cache = paths.cache.join(format!("events-schedule-{name}.json"));
    let fetched = async {
        let bytes = http_client()
            .get(metaforge::events_schedule_url(region.as_deref()))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let schedule = metaforge::parse_schedule(&bytes)?;
        if let Some(dir) = cache.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&cache, &bytes).await;
        Ok::<_, arclens_data::Error>(schedule)
    }
    .await;
    match fetched {
        Ok(schedule) => Ok(schedule),
        Err(error) => {
            tracing::warn!(%error, "event schedule fetch failed; trying cache");
            let bytes = tokio::fs::read(&cache)
                .await
                .map_err(|_| format!("could not load the event schedule: {error}"))?;
            metaforge::parse_schedule(&bytes).map_err(|e| e.to_string())
        }
    }
}

/// Load `<map>.json` files (saved `game-map-data` responses) from this
/// directory instead of fetching (offline dev).
pub const MAP_DATA_DIR_ENV: &str = "ARCLENS_MAP_DATA_DIR";
/// Markers change rarely; refetch them once a day.
const MAP_DATA_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

/// Markers of one map (MetaForge id): from the cache while it is fresh,
/// else fetched; a stale cache is the offline fallback.
pub async fn load_markers(paths: Paths, map: String) -> Result<Vec<arclens_core::Marker>, String> {
    use arclens_data::metaforge;
    // The game's place names come along: they locate the in-game view and
    // name places on the app's map.
    let parse = |bytes: &[u8]| {
        metaforge::parse_map_markers(bytes, &map)
            .map(|mut markers| {
                markers.extend(arclens_data::labels::map_labels(&map));
                markers
            })
            .map_err(|e| e.to_string())
    };
    if let Some(dir) = std::env::var_os(MAP_DATA_DIR_ENV) {
        let file = std::path::Path::new(&dir).join(format!("{map}.json"));
        let bytes = tokio::fs::read(&file)
            .await
            .map_err(|e| format!("{}: {e}", file.display()))?;
        return parse(&bytes);
    }
    let cache = paths.map_data(&map);
    let fresh = tokio::fs::metadata(&cache)
        .await
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age < MAP_DATA_MAX_AGE));
    if fresh && let Ok(bytes) = tokio::fs::read(&cache).await {
        return parse(&bytes);
    }
    let fetched = async {
        let bytes = http_client()
            .get(metaforge::map_data_url(&map))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        // Validate before caching.
        metaforge::parse_map_markers(&bytes, &map)?;
        if let Some(dir) = cache.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&cache, &bytes).await;
        Ok::<_, arclens_data::Error>(bytes)
    }
    .await;
    match fetched {
        // Parsed again so the place names come along (cheap next to the
        // download).
        Ok(bytes) => parse(&bytes),
        Err(error) => {
            tracing::warn!(%error, map, "map data fetch failed; trying cache");
            let bytes = tokio::fs::read(&cache)
                .await
                .map_err(|_| format!("could not load markers: {error}"))?;
            parse(&bytes)
        }
    }
}
