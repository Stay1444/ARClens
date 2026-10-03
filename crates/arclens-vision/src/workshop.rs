//! The workshop: a station's page names it and its level in the top bar,
//! e.g. `GUNSMITH — LEVEL 02` (the station's current level). Reading it
//! fills in the player's workshop progress without typing.
//!
//! Region measured on one 2560×1440 screenshot (2026-10-03, field report);
//! **unverified** on other resolutions and stations until fixtures exist.

use crate::map_header::bright_text;
use crate::{NameReader, Rect};
use image::RgbImage;

/// The station title in the top bar, right of the back button and before
/// the page tabs: `[x, y, width, height]` as fractions.
const HEADER: [f32; 4] = [0.068, 0.012, 0.137, 0.036];

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

/// Where the header text is, if the frame shows one (cheap, no OCR).
pub fn station_header_box(frame: &RgbImage) -> Option<Rect> {
    bright_text(frame, HEADER)
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
        assert_eq!(parse("PLAY WORKSHOP RAIDER"), None);
        assert_eq!(parse("— LEVEL 02"), None);
        assert_eq!(parse("GUNSMITH — LEVEL"), None);
    }
}
