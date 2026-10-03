//! On-disk cache of small remote images (map-condition icons).
//!
//! Each URL is downloaded once to `<dir>/<hash>.<ext>`; later calls return
//! the file. Only hosts listed in `docs/research/data-sources.md` are
//! fetched.

use crate::Error;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Image hosts we may fetch from.
const ALLOWED_HOSTS: &[&str] = &["cdn.metaforge.app", "cdn.arctracker.io"];

#[derive(Debug, Clone)]
pub struct ImageCache {
    dir: PathBuf,
    client: reqwest::Client,
}

impl ImageCache {
    pub fn new(dir: impl Into<PathBuf>, client: reqwest::Client) -> Self {
        Self {
            dir: dir.into(),
            client,
        }
    }

    /// The local file for `url`, downloading it first if needed. `Ok(None)`
    /// for URLs we don't fetch (other hosts, not https).
    pub async fn fetch(&self, url: &str) -> Result<Option<PathBuf>, Error> {
        let Some(name) = file_name(url) else {
            return Ok(None);
        };
        let path = self.dir.join(name);
        if path.is_file() {
            return Ok(Some(path));
        }
        let bytes = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        tokio::fs::create_dir_all(&self.dir).await?;
        let tmp = path.with_extension("part");
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(Some(path))
    }
}

/// `<hash>.<ext>` for an allowed https URL, else `None`.
fn file_name(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    if !ALLOWED_HOSTS.contains(&host) {
        return None;
    }
    let ext = path
        .rsplit('/')
        .next()?
        .split(['?', '#'])
        .next()?
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| matches!(ext.as_str(), "png" | "webp" | "jpg" | "jpeg"))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    Some(format!("{:016x}.{ext}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_only_allowed_image_urls() {
        let name = file_name("https://cdn.metaforge.app/arc-raiders/custom/coldsnap.webp").unwrap();
        assert_eq!((name.len(), &name[17..]), (21, "webp"));
        assert!(file_name("https://cdn.arctracker.io/map-events/night_raid.png?v=2").is_some());
        assert_eq!(file_name("http://cdn.arctracker.io/a.png"), None);
        assert_eq!(file_name("https://evil.example/a.png"), None);
        assert_eq!(file_name("https://cdn.arctracker.io/a.svg"), None);
        // Stable across calls (the cache key).
        assert_eq!(
            file_name("https://cdn.arctracker.io/a.png"),
            file_name("https://cdn.arctracker.io/a.png")
        );
    }
}
