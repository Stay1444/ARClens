//! The Workshop overview: the row of station tiles near the bottom centre,
//! each (except Scrappy) with its level as a roman numeral bottom right.
//!
//! Detection uses fixed UI elements only (no OCR): the outlined WORKSHOP tab
//! in the top bar and the dark navy footer strip of the seven station tiles.
//! Levels are read by counting the numeral's vertical bars (I, II, III), so
//! no recognition model is needed.
//!
//! All regions are fractions of the frame, measured on one 2560×1440
//! screenshot (2026-10-04, `tests/fixtures/frames/workshop_overview.jpg`).
//! The tile layout is assumed to scale with the frame; **unverified** at
//! other resolutions and aspect ratios, and for stations not yet built
//! (level 0: no fixture yet, may show no numeral or a lock).

use crate::geometry::Rect;
use crate::map_header::{bright_share, region};
use image::RgbImage;

/// Station ids (as in the RaidTheory `hideout/` data) of the tiles, left to
/// right. Identified from each tile's type icon on the 2026-10-04 fixture:
///
/// 0. `scrappy`: the rooster, no numeral.
/// 1. `workbench`: bench icon.
/// 2. `weapon_bench` (Gunsmith): gear and gun icon; "II" in the fixture,
///    matching the maintainer's Gunsmith 2.
/// 3. `equipment_bench` (Gear Bench): shield icon.
/// 4. `utility_bench` (Utility Station): crossed tools.
/// 5. `med_station` (Medical Lab): plus sign.
/// 6. `explosives_bench` (Explosives Station): grenade.
/// 7. `refiner`: atom-like rings.
///
/// The order comes from one account; it is **unverified** whether the game
/// ever reorders tiles (e.g. while a station is unbuilt).
pub const WORKSHOP_TILES: [&str; 8] = [
    "scrappy",
    "workbench",
    "weapon_bench",
    "equipment_bench",
    "utility_bench",
    "med_station",
    "explosives_bench",
    "refiner",
];

// Reference frame the pixel measurements below were taken on.
const REF_W: f32 = 2560.0;
const REF_H: f32 = 1440.0;

/// Both ends of the outlined "WORKSHOP" tab (a pill outline, 3 px at
/// x 238–240 and 452–454, y 16–68), around their vertical middle.
const TAB_LEFT: [f32; 4] = [234.0 / REF_W, 30.0 / REF_H, 12.0 / REF_W, 24.0 / REF_H];
const TAB_RIGHT: [f32; 4] = [446.0 / REF_W, 30.0 / REF_H, 12.0 / REF_W, 24.0 / REF_H];
/// Just inside the left end of the tab: dark (the selected-tab dot and the
/// text start further right).
const TAB_INSIDE: [f32; 4] = [244.0 / REF_W, 26.0 / REF_H, 12.0 / REF_W, 32.0 / REF_H];
/// Bright share of a tab end when outlined (~0.17 measured).
const MIN_OUTLINE: f32 = 0.06;
/// Bright share allowed inside the tab next to the outline.
const MAX_INSIDE: f32 = 0.02;

/// Left edge of station tile 1 (the outline column), the distance between
/// tiles, and Scrappy's (tile 0) left edge. Tiles are 137 px wide.
const FIRST_STATION_LEFT: f32 = 849.0 / REF_W;
const TILE_PITCH: f32 = 152.0 / REF_W;
const SCRAPPY_LEFT: f32 = 661.0 / REF_W;

/// The tile footer between the type icon and the numeral, relative to the
/// tile's left edge: `[dx, y, w, h]`. Uniform dark navy on every tile.
const FOOTER: [f32; 4] = [40.0 / REF_W, 1256.0 / REF_H, 60.0 / REF_W, 20.0 / REF_H];
/// Share of navy pixels a footer needs (~1.0 measured).
const MIN_NAVY: f32 = 0.8;

/// Where the numeral sits, relative to the tile's left edge: `[dx, y, w, h]`.
/// Glyphs measured at y 1258–1273, centred ~122 px right of the tile edge;
/// the box stops short of the tile outline at +137 and its rounded corner.
const NUMERAL: [f32; 4] = [102.0 / REF_W, 1255.0 / REF_H, 32.0 / REF_W, 23.0 / REF_H];
/// Rows of the numeral's stems, between the serifs (which join the bars of
/// "II" at top and bottom): `[y, h]`.
const STEMS: [f32; 2] = [1262.0 / REF_H, 8.0 / REF_H];
/// A stem is ~3 px wide at 1440 p (gap between stems 2 px); anything wider
/// than this is not an "I" (a lock icon, another glyph).
const MAX_STEM_WIDTH: f32 = 7.0 / REF_H;

/// Whether `frame` shows the Workshop overview. Cheap; run it every frame.
pub fn is_workshop_overview(frame: &RgbImage) -> bool {
    bright_share(frame, TAB_LEFT) >= MIN_OUTLINE
        && bright_share(frame, TAB_RIGHT) >= MIN_OUTLINE
        && bright_share(frame, TAB_INSIDE) <= MAX_INSIDE
        && (1..WORKSHOP_TILES.len()).all(|tile| navy_share(frame, footer(tile)) >= MIN_NAVY)
}

/// Station levels shown on the tiles, as `(tile index, level)` with the index
/// into [`WORKSHOP_TILES`]. Scrappy (tile 0) shows no level and is skipped,
/// as is any tile whose numeral can't be read (unknown, not level 0).
///
/// Only meaningful when [`is_workshop_overview`] holds.
pub fn read_workshop_levels(frame: &RgbImage) -> Vec<(usize, u32)> {
    (1..WORKSHOP_TILES.len())
        .filter_map(|tile| count_stems(frame, tile).map(|level| (tile, level)))
        .collect()
}

/// Left edge of tile `tile` as a fraction of the frame width.
fn tile_left(tile: usize) -> f32 {
    if tile == 0 {
        SCRAPPY_LEFT
    } else {
        FIRST_STATION_LEFT + (tile - 1) as f32 * TILE_PITCH
    }
}

/// `[dx, y, w, h]` relative to a tile → absolute `[x, y, w, h]` fractions.
fn in_tile(tile: usize, [dx, y, w, h]: [f32; 4]) -> [f32; 4] {
    [tile_left(tile) + dx, y, w, h]
}

fn footer(tile: usize) -> [f32; 4] {
    in_tile(tile, FOOTER)
}

/// Share of the tiles' dark navy background (~(9, 13, 25)) in the region.
fn navy_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    let area = region(frame, fraction);
    let (mut navy, mut total) = (0u32, 0u32);
    for row in area.y..area.bottom().min(frame.height()) {
        for col in area.x..area.right().min(frame.width()) {
            let [r, g, b] = frame.get_pixel(col, row).0;
            total += 1;
            navy += u32::from(b <= 60 && b >= r.saturating_add(8) && b >= g.saturating_add(5));
        }
    }
    if total == 0 {
        0.0
    } else {
        navy as f32 / total as f32
    }
}

/// Numeral ink: light grey-blue (~(100, 103, 118)) on the navy footer.
fn is_numeral_ink([r, g, b]: [u8; 3]) -> bool {
    r.min(g).min(b) >= 60
}

/// The numeral's value as its count of vertical bars (1–3), or `None` when
/// the box holds no numeral, more than three bars, or a wider shape.
fn count_stems(frame: &RgbImage, tile: usize) -> Option<u32> {
    let [x, _, w, _] = in_tile(tile, NUMERAL);
    let area: Rect = region(frame, [x, STEMS[0], w, STEMS[1]]);
    let rows = area.y..area.bottom().min(frame.height());
    let rows_needed = (area.height * 3).div_ceil(4);
    let column_is_stem = |col: u32| {
        let ink = rows
            .clone()
            .filter(|&row| is_numeral_ink(frame.get_pixel(col, row).0))
            .count();
        ink as u32 >= rows_needed
    };
    let mut runs: Vec<u32> = Vec::new();
    let mut previous = false;
    for col in area.x..area.right().min(frame.width()) {
        let stem = column_is_stem(col);
        match (stem, previous) {
            (true, true) => {
                if let Some(width) = runs.last_mut() {
                    *width += 1;
                }
            }
            (true, false) => runs.push(1),
            _ => {}
        }
        previous = stem;
    }
    let max_width = ((MAX_STEM_WIDTH * frame.height() as f32).round() as u32).max(2);
    let count = runs.len() as u32;
    ((1..=3).contains(&count) && runs.iter().all(|&width| width <= max_width)).then_some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAVY: [u8; 3] = [9, 13, 25];
    const INK: [u8; 3] = [100, 103, 118];

    fn paint(frame: &mut RgbImage, fraction: [f32; 4], colour: [u8; 3]) {
        let area = region(frame, fraction);
        for row in area.y..area.bottom() {
            for col in area.x..area.right() {
                frame.put_pixel(col, row, image::Rgb(colour));
            }
        }
    }

    /// Synthetic overview: outlined tab, navy footers, `levels[tile - 1]`
    /// bars per station tile.
    fn synthetic(levels: [u32; 7]) -> RgbImage {
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([30, 25, 20]));
        for end in [TAB_LEFT, TAB_RIGHT] {
            paint(
                &mut frame,
                [end[0] + 4.0 / REF_W, end[1], 2.0 / REF_W, end[3]],
                [240, 240, 240],
            );
        }
        for (index, &level) in levels.iter().enumerate() {
            let tile = index + 1;
            let left = tile_left(tile);
            paint(
                &mut frame,
                [left, 1250.0 / REF_H, 137.0 / REF_W, 32.0 / REF_H],
                NAVY,
            );
            for bar in 0..level {
                let x = left + (110.0 + 5.0 * bar as f32) / REF_W;
                paint(
                    &mut frame,
                    [x, 1258.0 / REF_H, 3.0 / REF_W, 16.0 / REF_H],
                    INK,
                );
            }
        }
        frame
    }

    #[test]
    fn counts_bars_and_skips_missing_numerals() {
        let frame = synthetic([1, 2, 3, 0, 1, 1, 1]);
        assert!(is_workshop_overview(&frame));
        assert_eq!(
            read_workshop_levels(&frame),
            vec![(1, 1), (2, 2), (3, 3), (5, 1), (6, 1), (7, 1)]
        );
    }

    #[test]
    fn needs_the_outlined_tab() {
        let mut frame = synthetic([1; 7]);
        paint(&mut frame, TAB_LEFT, [30, 25, 20]);
        assert!(!is_workshop_overview(&frame));
    }

    #[test]
    fn a_wide_shape_is_not_a_numeral() {
        let mut frame = synthetic([1; 7]);
        let [x, _, _, _] = in_tile(4, NUMERAL);
        paint(
            &mut frame,
            [x + 4.0 / REF_W, 1258.0 / REF_H, 14.0 / REF_W, 16.0 / REF_H],
            INK,
        );
        assert!(
            !read_workshop_levels(&frame)
                .iter()
                .any(|&(tile, _)| tile == 4)
        );
    }
}
