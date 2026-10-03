//! On-disk cache of item icons.
//!
//! Icons are fetched once from the item's `image_url` and stored as
//! `<cache>/<item-id>.png`. UIs load them by path; the overlay receives the
//! path over IPC rather than image bytes.

use crate::Error;
use arclens_core::{Item, ItemId};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct IconCache {
    dir: PathBuf,
    /// A local dataset checkout whose `images/items/<id>.png` is preferred
    /// over the network (offline development).
    local_images: Option<PathBuf>,
    client: reqwest::Client,
}

impl IconCache {
    pub fn new(dir: impl Into<PathBuf>, client: reqwest::Client) -> Self {
        Self {
            dir: dir.into(),
            local_images: None,
            client,
        }
    }

    /// Prefer icons from a RaidTheory checkout (`<root>/images/items`).
    #[must_use]
    pub fn with_local_dataset(mut self, root: &Path) -> Self {
        self.local_images = Some(root.join("images").join("items"));
        self
    }

    /// Path of the icon if it is already available locally.
    pub fn cached(&self, id: &ItemId) -> Option<PathBuf> {
        let name = file_name(id)?;
        self.local_images
            .iter()
            .map(|dir| dir.join(&name))
            .chain(std::iter::once(self.dir.join(&name)))
            .find(|path| path.is_file())
    }

    /// Returns the icon path, downloading it first if needed. `Ok(None)` when
    /// the item has no usable image URL.
    pub async fn fetch(&self, item: &Item) -> Result<Option<PathBuf>, Error> {
        if let Some(path) = self.cached(&item.id) {
            return Ok(Some(path));
        }
        let (Some(url), Some(name)) = (&item.image_url, file_name(&item.id)) else {
            return Ok(None);
        };
        let bytes = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        let path = self.dir.join(name);
        let tmp = path.with_extension("png.part");
        tokio::fs::create_dir_all(&self.dir).await?;
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(Some(path))
    }
}

/// `<id>.png`, or `None` if the id could escape the cache directory.
fn file_name(id: &ItemId) -> Option<String> {
    let id = id.as_str();
    let safe = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    safe.then(|| format!("{id}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ids_that_could_escape_the_cache_dir() {
        assert_eq!(
            file_name(&ItemId::new("rusted_gear")).as_deref(),
            Some("rusted_gear.png")
        );
        assert_eq!(file_name(&ItemId::new("../etc/passwd")), None);
        assert_eq!(file_name(&ItemId::new("a/b")), None);
        assert_eq!(file_name(&ItemId::new("")), None);
    }

    #[test]
    fn prefers_local_dataset_images() {
        let dir = tempfile::tempdir().unwrap();
        let dataset = dir.path().join("dataset");
        std::fs::create_dir_all(dataset.join("images/items")).unwrap();
        std::fs::write(dataset.join("images/items/battery.png"), b"png").unwrap();

        let cache = IconCache::new(dir.path().join("cache"), reqwest::Client::new())
            .with_local_dataset(&dataset);
        assert_eq!(
            cache.cached(&ItemId::new("battery")),
            Some(dataset.join("images/items/battery.png"))
        );
        assert_eq!(cache.cached(&ItemId::new("wires")), None);
    }
}
