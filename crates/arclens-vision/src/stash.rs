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

/// The stash's scrollbar: a 7-px light bar at x ≈ 778–784 (1440p), the
/// thumb ≥ 80 bright, between the grid's top and bottom.
const SCROLLBAR_X: [f32; 2] = [778.0, 785.0];
const SCROLL_THUMB_MIN: u8 = 80;

/// Where the scrollbar's thumb starts (pixels from the frame top): grows
/// as the grid scrolls down. `None` without a scrollbar.
pub fn stash_scroll(frame: &RgbImage) -> Option<f32> {
    let (s, offset) = scale(frame);
    let x0 = (offset.max(0.0) + SCROLLBAR_X[0] * s) as u32;
    let x1 = ((offset.max(0.0) + SCROLLBAR_X[1] * s) as u32).min(frame.width());
    let top = ((GRID_TOP + 40.0) * s) as u32;
    let bottom = ((GRID_BOTTOM + 20.0) * s) as u32;
    (top..bottom.min(frame.height()))
        .find(|&y| {
            let lit = (x0..x1)
                .filter(|&x| {
                    frame.get_pixel(x, y).0.iter().max().copied().unwrap_or(0) >= SCROLL_THUMB_MIN
                })
                .count() as u32;
            lit * 2 >= (x1 - x0).max(1)
        })
        .map(|y| y as f32)
}

/// The "75/280" under the STASH title: slots used and capacity. Measured
/// at x 233–305, y 232–255 (1440p).
const COUNT_BOX: [f32; 4] = [228.0, 228.0, 84.0, 32.0];

/// The box holding the stash's "used/capacity" count.
pub fn stash_count_box(frame: &RgbImage) -> Rect {
    let (scale, offset) = scale(frame);
    let [left, top, width, height] = COUNT_BOX;
    Rect::new(
        (offset.max(0.0) + left * scale) as u32,
        (top * scale) as u32,
        (width * scale) as u32,
        (height * scale) as u32,
    )
}

/// Reads the stash's "used/capacity" count, e.g. `(75, 280)`.
pub fn read_stash_count(
    reader: &NameReader,
    frame: &RgbImage,
) -> anyhow::Result<Option<(u32, u32)>> {
    Ok(reader
        .read_free_text(frame, stash_count_box(frame))?
        .as_deref()
        .and_then(parse_count))
}

/// "75/280" → (75, 280); used ≤ capacity. OCR sometimes reads the slash
/// as "1" ("751280"): without a slash, a "1", "l", "|" or "7" before a
/// three-digit capacity is taken for it.
pub fn parse_count(text: &str) -> Option<(u32, u32)> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let valid = |used: &str, capacity: &str| -> Option<(u32, u32)> {
        let used: u32 = used.parse().ok()?;
        let capacity: u32 = capacity.parse().ok()?;
        (used <= capacity && capacity > 0).then_some((used, capacity))
    };
    if let Some((used, capacity)) = text.split_once('/') {
        return valid(used, capacity);
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    (n >= 5 && matches!(chars[n - 4], '1' | 'l' | '|' | '7'))
        .then(|| {
            let used: String = chars[..n - 4].iter().collect();
            let capacity: String = chars[n - 3..].iter().collect();
            valid(&used, &capacity)
        })
        .flatten()
}

/// A small, fixed-size picture of a slot (icon and corner, inside the
/// outline), to tell whether two slots show the same thing: the same item
/// in the same stack across frames, or a slot seen before.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlotThumb(pub Vec<u8>);

/// [`SlotThumb`] size: 24×24 RGB.
pub const THUMB_SIDE: u32 = 24;

/// Thumbs closer than this ([`thumb_distance`]) show the same thing.
pub const SAME_SLOT: f32 = 2.0;

/// Looser: the same slot at another scroll position (JPEG noise and
/// sub-pixel shifts reach ~3.3 in a recording; other rows are ≥ 17).
pub const SAME_ROW_SLOT: f32 = 6.0;

pub fn slot_thumb(frame: &RgbImage, slot: Rect) -> SlotThumb {
    let inset = (slot.width / 30).max(1);
    let (x, y) = (slot.x + inset, slot.y + inset);
    let w = slot
        .width
        .saturating_sub(2 * inset)
        .max(1)
        .min(frame.width() - x);
    let h = slot
        .height
        .saturating_sub(2 * inset)
        .max(1)
        .min(frame.height() - y);
    let crop = image::imageops::crop_imm(frame, x, y, w, h).to_image();
    let small = image::imageops::resize(
        &crop,
        THUMB_SIDE,
        THUMB_SIDE,
        image::imageops::FilterType::Triangle,
    );
    SlotThumb(small.into_raw())
}

/// How different two thumbs look: the mean absolute channel difference
/// over the 90 % most alike pixels, so a small local change (a cursor
/// covers ~3 %) doesn't count. Measured on still frames: ≤ 0.7 for the
/// same slot, ≥ 2 for nearly all other items (identical stacks are, of
/// course, identical). See [`SAME_SLOT`].
pub fn thumb_distance(a: &SlotThumb, b: &SlotThumb) -> f32 {
    let (pa, pb) = (a.0.as_chunks::<3>().0, b.0.as_chunks::<3>().0);
    let mut diffs: Vec<u16> = pa
        .iter()
        .zip(pb)
        .map(|(p, q)| (0..3).map(|c| u16::from(p[c].abs_diff(q[c]))).sum())
        .collect();
    if diffs.is_empty() || a.0.len() != b.0.len() {
        return f32::MAX;
    }
    diffs.sort_unstable();
    let keep = diffs.len() * 9 / 10;
    diffs[..keep].iter().map(|&d| f32::from(d)).sum::<f32>() / (keep as f32 * 3.0)
}

/// Whether `slot` is empty: no item, just the dark background.
pub fn slot_is_empty(frame: &RgbImage, slot: Rect) -> bool {
    let inset = slot.width / 8;
    let area = Rect::new(
        slot.x + inset,
        slot.y + inset,
        slot.width - 2 * inset,
        slot.height * 7 / 10 - inset,
    );
    let mut bright = 0u32;
    for y in area.y..area.bottom().min(frame.height()) {
        for x in area.x..area.right().min(frame.width()) {
            bright += u32::from(frame.get_pixel(x, y).0.iter().any(|&c| c >= 90));
        }
    }
    // Items cover a good part of the slot; an empty one has at most a faint
    // placeholder glyph.
    bright * 50 < area.width * area.height
}

/// The hovered slot: the game draws a bright, coloured outline a few
/// pixels outside it. Only slots left of the tooltip `panel` are
/// considered (tooltips open to the right of the hovered slot and may
/// cover others). `None` when no slot clearly stands out.
pub fn hovered_slot(frame: &RgbImage, slots: &[Rect], panel: Option<Rect>) -> Option<Rect> {
    let ring = |slot: &Rect| {
        let (inner, outer) = ((slot.width / 40).max(1), (slot.width / 17).max(2));
        let (x0, y0) = (slot.x.saturating_sub(outer), slot.y.saturating_sub(outer));
        let (x1, y1) = (
            (slot.right() + outer).min(frame.width()),
            (slot.bottom() + outer).min(frame.height()),
        );
        let inside = |x: u32, y: u32| {
            x + inner >= slot.x
                && x < slot.right() + inner
                && y + inner >= slot.y
                && y < slot.bottom() + inner
        };
        let (mut sum, mut n) = (0u32, 0u32);
        for y in y0..y1 {
            for x in x0..x1 {
                if !inside(x, y) {
                    sum += u32::from(*frame.get_pixel(x, y).0.iter().max().unwrap_or(&0));
                    n += 1;
                }
            }
        }
        sum as f32 / n.max(1) as f32
    };
    let mut scored: Vec<(f32, Rect)> = slots
        .iter()
        .filter(|s| panel.is_none_or(|p| s.right() < p.x))
        .map(|s| (ring(s), *s))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    match scored[..] {
        [(best, slot), (second, _), ..] if best - second >= 8.0 => Some(slot),
        _ => None,
    }
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
    fn parses_stash_counts() {
        assert_eq!(parse_count("75/280"), Some((75, 280)));
        assert_eq!(parse_count(" 73 / 280 "), Some((73, 280)));
        assert_eq!(parse_count("280/75"), None);
        assert_eq!(parse_count("751280"), Some((75, 280)));
        assert_eq!(parse_count("75280"), None);
        assert_eq!(parse_count("280"), None);
    }

    #[test]
    fn thumbs_compare_by_their_most_alike_pixels() {
        let a = SlotThumb(vec![100; (THUMB_SIDE * THUMB_SIDE * 3) as usize]);
        let mut b = a.clone();
        // A small bright patch (a cursor) barely counts.
        for v in b.0.iter_mut().take(30) {
            *v = 255;
        }
        assert!(thumb_distance(&a, &b) < 1.0);
        let c = SlotThumb(vec![140; a.0.len()]);
        assert!(thumb_distance(&a, &c) > 30.0);
    }

    #[test]
    fn badge_box_is_inside_the_slot() {
        let slot = Rect::new(203, 367, 122, 120);
        let badge = badge_box(slot);
        assert!(badge.x > slot.x && badge.right() < slot.right());
        assert!(badge.y > slot.y && badge.bottom() <= slot.bottom());
    }
}
