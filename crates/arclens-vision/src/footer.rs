//! The tooltip footer: a darker beige bar under the body.
//!
//! * Main menu (stash, trader): two cells — weight | **sell value**.
//! * In raid: one cell — weight only.
//!
//! So the footer tells us both the item's real current value (the dataset
//! only has the base value; the game scales it, e.g. by durability) and
//! whether the player is in a raid.

use crate::Rect;
use image::RgbImage;

/// Footer bar colour, measured (203,191,174); same family as the header tab.
fn is_footer_beige([r, g, b]: [u8; 3]) -> bool {
    (185..=220).contains(&r) && (172..=208).contains(&g) && (155..=192).contains(&b) && r >= b
}

/// The footer bar directly below `panel`, if present.
pub fn footer(frame: &RgbImage, panel: Rect) -> Option<Rect> {
    let x0 = panel.x + panel.width / 20;
    let x1 = panel
        .right()
        .saturating_sub(panel.width / 20)
        .min(frame.width());
    let beige_row = |y: u32| {
        let total = (x1 - x0) / 4;
        let beige = (x0..x1)
            .step_by(4)
            .filter(|&x| is_footer_beige(frame.get_pixel(x, y).0))
            .count() as u32;
        // Text and icons cover well under half of any footer row.
        beige * 10 >= total * 5
    };
    // Up to ~6 % of the frame height below the body; allow a few px of
    // anti-aliased seam first.
    let limit = (panel.bottom() + frame.height() * 6 / 100).min(frame.height());
    let start = (panel.bottom()..panel.bottom() + 6)
        .take_while(|&y| y < limit)
        .find(|&y| beige_row(y))?;
    let end = (start..limit).find(|&y| !beige_row(y)).unwrap_or(limit);
    let height = end - start;
    // A real footer is ~4.4 % of the frame height; reject slivers.
    (height * 1000 >= frame.height() * 25).then(|| Rect::new(panel.x, start, panel.width, height))
}

/// Footer text: dark on the beige bar (≈ 200). Looser than the name's
/// near-black, so thin strokes still count in small frames (1280×720),
/// where they blur towards the bar colour.
fn is_ink([r, g, b]: [u8; 3]) -> bool {
    r.max(g).max(b) <= 120
}

/// Groups of ink inside the footer, left to right: one per cell.
pub fn footer_cells(frame: &RgbImage, footer: Rect) -> Vec<Rect> {
    // Stay clear of the panel's edges and the cell divider's anti-aliasing.
    let inset = footer.width / 25;
    // 2 px of a 1440p footer (≈ 63 px tall).
    let inset_y = (footer.height / 30).max(1);
    let footer = Rect::new(
        footer.x + inset,
        footer.y + inset_y,
        footer.width.saturating_sub(2 * inset),
        footer.height.saturating_sub(2 * inset_y),
    );
    let column_ink = |x: u32| (footer.y..footer.bottom()).any(|y| is_ink(frame.get_pixel(x, y).0));
    // Cells are far apart (half the panel); glyphs and the icon are close.
    let gap = footer.width / 10;
    let mut groups: Vec<(u32, u32)> = Vec::new();
    for x in footer.x..footer.right().min(frame.width()) {
        if !column_ink(x) {
            continue;
        }
        match groups.last_mut() {
            Some(g) if x - g.1 < gap => g.1 = x + 1,
            _ => groups.push((x, x + 1)),
        }
    }
    groups
        .into_iter()
        .filter_map(|(a, b)| {
            let rows: Vec<u32> = (footer.y..footer.bottom())
                .filter(|&y| (a..b).any(|x| is_ink(frame.get_pixel(x, y).0)))
                .collect();
            let (top, bottom) = (*rows.first()?, *rows.last()? + 1);
            Some(Rect::new(a, top, b - a, bottom - top))
        })
        .collect()
}

/// Parses a sell value like "23,569", "800" or "1.250" (locale separators),
/// ignoring anything else the recogniser produced (e.g. the coin icon).
pub fn parse_value(text: &str) -> Option<u32> {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 7 {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values_with_separators_and_noise() {
        assert_eq!(parse_value("23,569"), Some(23_569));
        assert_eq!(parse_value("@ 800"), Some(800));
        assert_eq!(parse_value("1.250"), Some(1_250));
        assert_eq!(parse_value("o"), None);
    }
}

/// What the footer says about the hovered item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterInfo {
    /// The game's current sell value (main menu only; already reflects
    /// durability etc.). `None` in raid, where the footer shows only weight.
    pub sell_value: Option<u32>,
    /// The footer has no value cell: the player is in a raid.
    pub in_raid: bool,
}

/// The full-height cells of a footer: `[weight]` in raid, `[weight, value]`
/// in the main menu. Smaller cells (stack counts like "80/80") are dropped.
pub fn value_cells(frame: &RgbImage, footer: Rect) -> Vec<Rect> {
    let cells = footer_cells(frame, footer);
    let tallest = cells.iter().map(|c| c.height).max().unwrap_or(0);
    cells
        .into_iter()
        // A pixel of slack: in small frames (720p) a cell is ~12 px tall
        // and blur moves its edges by one.
        .filter(|c| (c.height + 1) * 100 >= tallest * 85)
        .collect()
}
