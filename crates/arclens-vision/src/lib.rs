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
//! 3. [`NameReader`]: recognise the name text (OCR on just those lines).
//!    Matching the text to a catalogue item is `arclens_data::match_name`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "pixel geometry: frame dimensions are < 2^16, so u32/usize/f32 \
              conversions here are exact and non-negative"
)]

mod analyzer;
mod footer;
mod geometry;
mod hovered_side;
mod map_header;
mod map_labels;
mod map_motion;
mod panel;
mod read;
mod text;

pub use analyzer::{Analyzer, Hover, MapLabel};
pub use footer::{FooterInfo, footer, footer_cells, parse_value, value_cells};
pub use geometry::Rect;
pub use hovered_side::{Side, hovered_side, item_side};
pub use map_header::{MapHeader, is_map_screen, quest_panel_open, read_map_header};
pub use map_labels::{LabelParams, find_map_labels};
pub use map_motion::{Estimate, Footprint, Motion, MotionTracker, TRACK_REGION};
pub use panel::{PanelParams, find_panels};
pub use read::{NameReader, RECOGNITION_MODEL_URL};
pub use text::{name_line, name_lines};
