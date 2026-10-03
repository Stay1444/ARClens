//! Which menu screen the game shows, from fixed UI elements (no OCR).
//!
//! The main menu ("PLAY" tab): its big yellow PLAY button bottom right,
//! and the PLAY tab's outline top left. Regions measured on one 2560×1440
//! screenshot (2026-10-03, field report); **unverified** elsewhere until
//! fixture frames exist.

use crate::map_header::{bright_share, region};
use image::RgbImage;

/// Inside the PLAY button, right of its dark chevron: `[x, y, w, h]`.
const PLAY_BUTTON: [f32; 4] = [0.82, 0.825, 0.14, 0.045];
/// Both ends of the outlined "PLAY" tab in the top bar.
const PLAY_TAB_LEFT: [f32; 4] = [0.0415, 0.012, 0.006, 0.034];
const PLAY_TAB_RIGHT: [f32; 4] = [0.0790, 0.012, 0.006, 0.034];
/// The button is mostly yellow; its black "PLAY" text covers the rest.
const MIN_YELLOW: f32 = 0.45;
/// Bright share of a tab end when outlined (as for the map's MAP tab).
const MIN_OUTLINE: f32 = 0.03;

/// Whether `frame` shows the main menu. Cheap; run it every frame.
pub fn is_main_menu(frame: &RgbImage) -> bool {
    yellow_share(frame, PLAY_BUTTON) >= MIN_YELLOW
        && bright_share(frame, PLAY_TAB_LEFT) >= MIN_OUTLINE
        && bright_share(frame, PLAY_TAB_RIGHT) >= MIN_OUTLINE
}

/// Share of saturated yellow (the game's accent) in the region.
fn yellow_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    let area = region(frame, fraction);
    let (mut yellow, mut total) = (0u32, 0u32);
    for row in area.y..area.bottom().min(frame.height()) {
        for col in area.x..area.right().min(frame.width()) {
            let [r, g, b] = frame.get_pixel(col, row).0;
            total += 1;
            yellow += u32::from(r >= 200 && g >= 150 && b <= 100 && r - b >= 120);
        }
    }
    if total == 0 {
        0.0
    } else {
        yellow as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(frame: &mut RgbImage, fraction: [f32; 4], colour: [u8; 3]) {
        let area = region(frame, fraction);
        for row in area.y..area.bottom() {
            for col in area.x..area.right() {
                frame.put_pixel(col, row, image::Rgb(colour));
            }
        }
    }

    #[test]
    fn needs_the_play_button_and_the_outlined_tab() {
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([30, 25, 20]));
        assert!(!is_main_menu(&frame));
        paint(&mut frame, PLAY_BUTTON, [245, 197, 24]);
        assert!(!is_main_menu(&frame), "button alone");
        // A 2 px outline at each end of the tab.
        for end in [PLAY_TAB_LEFT, PLAY_TAB_RIGHT] {
            paint(
                &mut frame,
                [end[0], end[1], 0.0008, end[3]],
                [240, 240, 240],
            );
        }
        assert!(is_main_menu(&frame));
    }

    #[test]
    fn other_screens_are_not_the_main_menu() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        for sub in ["frames", "map"] {
            for entry in std::fs::read_dir(format!("{dir}/{sub}")).unwrap() {
                let path = entry.unwrap().path();
                let frame = image::open(&path).unwrap().to_rgb8();
                assert!(!is_main_menu(&frame), "{}", path.display());
            }
        }
    }
}
