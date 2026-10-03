//! Game-data providers, caching and search for ARClens.
//!
//! Providers turn an upstream source into a [`Catalog`]. See
//! `docs/research/data-sources.md` for which sources exist and their terms.

mod cache;
mod catalog;
pub mod download;
mod error;
pub mod raidtheory;
mod search;

pub use cache::DiskCache;
pub use catalog::Catalog;
pub use error::Error;
pub use search::ItemSearch;
