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
fn bright_text(frame: &RgbImage, [fx, fy, fw, fh]: [f32; 4]) -> Option<Rect> {
    let (w, h) = (frame.width() as f32, frame.height() as f32);
    let region = Rect::new(
        (fx * w) as u32,
        (fy * h) as u32,
        (fw * w) as u32,
        (fh * h) as u32,
    );
    let bright = |x: u32, y: u32| {
        let [r, g, b] = frame.get_pixel(x, y).0;
        r.min(g).min(b) >= 190
    };
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let mut count = 0u32;
    for row in region.y..region.bottom().min(frame.height()) {
        for col in region.x..region.right().min(frame.width()) {
            if bright(col, row) {
                count += 1;
                x0 = x0.min(col);
                y0 = y0.min(row);
                x1 = x1.max(col);
                y1 = y1.max(row);
            }
        }
    }
    // Text, not a stray highlight: enough pixels, and wider than tall.
    (count >= 40 && x1 > x0 + 2 * (y1 - y0)).then(|| Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}
