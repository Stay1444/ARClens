//! Map-condition icons for the Events tab: MetaForge's per-event icon, else
//! RaidTheory's per-type icon. Downloaded once to the cache directory,
//! decoded once off the UI thread, kept in memory by URL.

use crate::data;
use crate::paths::Paths;
use arclens_core::ScheduledEvent;
use arclens_data::{Catalog, ImageCache};
use iced::widget::image::Handle;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const ICON_PX: u32 = 64;

#[derive(Debug)]
pub struct EventIcons {
    cache: ImageCache,
    /// Icon URL by event key.
    urls: HashMap<String, String>,
    /// Decoded icon and its file, by URL.
    loaded: HashMap<String, (Handle, PathBuf)>,
    /// URLs requested, or known to be unavailable: never re-requested.
    requested: HashSet<String>,
}

impl EventIcons {
    pub fn new(paths: &Paths) -> Self {
        Self {
            cache: ImageCache::new(paths.cache.join("images"), data::http_client()),
            urls: HashMap::new(),
            loaded: HashMap::new(),
            requested: HashSet::new(),
        }
    }

    /// The decoded icon for an event name, once loaded.
    pub fn get(&self, name: &str) -> Option<&Handle> {
        self.entry(name).map(|(handle, _)| handle)
    }

    /// The icon's file, for the overlay (which decodes it itself).
    pub fn path(&self, name: &str) -> Option<&Path> {
        self.entry(name).map(|(_, path)| path.as_path())
    }

    fn entry(&self, name: &str) -> Option<&(Handle, PathBuf)> {
        self.loaded
            .get(self.urls.get(&arclens_data::event_key(name))?)
    }

    /// Picks each event's icon URL and starts downloads for new ones.
    pub fn request(
        &mut self,
        events: &[ScheduledEvent],
        catalog: Option<&Catalog>,
    ) -> Vec<impl Future<Output = (String, Option<(Handle, PathBuf)>)> + use<>> {
        for event in events {
            let url = event
                .icon
                .clone()
                .filter(|u| !u.is_empty())
                .or_else(|| catalog?.event_icon(&event.name).map(str::to_owned));
            if let Some(url) = url {
                self.urls
                    .entry(arclens_data::event_key(&event.name))
                    .or_insert(url);
            }
        }
        let new: Vec<String> = self
            .urls
            .values()
            .filter(|url| self.requested.insert((*url).clone()))
            .cloned()
            .collect();
        new.into_iter()
            .map(|url| {
                let cache = self.cache.clone();
                async move {
                    let icon = match cache.fetch(&url).await {
                        Ok(Some(path)) => tokio::task::spawn_blocking(move || {
                            arclens_ui::decode_icon(&path, ICON_PX).map(|icon| (icon, path))
                        })
                        .await
                        .ok()
                        .flatten(),
                        Ok(None) => None,
                        Err(error) => {
                            tracing::debug!(%error, url, "event icon unavailable");
                            None
                        }
                    };
                    (url, icon)
                }
            })
            .collect()
    }

    pub fn insert(&mut self, url: String, icon: Option<(Handle, PathBuf)>) {
        if let Some(icon) = icon {
            self.loaded.insert(url, icon);
        }
    }
}
