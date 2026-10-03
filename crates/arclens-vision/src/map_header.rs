//! The map screen's right-hand panel: map name, raid time left, and the
//! active map condition, e.g.
//!
//! ```text
//! DAM BATTLEGROUNDS — 18:55
//! Matriarch
//! ```
//!
//! It sits at a fixed place in the UI, so we read two fixed strips of the
//! frame (fractions measured at 2560×1440; the UI scales with resolution).

use crate::{NameReader, Rect};
use image::RgbImage;

/// Header line: x 78–97 % of the width, y 13.2–16.4 % of the height.
const TITLE: [f32; 4] = [0.78, 0.132, 0.19, 0.032];
/// Condition line just below it.
const CONDITION: [f32; 4] = [0.80, 0.160, 0.17, 0.028];

/// Raw text of the map panel header, if one is on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHeader {
    /// e.g. "DAM BATTLEGROUNDS - 18:55"
    pub title: String,
    /// e.g. "Matriarch" (may also be the player's name if no condition).
    pub condition: Option<String>,
}

/// Reads the header. `None` when the strip holds no bright text (map closed).
pub fn read_map_header(reader: &NameReader, frame: &RgbImage) -> anyhow::Result<Option<MapHeader>> {
    let Some(title_box) = bright_text(frame, TITLE) else {
        return Ok(None);
    };
    let Some(title) = reader.read_free_text(frame, title_box)? else {
        return Ok(None);
    };
    let condition = match bright_text(frame, CONDITION) {
        Some(rect) => reader.read_free_text(frame, rect)?,
        None => None,
    };
    Ok(Some(MapHeader { title, condition }))
}

/// Tight box around bright (white-ish) text inside the fractional `region`.
fn bright_text(frame: &RgbImage, fraction: [f32; 4]) -> Option<Rect> {
    let (width, height) = (frame.width() as f32, frame.height() as f32);
    let region = Rect::new(
        (fraction[0] * width) as u32,
        (fraction[1] * height) as u32,
        (fraction[2] * width) as u32,
        (fraction[3] * height) as u32,
    );
    let bright = |col: u32, row: u32| frame.get_pixel(col, row).0.iter().all(|&c| c >= 190);
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    let mut count = 0u32;
    for row in region.y..region.bottom().min(frame.height()) {
        for col in region.x..region.right().min(frame.width()) {
            if bright(col, row) {
                count += 1;
                left = left.min(col);
                top = top.min(row);
                right = right.max(col);
                bottom = bottom.max(row);
            }
        }
    }
    // Text, not a stray highlight: enough pixels, and wider than tall.
    (count >= 40 && right > left + 2 * (bottom - top))
        .then(|| Rect::new(left, top, right - left + 1, bottom - top + 1))
}
