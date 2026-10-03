//! Frame analysis: locate ARC Raiders item tooltips in a screen capture.
//!
//! Everything here is pure image processing on an RGB frame: no capture,
//! no I/O, no game access. See `docs/vision/` for what the UI looks like and
//! why this approach was chosen.
//!
//! Pipeline:
//! 1. [`find_panels`]: cream-coloured panels (hover tooltips, and on trader
//!    screens also the persistent purchase panel).
//! 2. [`name_line`]: the bold item-name line at the top of a panel.
//! 3. (next) recognise the name text and fuzzy-match it against the catalog.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "pixel geometry: frame dimensions are < 2^16, so u32/usize/f32 \
              conversions here are exact and non-negative"
)]

mod geometry;
mod panel;
mod text;

pub use geometry::Rect;
pub use panel::{PanelParams, find_panels};
pub use text::name_line;
