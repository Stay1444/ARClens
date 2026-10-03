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
