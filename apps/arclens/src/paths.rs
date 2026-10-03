//! XDG locations used by the app.

use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    /// `$XDG_CACHE_HOME/arclens` — safe to delete at any time.
    pub cache: PathBuf,
}

impl Paths {
    pub fn discover() -> anyhow::Result<Self> {
        let dirs = ProjectDirs::from("", "", "arclens")
            .ok_or_else(|| anyhow::anyhow!("could not determine a home directory"))?;
        Ok(Self {
            cache: dirs.cache_dir().to_owned(),
        })
    }

    pub fn catalog_cache(&self) -> PathBuf {
        self.cache.join("catalog.json")
    }

    pub fn raidtheory_dir(&self) -> PathBuf {
        self.cache.join("raidtheory")
    }
}
