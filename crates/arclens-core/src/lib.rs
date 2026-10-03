//! Pure domain model and logic for ARClens.
//!
//! This crate must stay free of I/O, async runtimes and UI dependencies so it
//! can be shared by every other crate and tested in isolation.

pub mod advice;
pub mod events;
pub mod item;
pub mod map;
pub mod progress;

pub use advice::{
    Advice, PARTS_MIN_VALUE_PERCENT, Place, Situation, Verdict, advise, advise_in, breakdown,
};
pub use events::{Agenda, ScheduledEvent, agenda, countdown};
pub use item::{Item, ItemId, ItemQuantity, Rarity, Requirement, RequirementKind};
pub use map::{MapId, MapPoint, Marker, MarkerKind, Transform};
pub use progress::{Progress, Station};
