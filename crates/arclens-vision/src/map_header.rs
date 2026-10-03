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
/// Left end of the top bar's "MAP" tab, whose outline is drawn when that
/// tab is selected.
const MAP_TAB_EDGE: [f32; 4] = [0.486, 0.022, 0.0102, 0.039];
/// Right end of the same outline. Both ends are needed: in raid the HUD
/// compass ("W 276") sits where the tab's left end is (field report
/// 2026-10-03: a bush opened the map panel).
const MAP_TAB_RIGHT_EDGE: [f32; 4] = [0.524, 0.022, 0.0102, 0.039];
/// Share of bright pixels in each tab edge when outlined: ~6-8 % measured
/// on Dam and Buried City, 0 on inventory, raid and trader screens.
const MAP_TAB_MIN_BRIGHT: f32 = 0.03;
/// The map title sits on the dark legend panel: ≥ 83 % of the title strip
/// is near-black on map frames. Bright scenery there is not the map.
const TITLE_MIN_DARK: f32 = 0.6;
/// The "QUESTS" header of the quest panel on the map's left (~20 % bright
/// when the panel is open).
const QUESTS_HEADER: [f32; 4] = [0.035, 0.132, 0.039, 0.025];
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

/// Whether `frame` shows the map screen: the "MAP" tab is outlined and the
/// map panel has a title. Cheap (no OCR); run it every frame.
pub fn is_map_screen(frame: &RgbImage) -> bool {
    bright_share(frame, MAP_TAB_EDGE) >= MAP_TAB_MIN_BRIGHT
        && bright_share(frame, MAP_TAB_RIGHT_EDGE) >= MAP_TAB_MIN_BRIGHT
        && dark_share(frame, TITLE) >= TITLE_MIN_DARK
        && bright_text(frame, TITLE).is_some()
}

/// Whether the quest panel covers the left of the map (markers must not
/// be drawn over it). Only meaningful on the map screen.
pub fn quest_panel_open(frame: &RgbImage) -> bool {
    bright_share(frame, QUESTS_HEADER) >= 0.08
}

/// Share of near-black pixels in the fractional region.
fn dark_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    let area = region(frame, fraction);
    let (mut dark, mut total) = (0u32, 0u32);
    for row in area.y..area.bottom().min(frame.height()) {
        for col in area.x..area.right().min(frame.width()) {
            total += 1;
            dark += u32::from(frame.get_pixel(col, row).0.iter().all(|&c| c <= 60));
        }
    }
    if total == 0 {
        0.0
    } else {
        dark as f32 / total as f32
    }
}

fn region(frame: &RgbImage, fraction: [f32; 4]) -> Rect {
    let (width, height) = (frame.width() as f32, frame.height() as f32);
    Rect::new(
        (fraction[0] * width) as u32,
        (fraction[1] * height) as u32,
        ((fraction[2] * width) as u32).max(1),
        ((fraction[3] * height) as u32).max(1),
    )
}

fn is_bright(frame: &RgbImage, col: u32, row: u32) -> bool {
    frame.get_pixel(col, row).0.iter().all(|&c| c >= 190)
}

/// Share of bright pixels in the fractional region.
fn bright_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    let area = region(frame, fraction);
    let (mut bright, mut total) = (0u32, 0u32);
    for row in area.y..area.bottom().min(frame.height()) {
        for col in area.x..area.right().min(frame.width()) {
            total += 1;
            bright += u32::from(is_bright(frame, col, row));
        }
    }
    if total == 0 {
        0.0
    } else {
        bright as f32 / total as f32
    }
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
pub(crate) fn bright_text(frame: &RgbImage, fraction: [f32; 4]) -> Option<Rect> {
    let region = region(frame, fraction);
    let bright = |col: u32, row: u32| is_bright(frame, col, row);
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
