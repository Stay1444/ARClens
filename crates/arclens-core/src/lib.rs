//! Pure domain model and logic for ARClens.
//!
//! This crate must stay free of I/O, async runtimes and UI dependencies so it
//! can be shared by every other crate and tested in isolation.

pub mod advice;
pub mod areas;
pub mod events;
pub mod item;
pub mod map;
pub mod presets;
pub mod progress;
pub mod spots;

pub use advice::{
    Advice, PARTS_MIN_VALUE_PERCENT, Place, Situation, Verdict, advise, advise_in, breakdown,
};
pub use areas::{MarkerArea, MarkerLayout, layout};
pub use events::{Agenda, ScheduledEvent, agenda, condition_key, countdown, due_alerts};
pub use item::{
    Item, ItemDetails, ItemFilter, ItemId, ItemQuantity, Rarity, Requirement, RequirementKind,
    Vendor, categories,
};
pub use map::{
    MapId, MapPoint, Marker, MarkerFilter, PointPair, Transform, humanize, marker_counts,
};
pub use presets::{Preset, PresetBook};
pub use progress::{Progress, Project, Quest, Station};
pub use spots::{ItemSpots, Spot};
