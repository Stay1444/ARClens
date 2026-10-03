//! An in-memory snapshot of all game data ARClens needs.

use arclens_core::{Item, ItemId, MapId, Marker};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Everything fetched from a [`crate::Provider`], plus where and when it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// Name of the provider that produced this snapshot (for attribution).
    pub source: String,
    pub fetched_at: SystemTime,
    pub items: Vec<Item>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(skip)]
    index: HashMap<ItemId, usize>,
}

impl Catalog {
    pub fn new(source: impl Into<String>, items: Vec<Item>, markers: Vec<Marker>) -> Self {
        let mut catalog = Self {
            source: source.into(),
            fetched_at: SystemTime::now(),
            items,
            markers,
            index: HashMap::new(),
        };
        catalog.reindex();
        catalog
    }

    /// Rebuilds the id index. Must be called after deserialising.
    pub fn reindex(&mut self) {
        self.index = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.clone(), i))
            .collect();
    }

    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.index.get(id).map(|&i| &self.items[i])
    }

    pub fn markers_on<'a>(&'a self, map: &'a MapId) -> impl Iterator<Item = &'a Marker> + 'a {
        self.markers.iter().filter(move |m| &m.map == map)
    }

    /// Whether the snapshot is older than `max_age` (or from the future,
    /// which means the clock moved and we should refetch anyway).
    pub fn is_stale(&self, max_age: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.fetched_at)
            .map_or(true, |age| age > max_age)
    }
}
