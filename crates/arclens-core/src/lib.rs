//! Pure domain model and logic for ARClens.
//!
//! This crate must stay free of I/O, async runtimes and UI dependencies so it
//! can be shared by every other crate and tested in isolation.

pub mod advice;
pub mod item;
pub mod map;

pub use advice::{Advice, Verdict, advise};
pub use item::{Item, ItemId, ItemQuantity, Rarity, Requirement, RequirementKind};
pub use map::{MapId, MapPoint, Marker, MarkerKind, Transform};
