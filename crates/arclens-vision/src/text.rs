//! Locating the item-name line inside a panel.
//!
//! Panel layout, top to bottom: a coloured type/rarity chip (white text),
//! then the **name** in bold near-black uppercase (one or two lines), then
//! the smaller description. The chip has no near-black pixels, so the first
//! band of dark "ink" rows is the name.

use crate::Rect;
use crate::panel::is_cream;
use image::RgbImage;

/// Near-black: the tooltip's text colour. Measured ≤ 25 per channel on the
/// name; grey "COMMON" chips are ~100–115, so 60 separates them cleanly.
fn is_ink([r, g, b]: [u8; 3]) -> bool {
    r.max(g).max(b) <= 60
}

/// The name line(s) of `panel`, tightly bounded. `None` if the panel holds no
/// text (or isn't really a tooltip).
pub fn name_line(frame: &RgbImage, panel: Rect) -> Option<Rect> {
    // Ignore a margin so panel borders/shadows never count as ink.
    let margin = (panel.width / 40).max(2);
    let x0 = panel.x + margin;
    let x1 = panel.right().saturating_sub(margin).min(frame.width());
    let y_end = panel.bottom().min(frame.height());
    // Text in the name is ≥ ~1 % of the panel width tall; a row needs a few
    // ink pixels to count, so specks don't start a band.
    let min_ink = ((x1 - x0) / 200).max(2);

    let row_has_ink = |y: u32| {
        (x0..x1)
            .filter(|&x| is_ink(frame.get_pixel(x, y).0))
            .count() as u32
            >= min_ink
    };

    // Chip text ("LMG · RARE") is dark too, but sits on a coloured chip; the
    // name sits on cream. Judge each band only across its own ink extent.
    let ink_extent = |(y0, y1): (u32, u32)| {
        let (mut left, mut right) = (u32::MAX, 0);
        for y in y0..y1 {
            for x in x0..x1 {
                if is_ink(frame.get_pixel(x, y).0) {
                    left = left.min(x);
                    right = right.max(x);
                }
            }
        }
        (left <= right).then_some((left, right + 1))
    };
    let on_cream = |&(y0, y1): &(u32, u32)| {
        let Some((left, right)) = ink_extent((y0, y1)) else {
            return false;
        };
        let (mut cream, mut other) = (0u32, 0u32);
        for y in y0..y1 {
            for x in left..right {
                let p = frame.get_pixel(x, y).0;
                if is_ink(p) {
                    continue;
                }
                if is_cream(p) {
                    cream += 1;
                } else {
                    other += 1;
                }
            }
        }
        cream * 10 >= (cream + other) * 7
    };
    let bands: Vec<(u32, u32)> = ink_bands(panel.y + margin..y_end, row_has_ink)
        .into_iter()
        .filter(on_cream)
        .collect();
    let first = *bands.first()?;
    let first_h = first.1 - first.0;
    let mut name = first;
    // A wrapped name continues with a band of similar height right below.
    for &(start, end) in &bands[1..] {
        let h = end - start;
        let gap = start - name.1;
        if gap * 10 <= first_h * 8 && h * 10 >= first_h * 8 {
            name.1 = end;
        } else {
            break;
        }
    }

    let (left, right) = ink_extent(name)?;
    Some(Rect::new(left, name.0, right - left, name.1 - name.0))
}

/// Runs of consecutive rows for which `has_ink` holds, as `(start, end)`
/// half-open ranges. Single blank rows inside a run are bridged
/// (anti-aliasing gaps).
fn ink_bands(rows: std::ops::Range<u32>, has_ink: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut bands: Vec<(u32, u32)> = Vec::new();
    for y in rows {
        if !has_ink(y) {
            continue;
        }
        match bands.last_mut() {
            Some(band) if y <= band.1 + 1 => band.1 = y + 1,
            _ => bands.push((y, y + 1)),
        }
    }
    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_bridge_single_row_gaps() {
        let ink = [10, 11, 13, 14, 30, 31];
        let bands = ink_bands(0..40, |y| ink.contains(&y));
        assert_eq!(bands, vec![(10, 15), (30, 32)]);
    }
}
