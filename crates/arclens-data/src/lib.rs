//! Game-data providers, caching and search for ARClens.
//!
//! Providers turn an upstream source into a [`Catalog`]. See
//! `docs/research/data-sources.md` for which sources exist and their terms.

pub mod anchors;
mod cache;
mod catalog;
pub mod download;
mod error;
mod icons;
pub mod images;
pub mod labels;
pub mod map_images;
pub mod metaforge;
mod name_match;
pub mod presets;
pub mod raidtheory;
mod search;

pub use cache::DiskCache;
pub use catalog::Catalog;
pub use error::Error;
pub use icons::IconCache;
pub use images::ImageCache;
pub use name_match::{match_name, normalize_name};
pub use search::ItemSearch;

/// Key for matching map-condition names across sources: lowercase letters
/// and digits only ("Night Raid", "night-raid" → "nightraid").
pub fn event_key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
