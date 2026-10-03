//! XDG locations used by the app.

use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    /// `$XDG_CACHE_HOME/arclens` — safe to delete at any time.
    pub cache: PathBuf,
    /// `$XDG_CONFIG_HOME/arclens` — the player's own settings and progress.
    pub config: PathBuf,
    /// `$XDG_STATE_HOME/arclens` — things worth keeping across runs that
    /// aren't settings (e.g. the screen-capture restore token).
    pub state: PathBuf,
}

impl Paths {
    pub fn discover() -> anyhow::Result<Self> {
        let dirs = ProjectDirs::from("", "", "arclens")
            .ok_or_else(|| anyhow::anyhow!("could not determine a home directory"))?;
        Ok(Self {
            cache: dirs.cache_dir().to_owned(),
            config: dirs.config_dir().to_owned(),
            state: dirs
                .state_dir()
                .unwrap_or_else(|| dirs.data_local_dir())
                .to_owned(),
        })
    }

    pub fn catalog_cache(&self) -> PathBuf {
        self.cache.join("catalog.json")
    }

    pub fn progress(&self) -> PathBuf {
        self.config.join("progress.json")
    }

    pub fn capture_token(&self) -> PathBuf {
        self.state.join("screencast-restore-token")
    }

    pub fn icons_dir(&self) -> PathBuf {
        self.cache.join("icons")
    }

    /// Small app settings (server region, …).
    pub fn settings(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    /// Which map-marker kinds the player hid.
    pub fn marker_filter(&self) -> PathBuf {
        self.config.join("marker-filter.json")
    }

    /// Cached `game-map-data` response for `map`.
    pub fn map_data(&self, map: &str) -> PathBuf {
        self.cache.join("map-data").join(format!("{map}.json"))
    }

    pub fn raidtheory_dir(&self) -> PathBuf {
        self.cache.join("raidtheory")
    }
}
