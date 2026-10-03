//! Item icons for the companion window: fetched through
//! [`arclens_data::IconCache`], decoded off the UI thread, kept in memory.

use crate::data::{self, DATA_DIR_ENV};
use crate::paths::Paths;
use arclens_core::{Item, ItemId};
use arclens_data::IconCache;
use iced::widget::image::Handle;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const THUMB_PX: u32 = 64;
const LARGE_PX: u32 = 192;

/// A decoded icon in the two sizes the UI uses.
#[derive(Debug, Clone)]
pub struct Icon {
    pub path: PathBuf,
    pub thumb: Handle,
    pub large: Handle,
}

#[derive(Debug)]
pub struct Icons {
    cache: IconCache,
    loaded: HashMap<ItemId, Icon>,
    /// Requested, or known to be unavailable — never re-requested.
    requested: HashSet<ItemId>,
}

impl Icons {
    pub fn new(paths: &Paths) -> Self {
        let mut cache = IconCache::new(paths.icons_dir(), data::http_client());
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
            cache = cache.with_local_dataset(std::path::Path::new(&dir));
        }
        Self {
            cache,
            loaded: HashMap::new(),
            requested: HashSet::new(),
        }
    }

    pub fn get(&self, id: &ItemId) -> Option<&Icon> {
        self.loaded.get(id)
    }

    /// Starts loading `item`'s icon unless already loaded or in flight.
    /// Returns the future to run, if any.
    pub fn request(
        &mut self,
        item: &Item,
    ) -> Option<impl Future<Output = (ItemId, Option<Icon>)> + use<>> {
        if !self.requested.insert(item.id.clone()) {
            return None;
        }
        let cache = self.cache.clone();
        let item = item.clone();
        Some(async move {
            let id = item.id.clone();
            let icon = match cache.fetch(&item).await {
                Ok(Some(path)) => tokio::task::spawn_blocking(move || decode(path))
                    .await
                    .ok()
                    .flatten(),
                Ok(None) => None,
                Err(error) => {
                    tracing::debug!(%error, item = %id, "icon unavailable");
                    None
                }
            };
            (id, icon)
        })
    }

    pub fn insert(&mut self, id: ItemId, icon: Option<Icon>) {
        if let Some(icon) = icon {
            self.loaded.insert(id, icon);
        }
    }
}

fn decode(path: PathBuf) -> Option<Icon> {
    Some(Icon {
        thumb: arclens_ui::decode_icon(&path, THUMB_PX)?,
        large: arclens_ui::decode_icon(&path, LARGE_PX)?,
        path,
    })
}
