//! The game's own mouse cursor, found in the frame.
//!
//! ARC Raiders hides the system pointer (it stays pinned at the window's
//! centre) and draws its own: a plain white arrow, tip at the top left,
//! about 19 × 27 px at 1440p. So the pointer the screen-capture portal
//! reports is useless on the map; the arrow in the frame is where the
//! player points (verified 2026-10-04 on Dam, Buried City and Blue Gate
//! recordings: found in 256 of the 261 map frames, no false hits).
//!
//! The arrow is told apart by shape and colour: a one- or two-pixel tip
//! with nothing white above or to its left, rows that widen by about a
//! pixel each with a straight left edge, and neutral white (the game's
//! cream "Industrial" icon has a straight left edge too, but starts four
//! pixels wide and is warm).

use image::RgbImage;

/// Near-white: every channel at least this.
const WHITE: u8 = 215;
/// Rows below the tip checked at 1440p (the arrow's straight part).
const ROWS: f32 = 14.0;
/// How much warmer than neutral (red minus blue, averaged) the arrow may be.
const MAX_WARMTH: i32 = 10;

fn white(frame: &RgbImage, x: u32, y: u32) -> bool {
    frame.get_pixel(x, y).0.iter().all(|&c| c >= WHITE)
}

/// Length of the white run starting at (x, y), going right (capped).
fn run(frame: &RgbImage, x: u32, y: u32, cap: u32) -> u32 {
    (x..frame.width().min(x + cap))
        .take_while(|&xx| white(frame, xx, y))
        .count()
        .try_into()
        .unwrap_or(cap)
}

/// The cursor's tip (its hotspot) in frame pixels, if the game's arrow is
/// in `frame`.
pub fn find_cursor(frame: &RgbImage) -> Option<(u32, u32)> {
    let (w, h) = frame.dimensions();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "small row counts"
    )]
    let rows = ((ROWS * h as f32 / 1440.0).round() as u32).max(6);
    for y in 1..h.saturating_sub(rows + 2) {
        for x in 3..w.saturating_sub(4) {
            if white(frame, x, y)
                && !white(frame, x, y - 1)
                && !white(frame, x - 2, y)
                && is_arrow(frame, x, y, rows)
            {
                return Some((x, y));
            }
        }
    }
    None
}

/// Whether an arrow's tip is at (x, y): narrow, then widening about a
/// pixel a row along a straight left edge, and neutral white.
fn is_arrow(frame: &RgbImage, x: u32, y: u32, rows: u32) -> bool {
    let widest = |yy: u32, cap: u32| (x - 1..=x + 1).map(|xx| run(frame, xx, yy, cap)).max();
    if run(frame, x, y, 4) > 2 || widest(y + 1, 5).unwrap_or(0) > 3 {
        return false;
    }
    let mut warmth = 0;
    for i in 2..=rows {
        let yy = y + i;
        #[allow(clippy::cast_precision_loss, reason = "small numbers")]
        let (i_f, run_f) = (i as f32, widest(yy, rows * 2 + 4).unwrap_or(0) as f32);
        if run_f < i_f * 0.6 - 1.0 || run_f > i_f * 1.3 + 2.0 || white(frame, x - 2, yy) {
            return false;
        }
        let [r, _, b] = frame.get_pixel(x + 1, yy).0;
        warmth += i32::from(r) - i32::from(b);
    }
    warmth / i32::try_from(rows - 1).unwrap_or(1) < MAX_WARMTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    /// A white arrow (tip at `at`) on a dark frame.
    fn arrow(at: (u32, u32), color: [u8; 3]) -> RgbImage {
        let mut frame = RgbImage::from_pixel(200, 200, Rgb([30, 34, 40]));
        for i in 0..18 {
            for dx in 0..=i {
                frame.put_pixel(at.0 + dx, at.1 + i, Rgb(color));
            }
        }
        frame
    }

    #[test]
    fn finds_the_tip_of_a_white_arrow() {
        assert_eq!(
            find_cursor(&arrow((60, 40), [240, 240, 242])),
            Some((60, 40))
        );
    }

    #[test]
    fn ignores_warm_shapes_and_blocks() {
        // The same shape in the icons' cream.
        assert_eq!(find_cursor(&arrow((60, 40), [247, 232, 215])), None);
        // A white square: wide from its first row.
        let mut frame = RgbImage::from_pixel(200, 200, Rgb([30, 34, 40]));
        for y in 40..60 {
            for x in 60..80 {
                frame.put_pixel(x, y, Rgb([250, 250, 250]));
            }
        }
        assert_eq!(find_cursor(&frame), None);
    }
}
