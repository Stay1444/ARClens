//! An in-memory snapshot of all game data ARClens needs.

use arclens_core::{Item, ItemId, MapId, Marker};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, SystemTime};

/// Everything fetched from a [`crate::Provider`], plus where and when it came from.
/// Bump when the catalogue's content or meaning changes (new fields filled by
/// the loader), so caches written by older versions are rebuilt.
pub const SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// [`SCHEMA_VERSION`] of the code that built it (0 for pre-versioning).
    #[serde(default)]
    pub schema: u32,
    /// Name of the provider that produced this snapshot (for attribution).
    pub source: String,
    pub fetched_at: SystemTime,
    pub items: Vec<Item>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    /// Workshop stations, for the progress editor.
    #[serde(default)]
    pub stations: Vec<arclens_core::Station>,
    /// Quests and projects, for the progress editor.
    #[serde(default)]
    pub quests: Vec<arclens_core::Quest>,
    #[serde(default)]
    pub projects: Vec<arclens_core::Project>,
    /// Map-condition icon URLs by [`crate::event_key`].
    #[serde(default)]
    pub event_icons: BTreeMap<String, String>,
    #[serde(skip)]
    index: HashMap<ItemId, usize>,
}

impl Catalog {
    pub fn new(source: impl Into<String>, items: Vec<Item>, markers: Vec<Marker>) -> Self {
        let mut catalog = Self {
            schema: SCHEMA_VERSION,
            source: source.into(),
            fetched_at: SystemTime::now(),
            items,
            markers,
            stations: Vec::new(),
            quests: Vec::new(),
            projects: Vec::new(),
            event_icons: BTreeMap::new(),
            index: HashMap::new(),
        };
        catalog.reindex();
        catalog
    }

    #[must_use]
    pub fn with_stations(mut self, stations: Vec<arclens_core::Station>) -> Self {
        self.stations = stations;
        self
    }

    #[must_use]
    pub fn with_quests_and_projects(
        mut self,
        quests: Vec<arclens_core::Quest>,
        projects: Vec<arclens_core::Project>,
    ) -> Self {
        self.quests = quests;
        self.projects = projects;
        self
    }

    #[must_use]
    pub fn with_event_icons(mut self, icons: BTreeMap<String, String>) -> Self {
        self.event_icons = icons;
        self
    }

    /// Icon URL for a map condition, by name ("Night Raid").
    pub fn event_icon(&self, name: &str) -> Option<&str> {
        self.event_icons
            .get(&crate::event_key(name))
            .map(String::as_str)
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
        self.schema != SCHEMA_VERSION
            || SystemTime::now()
                .duration_since(self.fetched_at)
                .map_or(true, |age| age > max_age)
    }
}
