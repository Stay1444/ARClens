//! Reading the name text with `ocrs` (pure-Rust OCR).
//!
//! We already know exactly where the name lines are, so only the
//! *recognition* model runs, on a small crop. The detection model, and any
//! work on the full frame, is skipped.

use crate::Rect;
use crate::text::is_ink;
use anyhow::Context as _;
use image::RgbImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten_imageproc::{RectF, RotatedRect};
use std::path::Path;

/// Where the recognition model is published by the ocrs project.
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

/// Padding around each line crop, in pixels, so glyph edges aren't clipped.
const PAD: u32 = 6;

/// Item names only use these characters (plus `×` in trader quantities,
/// which we drop before matching anyway).
const ALLOWED: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .-'()&:/x×";

pub struct NameReader {
    engine: OcrEngine,
}

impl std::fmt::Debug for NameReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NameReader").finish_non_exhaustive()
    }
}

impl NameReader {
    /// Loads the ocrs recognition model (`text-recognition.rten`).
    pub fn from_model_file(path: &Path) -> anyhow::Result<Self> {
        let model = rten::Model::load_file(path)
            .with_context(|| format!("loading OCR model {}", path.display()))?;
        let engine = OcrEngine::new(OcrEngineParams {
            recognition_model: Some(model),
            allowed_chars: Some(ALLOWED.to_owned()),
            ..OcrEngineParams::default()
        })?;
        Ok(Self { engine })
    }

    /// Recognises the text of `lines` (from [`crate::name_lines`]) and joins
    /// them with spaces. Returns `None` if nothing legible was found.
    pub fn read(&self, frame: &RgbImage, lines: &[Rect]) -> anyhow::Result<Option<String>> {
        let mut parts = Vec::new();
        for &line in lines {
            if let Some(text) = self.read_line(frame, line)? {
                parts.push(text);
            }
        }
        let text = parts.join(" ");
        Ok((!text.trim().is_empty()).then(|| text.trim().to_owned()))
    }

    fn read_line(&self, frame: &RgbImage, line: Rect) -> anyhow::Result<Option<String>> {
        let x = line.x.saturating_sub(PAD);
        let y = line.y.saturating_sub(PAD);
        let w = (line.width + 2 * PAD).min(frame.width() - x);
        let h = (line.height + 2 * PAD).min(frame.height() - y);
        let crop = image::imageops::crop_imm(frame, x, y, w, h).to_image();

        let source = ImageSource::from_bytes(crop.as_raw(), crop.dimensions())?;
        let input = self.engine.prepare_input(source)?;
        let whole = RotatedRect::from_rect(RectF::from_tlhw(0.0, 0.0, h as f32, w as f32));
        let lines = self.engine.recognize_text(&input, &[vec![whole]])?;
        Ok(lines
            .into_iter()
            .flatten()
            .next()
            .map(|text| fix_roman_tail(&text.to_string(), trailing_i_count(frame, line))))
    }
}

/// Glyphs of a text line as `(start, end)` column ranges, split into words.
fn words(frame: &RgbImage, line: Rect) -> Vec<Vec<(u32, u32)>> {
    let column_has_ink = |x: u32| (line.y..line.bottom()).any(|y| is_ink(frame.get_pixel(x, y).0));
    let mut glyphs: Vec<(u32, u32)> = Vec::new();
    for x in line.x..line.right() {
        if !column_has_ink(x) {
            continue;
        }
        match glyphs.last_mut() {
            Some(g) if x == g.1 => g.1 = x + 1,
            _ => glyphs.push((x, x + 1)),
        }
    }
    // Measured at 23 px cap height: letter gaps 3–4 px, the gap between the
    // bars of "II" 6 px, word gaps 12 px. 0.4 × height separates them.
    let word_gap = (line.height * 4 / 10).max(3);
    let mut words: Vec<Vec<(u32, u32)>> = Vec::new();
    for glyph in glyphs {
        match words.last_mut() {
            Some(word) if glyph.0 - word[word.len() - 1].1 < word_gap => word.push(glyph),
            _ => words.push(vec![glyph]),
        }
    }
    words
}

/// If the line's last word consists only of narrow full-height bars, how
/// many: the value of a trailing Roman numeral I/II/III.
fn trailing_i_count(frame: &RgbImage, line: Rect) -> Option<usize> {
    let words = words(frame, line);
    let last = words.last()?;
    // Bold "I" measures ~0.25 of the cap height; any other letter is wider.
    let narrow = last.iter().all(|&(a, b)| (b - a) * 10 <= line.height * 4);
    (words.len() > 1 && narrow && last.len() <= 3).then_some(last.len())
}

/// The recogniser's CTC decoding merges repeated characters ("II" → "I") and
/// can drop the space before a lone "I" ("GRIP I" → "GRIPI"). When the image
/// shows a trailing word of `n` bars, rewrite the text's tail as `n` I's.
fn fix_roman_tail(text: &str, trailing_bars: Option<usize>) -> String {
    let text = text.trim();
    let Some(n) = trailing_bars else {
        return text.to_owned();
    };
    if !text.ends_with('I') {
        return text.to_owned();
    }
    let head = text.trim_end_matches('I').trim_end();
    if head.is_empty() {
        return text.to_owned();
    }
    format!("{head} {}", "I".repeat(n))
}

#[cfg(test)]
mod tests {
    use super::fix_roman_tail;

    #[test]
    fn restores_merged_roman_numerals() {
        assert_eq!(fix_roman_tail("OSPREY I", Some(2)), "OSPREY II");
        assert_eq!(fix_roman_tail("ANGLED GRIPI", Some(1)), "ANGLED GRIP I");
        assert_eq!(fix_roman_tail("HAIRPIN I", Some(1)), "HAIRPIN I");
        assert_eq!(fix_roman_tail("RENEGADE IV", None), "RENEGADE IV");
        assert_eq!(fix_roman_tail("JOLT MINE", None), "JOLT MINE");
        // Text not ending in I is left alone even if the image disagrees.
        assert_eq!(fix_roman_tail("LIGHT AMMO", Some(1)), "LIGHT AMMO");
    }
}
