//! Shared UI for ARClens: design tokens and the item card.
//!
//! Both the companion app and the overlay render items through
//! [`item_card`], so a player sees the same layout and colours everywhere.

pub mod card;
pub mod format;
pub mod markers;
pub mod palette;

pub use card::{CardSize, ItemCard, decode_icon, item_card};
