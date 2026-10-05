//! The stash grid on the pause menu's INVENTORY tab: where its slots are,
//! and what each slot's corner says (a stack size or a weapon tier).
//!
//! Layout measured at 2560×1440 on the 2026-10-05 recordings
//! (`tests/fixtures/stash/`):
//!
//! - four columns, slot left edges at x = 203 + 139·c, slots 122 px wide
//!   and ~120 px tall, 139 px apart vertically;
//! - the grid scrolls; rows are clipped to y ≈ 356–1262;
//! - each slot's outline is a 1-px line tinted with the item's rarity
//!   (≥ 70 on the brightest channel), with ~16 px of dark gap between rows;
//! - the bottom ~18 % of a slot is a bar: a category glyph on the left, and
//!   on the right either a white stack size ("×60") or a grey roman
//!   numeral tier ("I" … "IV", often followed by a small "▸").
//!
//! The UI scales with the frame height. Other aspect ratios than 16:9 are
//! **unverified**; the grid is assumed centred like the rest of the menu.

use crate::Rect;
use crate::read::NameReader;
use image::RgbImage;

/// Measured at 1440p (see the module docs).
const FIRST_COLUMN_X: f32 = 203.0;
const COLUMN_PITCH: f32 = 139.0;
const SLOT_WIDTH: f32 = 122.0;
const COLUMNS: u32 = 4;
/// Rows are looked for between these y values (the visible grid).
const GRID_TOP: f32 = 300.0;
const GRID_BOTTOM: f32 = 1262.0;
/// A full slot's outline is 118–122 px tall; a clipped row is shorter.
const MIN_SLOT_HEIGHT: f32 = 115.0;
const MAX_SLOT_HEIGHT: f32 = 130.0;
/// Brightest channel of a slot outline (≥ 70 measured; the gap between
/// rows is ≤ 30, up to ~50 on a brighter background).
const OUTLINE_MIN: u8 = 55;

/// UI scale and horizontal offset of a frame, relative to 2560×1440.
fn scale(frame: &RgbImage) -> (f32, f32) {
    let s = frame.height() as f32 / 1440.0;
    let offset = (frame.width() as f32 - 2560.0 * s) / 2.0;
    (s, offset)
}

/// The stash grid's fully visible slots, row by row, left to right. Empty
/// when the frame shows no stash grid (other screens, the raid backpack).
pub fn stash_slots(frame: &RgbImage) -> Vec<Rect> {
    let (s, offset) = scale(frame);
    if offset < 0.0 {
        return Vec::new();
    }
    let x_of = |c: u32| (offset + (FIRST_COLUMN_X + COLUMN_PITCH * c as f32) * s).round() as u32;
    let width = (SLOT_WIDTH * s).round() as u32;
    rows(frame, x_of(0), s)
        .into_iter()
        .flat_map(|(top, bottom)| {
            (0..COLUMNS).map(move |c| Rect::new(x_of(c), top, width, bottom - top))
        })
        .filter(|slot| slot.right() <= frame.width())
        .collect()
}

/// The first column's outline profile, split into slot-tall runs. Tooltips
/// open to the right of the hovered slot, so this column is never covered.
fn rows(frame: &RgbImage, x: u32, s: f32) -> Vec<(u32, u32)> {
    let (top, bottom) = (
        (GRID_TOP * s) as u32,
        ((GRID_BOTTOM * s) as u32).min(frame.height()),
    );
    // The outline is one pixel wide: look a little either side of it, so
    // rounding at other resolutions still hits it.
    let reach = s.round().max(1.0) as u32;
    let lit = |y: u32| {
        (x.saturating_sub(reach)..=(x + reach).min(frame.width() - 1))
            .any(|col| frame.get_pixel(col, y).0.iter().any(|&c| c >= OUTLINE_MIN))
    };
    let (min_h, max_h) = (MIN_SLOT_HEIGHT * s, MAX_SLOT_HEIGHT * s);
    let mut runs = Vec::new();
    let mut start = None;
    for y in top..=bottom {
        match (y < bottom && lit(y), start) {
            (true, None) => start = Some(y),
            (false, Some(from)) => {
                let h = (y - from) as f32;
                if (min_h..=max_h).contains(&h) {
                    runs.push((from, y));
                }
                start = None;
            }
            _ => {}
        }
    }
    runs
}

/// What a slot's corner says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotBadge {
    /// A stack: "×60".
    Quantity(u32),
    /// A weapon or mod tier: "I" … "IV".
    Tier(u8),
}

/// The corner of `slot` holding its stack size or tier.
pub fn badge_box(slot: Rect) -> Rect {
    let x = slot.x + slot.width * 50 / 100;
    let y = slot.y + slot.height * 80 / 100;
    Rect::new(
        x,
        y,
        slot.right() - x - slot.width * 3 / 100,
        slot.height * 18 / 100,
    )
}

/// Reads `slot`'s corner: a grey numeral by its strokes, a white stack size
/// with OCR. `None` for a blank corner (a single item that isn't tiered).
pub fn read_badge(
    reader: &NameReader,
    frame: &RgbImage,
    slot: Rect,
) -> anyhow::Result<Option<SlotBadge>> {
    if let Some(tier) = slot_tier(frame, slot) {
        return Ok(Some(SlotBadge::Tier(tier)));
    }
    let area = badge_box(slot);
    if !has_white_text(frame, area) {
        return Ok(None);
    }
    // The whole corner reads better than a tight box around the digits.
    let read = reader.read_free_text(frame, area)?;
    Ok(read
        .as_deref()
        .and_then(parse_quantity)
        .or_else(|| lone_one(frame, area).then_some(1))
        .map(SlotBadge::Quantity))
}

/// "×1": OCR tends to drop a lone "1" after the "×". Two white clusters,
/// the second a narrow upright bar, read as 1.
fn lone_one(frame: &RgbImage, area: Rect) -> bool {
    let column_rows = |x: u32| {
        (area.y..area.bottom().min(frame.height()))
            .filter(|&y| is_white(frame.get_pixel(x, y).0))
            .count() as u32
    };
    let mut clusters: Vec<(u32, u32, u32)> = Vec::new(); // first x, last x, tallest column
    for x in area.x..area.right().min(frame.width()) {
        let rows = column_rows(x);
        if rows == 0 {
            continue;
        }
        match clusters.last_mut() {
            Some(c) if c.1 + 2 >= x => {
                c.1 = x;
                c.2 = c.2.max(rows);
            }
            _ => clusters.push((x, x, rows)),
        }
    }
    let [_, (first, last, tallest)] = clusters[..] else {
        return false;
    };
    let width = last - first + 1;
    tallest * 10 >= area.height * 5 && width * 3 <= tallest
}

/// "×60", "x60" or "60" → 60. OCR misreads the "×" now and then ("?50",
/// "*1"), so up to two characters before the digits are dropped.
pub fn parse_quantity(text: &str) -> Option<u32> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits = &text[start..];
    (text[..start].chars().count() <= 2 && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// Stack sizes are near-white; tier numerals are a mid grey.
fn is_white([r, g, b]: [u8; 3]) -> bool {
    r.min(g).min(b) >= 190
}

fn is_numeral_grey([r, g, b]: [u8; 3]) -> bool {
    let (min, max) = (r.min(g).min(b), r.max(g).max(b));
    (95..190).contains(&min) && max - min <= 40
}

/// Whether `area` holds white text (a stack size), not just a highlight.
fn has_white_text(frame: &RgbImage, area: Rect) -> bool {
    // Stray highlights are a few pixels; "×5" is ~16 px tall at 1440p.
    ink_box(frame, area, is_white)
        .is_some_and(|ink| ink.width >= 3 && ink.height >= area.height / 3)
}

fn ink_box(frame: &RgbImage, area: Rect, ink: impl Fn([u8; 3]) -> bool) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in area.y..area.bottom().min(frame.height()) {
        for x in area.x..area.right().min(frame.width()) {
            if ink(frame.get_pixel(x, y).0) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0 <= x1).then(|| Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// The tier numeral in `slot`'s corner, by counting its full-height stems;
/// a "V" (no stem, ink spread over a wide diagonal) after one stem makes
/// "IV". No OCR needed.
pub fn slot_tier(frame: &RgbImage, slot: Rect) -> Option<u8> {
    let area = badge_box(slot);
    // A stack size is white; its anti-aliased edges would pass for grey.
    if has_white_text(frame, area) {
        return None;
    }
    let glyph = numeral_box(frame, area)?;
    let h = glyph.height;
    let ink = |x: u32, y: u32| x < glyph.right() && is_numeral_grey(frame.get_pixel(x, y).0);
    // A column counts a row if it or its right neighbour is inked there:
    // a 2–3 px stem with a gap in one column stays one stem.
    let coverage = |x: u32| {
        (glyph.y..glyph.bottom())
            .filter(|&y| ink(x, y) || ink(x + 1, y))
            .count() as u32
    };
    // Stems: runs of columns inked over most of the glyph's height (the
    // serifs joining "II" only cover its top and bottom rows).
    let mut stems: Vec<(u32, u32)> = Vec::new();
    for x in glyph.x..glyph.right() {
        if coverage(x) * 10 < h * 7 {
            continue;
        }
        match stems.last_mut() {
            Some(stem) if stem.1 + 1 == x => stem.1 = x,
            _ => stems.push((x, x)),
        }
    }
    let first = *stems.first()?;
    // "IV": right of the first stem, the V is wide near the top and narrow
    // near the bottom, where its diagonals meet (9 vs 6 px at 1440p; "II"
    // has 2–3 px there, and "III"'s stems keep their spacing).
    let span = |from: u32, to: u32| {
        let xs: Vec<u32> = (first.1 + 2..glyph.right())
            .filter(|&x| (glyph.y + h * from / 100..glyph.y + h * to / 100).any(|y| ink(x, y)))
            .collect();
        xs.last().zip(xs.first()).map_or(0, |(r, l)| r - l + 1)
    };
    let (top, bottom) = (span(20, 45), span(60, 85));
    if top * 10 >= h * 3 && bottom * 4 <= top * 3 {
        return Some(4);
    }
    match stems.len() {
        n @ 1..=3 => Some(n as u8),
        _ => None,
    }
}

/// The numeral: the rightmost cluster of grey columns at least ~half the
/// bar tall. Weapons also show a grey attachment glyph further left, and
/// the numeral is often followed by a small "▸".
fn numeral_box(frame: &RgbImage, area: Rect) -> Option<Rect> {
    let grey_rows = |x: u32| -> Option<(u32, u32)> {
        let rows: Vec<u32> = (area.y..area.bottom().min(frame.height()))
            .filter(|&y| is_numeral_grey(frame.get_pixel(x, y).0))
            .collect();
        Some((*rows.first()?, *rows.last()?))
    };
    let gap = (area.height / 6).max(2);
    let mut clusters: Vec<Rect> = Vec::new();
    let mut current: Option<(u32, u32, u32, u32)> = None; // x0, x1, y0, y1
    let mut blank = 0;
    for x in area.x..area.right().min(frame.width()) {
        if let Some((top, bottom)) = grey_rows(x) {
            blank = 0;
            current = Some(match current {
                Some((x0, _, y0, y1)) => (x0, x, y0.min(top), y1.max(bottom)),
                None => (x, x, top, bottom),
            });
        } else {
            blank += 1;
            if blank > gap
                && let Some((x0, x1, y0, y1)) = current.take()
            {
                clusters.push(Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1));
            }
        }
    }
    if let Some((x0, x1, y0, y1)) = current {
        clusters.push(Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1));
    }
    clusters
        .into_iter()
        .rev()
        .find(|c| c.height >= area.height * 2 / 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stack_sizes() {
        assert_eq!(parse_quantity("×60"), Some(60));
        assert_eq!(parse_quantity("x100"), Some(100));
        assert_eq!(parse_quantity(" 5 "), Some(5));
        assert_eq!(parse_quantity("?50"), Some(50));
        assert_eq!(parse_quantity("Price 50"), None);
        assert_eq!(parse_quantity("×"), None);
        assert_eq!(parse_quantity("II"), None);
        assert_eq!(parse_quantity("×6O"), None);
    }

    #[test]
    fn badge_box_is_inside_the_slot() {
        let slot = Rect::new(203, 367, 122, 120);
        let badge = badge_box(slot);
        assert!(badge.x > slot.x && badge.right() < slot.right());
        assert!(badge.y > slot.y && badge.bottom() <= slot.bottom());
    }
}
