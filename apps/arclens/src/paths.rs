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

    /// The catalog cache for dataset language `lang` (`en`, `es`).
    pub fn catalog_cache(&self, lang: &str) -> PathBuf {
        if lang == "en" {
            self.cache.join("catalog.json")
        } else {
            self.cache.join(format!("catalog-{lang}.json"))
        }
    }

    pub fn progress(&self) -> PathBuf {
        self.config.join("progress.json")
    }

    pub fn capture_token(&self) -> PathBuf {
        // "-cursor": sessions saved before the pointer was requested as
        // metadata restore without it (no marker tooltips), so those
        // tokens are left behind and the portal asks once more.
        self.state.join("screencast-restore-token-cursor")
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

    /// The player's map presets and which one each map and condition uses.
    pub fn presets(&self) -> PathBuf {
        self.config.join("presets.json")
    }

    /// Cached `game-map-data` response for `map`.
    pub fn map_data(&self, map: &str) -> PathBuf {
        self.cache.join("map-data").join(format!("{map}.json"))
    }

    /// Pictures of stash slots the player hovered, by item.
    pub fn stash_exemplars(&self) -> PathBuf {
        self.state.join("stash-exemplars.json")
    }

    /// Finished stash scans.
    pub fn stash_history(&self) -> PathBuf {
        self.state.join("stash-history.json")
    }

    pub fn raidtheory_dir(&self) -> PathBuf {
        self.cache.join("raidtheory")
    }
}
