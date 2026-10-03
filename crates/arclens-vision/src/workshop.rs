//! The workshop: a station's page names it and its level in the top bar,
//! e.g. `GUNSMITH — LEVEL 02` (the station's current level). Reading it
//! fills in the player's workshop progress without typing.
//!
//! Region measured on screenshots of Gunsmith (2560×1440), Explosives
//! Station and Utility Station (2000×1125), 2026-10-03 field reports.

use crate::map_header::region;
use crate::{NameReader, Rect};
use image::RgbImage;

/// The top-bar strip the station title starts in, right of the back
/// button: `[x, y, width, height]` as fractions. Wide enough for the
/// longest title ("EXPLOSIVES STATION — LEVEL 01" ends at ~23.5 %); the
/// page tabs after it are cut off at the first wide gap.
const HEADER: [f32; 4] = [0.068, 0.012, 0.25, 0.036];
/// A gap wider than this (share of the frame width) ends the title: the
/// space before the CRAFTING tab is ≥ 1.7 %, gaps inside the title
/// (spaces, the dash) ≤ 0.6 %.
const TITLE_GAP: f32 = 0.011;

/// A station and its level, as the game names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationLevel {
    /// As written, e.g. "GUNSMITH".
    pub station: String,
    pub level: u32,
}

/// Parses `GUNSMITH — LEVEL 02` (OCR slips such as `O2` for `02` allowed).
pub fn parse_station_header(text: &str) -> Option<StationLevel> {
    let upper = text.to_uppercase();
    let at = upper.find("LEVEL")?;
    let station: String = upper[..at]
        .trim()
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_owned();
    let digits: String = upper[at + "LEVEL".len()..]
        .chars()
        .filter(|c| !c.is_whitespace())
        .take_while(|c| c.is_ascii_digit() || matches!(c, 'O' | 'I' | 'L'))
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect();
    let level = digits.parse().ok()?;
    (!station.is_empty() && station.chars().any(char::is_alphabetic) && level <= 10)
        .then_some(StationLevel { station, level })
}

fn is_bright(frame: &RgbImage, col: u32, row: u32) -> bool {
    frame.get_pixel(col, row).0.iter().all(|&c| c >= 190)
}

/// Where the header text is, if the frame shows one (cheap, no OCR): the
/// bright text from the strip's first bright column up to the first wide
/// gap.
pub fn station_header_box(frame: &RgbImage) -> Option<Rect> {
    let strip = region(frame, HEADER);
    let (right, bottom) = (
        strip.right().min(frame.width()),
        strip.bottom().min(frame.height()),
    );
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few pixels"
    )]
    let max_gap = ((TITLE_GAP * frame.width() as f32) as u32).max(2);
    let column_bright = |col: u32| {
        (strip.y..bottom)
            .filter(|&row| is_bright(frame, col, row))
            .count()
    };
    let start = (strip.x..right).find(|&col| column_bright(col) > 0)?;
    let mut end = start;
    let mut gap = 0;
    for col in start..right {
        if column_bright(col) > 0 {
            end = col;
            gap = 0;
        } else {
            gap += 1;
            if gap > max_gap {
                break;
            }
        }
    }
    let (mut top, mut low, mut count) = (u32::MAX, 0, 0u32);
    for col in start..=end {
        for row in strip.y..bottom {
            if is_bright(frame, col, row) {
                top = top.min(row);
                low = low.max(row);
                count += 1;
            }
        }
    }
    // Text, not a stray highlight: enough pixels, and wider than tall.
    (count >= 40 && end > start + 2 * (low - top))
        .then(|| Rect::new(start, top, end - start + 1, low - top + 1))
}

/// Reads the station header, if the frame shows a station's page.
pub fn read_station_header(
    reader: &NameReader,
    frame: &RgbImage,
) -> anyhow::Result<Option<StationLevel>> {
    let Some(rect) = station_header_box(frame) else {
        return Ok(None);
    };
    Ok(reader
        .read_free_text(frame, rect)?
        .as_deref()
        .and_then(parse_station_header))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dark top bar with white blocks standing in for words.
    fn bar(words: &[(f32, f32)]) -> RgbImage {
        let (w, h) = (2000, 1125);
        let mut frame = RgbImage::from_pixel(w, h, image::Rgb([20, 20, 24]));
        for &(from, to) in words {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            for x in (from * w as f32) as u32..(to * w as f32) as u32 {
                for y in 25..42 {
                    frame.put_pixel(x, y, image::Rgb([240, 240, 240]));
                }
            }
        }
        frame
    }

    #[test]
    fn header_box_covers_long_titles_and_stops_before_the_tabs() {
        // "EXPLOSIVES STATION — LEVEL 01" then the CRAFTING tab (as in
        // the field screenshot, 2000 px wide).
        let frame = bar(&[
            (0.0735, 0.182),
            (0.186, 0.192),
            (0.199, 0.2235),
            (0.226, 0.235),
            (0.2535, 0.329),
        ]);
        let rect = station_header_box(&frame).unwrap();
        assert_eq!(rect.x, 147);
        assert!((469..=471).contains(&rect.right()), "{rect:?}");
        // Short title, tab close behind ("UTILITY STATION — LEVEL 01").
        let frame = bar(&[(0.0735, 0.212), (0.2295, 0.305)]);
        let rect = station_header_box(&frame).unwrap();
        assert!((423..=425).contains(&rect.right()), "{rect:?}");
        assert!(station_header_box(&bar(&[])).is_none());
    }

    #[test]
    fn parses_station_headers() {
        let parse = |t| parse_station_header(t).map(|s| (s.station, s.level));
        assert_eq!(parse("GUNSMITH — LEVEL 02"), Some(("GUNSMITH".into(), 2)));
        assert_eq!(
            parse("Gear Bench - Level 1"),
            Some(("GEAR BENCH".into(), 1))
        );
        assert_eq!(
            parse("MEDICAL LAB — LEVEL O3"),
            Some(("MEDICAL LAB".into(), 3))
        );
        assert_eq!(parse("REFINER LEVEL 01"), Some(("REFINER".into(), 1)));
        assert_eq!(
            parse("EXPLOSIVES STATION — LEVEL 01"),
            Some(("EXPLOSIVES STATION".into(), 1))
        );
        assert_eq!(parse("PLAY WORKSHOP RAIDER"), None);
        assert_eq!(parse("— LEVEL 02"), None);
        assert_eq!(parse("GUNSMITH — LEVEL"), None);
    }
}
