//! On-disk cache of the last good [`Catalog`], so ARClens works offline and
//! does not hammer upstream APIs on every launch.

use crate::{Catalog, Error};
use std::path::{Path, PathBuf};

/// JSON file cache at a fixed path (normally `$XDG_CACHE_HOME/arclens/catalog.json`).
#[derive(Debug, Clone)]
pub struct DiskCache {
    path: PathBuf,
}

impl DiskCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the cached catalog. A missing file is `Ok(None)`, not an error.
    pub fn load(&self) -> Result<Option<Catalog>, Error> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Io(e)),
        };
        let mut catalog: Catalog = serde_json::from_slice(&bytes)?;
        catalog.reindex();
        Ok(Some(catalog))
    }

    /// Atomically replaces the cache file (write to temp file, then rename).
    pub fn store(&self, catalog: &Catalog) -> Result<(), Error> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(catalog)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::{Item, ItemId};

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("nope.json"));
        assert!(cache.load().unwrap().is_none());
    }

    #[test]
    fn round_trips_and_reindexes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("sub/catalog.json"));
        let item = Item {
            id: ItemId::new("battery"),
            name: "Battery".into(),
            aliases: Vec::new(),
            description: None,
            rarity: None,
            category: None,
            value: Some(250),
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: None,
        };
        cache
            .store(&Catalog::new("test", vec![item], Vec::new()))
            .unwrap();

        let loaded = cache.load().unwrap().expect("cache present");
        assert_eq!(loaded.source, "test");
        assert_eq!(
            loaded.item(&ItemId::new("battery")).map(|i| i.value),
            Some(Some(250))
        );
    }
}
