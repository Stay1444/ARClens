//! Which side of the tooltip the hovered item is on. The game draws the
//! tooltip right next to the item and outlines the hovered slot in white,
//! so our card should go on the *other* side.

use crate::Rect;
use crate::panel::is_cream;
use image::RgbImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Width of the strip searched beside the tooltip, as a fraction of the
/// frame width (the slot sits ~5–10 px from the panel at 1440p).
const STRIP: f32 = 0.03;

/// The hovered item's side: from the slot outline when visible, else from
/// how the game places tooltips (right of the item, unless that would run
/// off screen).
pub fn item_side(frame: &RgbImage, panel: Rect) -> Side {
    hovered_side(frame, panel).unwrap_or_else(|| placement_guess(frame.width(), panel))
}

/// The game puts the tooltip to the right of the item when it fits; so the
/// item is on the right only when a tooltip there would not have fit.
fn placement_guess(frame_width: u32, panel: Rect) -> Side {
    // Roughly one inventory slot between the tooltip and its mirror spot.
    let slot = frame_width * 6 / 100;
    if panel.right() + slot + panel.width > frame_width {
        Side::Right
    } else {
        Side::Left
    }
}

/// Near-white pixels in the strips beside `panel`: the hovered slot's
/// outline. `None` when neither side clearly has more.
pub fn hovered_side(frame: &RgbImage, panel: Rect) -> Option<Side> {
    let strip = ((STRIP * frame.width() as f32) as u32).max(4);
    let count = |x0: u32, x1: u32| -> u32 {
        let mut n = 0;
        for y in panel.y..panel.bottom().min(frame.height()) {
            for x in x0..x1.min(frame.width()) {
                let [r, g, b] = frame.get_pixel(x, y).0;
                n += u32::from(r.min(g).min(b) >= 215);
            }
        }
        n
    };
    // `find_panels` snaps the panel to its sampling grid, so a few columns
    // of the panel's own cream (which is bright enough to count) may lie
    // outside `panel`: the strips start where the cream ends.
    let cream_column = |x: u32| {
        let rows = panel.y..panel.bottom().min(frame.height());
        let total = rows.len();
        let cream = rows.filter(|&y| is_cream(frame.get_pixel(x, y).0)).count();
        cream * 2 > total
    };
    let edge_left = (0..panel.x)
        .rev()
        .take(strip as usize)
        .find(|&x| !cream_column(x))
        .map_or(0, |x| x + 1);
    let edge_right = (panel.right()..frame.width())
        .take(strip as usize)
        .find(|&x| !cream_column(x))
        .unwrap_or(panel.right());
    let left = count(edge_left.saturating_sub(strip), edge_left.saturating_sub(2));
    let right = count(edge_right + 2, edge_right + strip);
    // A slot outline is two tall bright lines: at least ~a slot's height.
    let min = panel.height / 8;
    match (left, right) {
        (l, r) if r >= min && r > 2 * l => Some(Side::Right),
        (l, r) if l >= min && l > 2 * r => Some(Side::Left),
        _ => None,
    }
}
