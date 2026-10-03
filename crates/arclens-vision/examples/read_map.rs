//! Experiment: full ocrs (detection + recognition) on a map screenshot,
//! printing every text line with its centre.
//!
//! `ARCLENS_OCR_DIR=/dir/with/rten cargo run -p arclens-vision --release --example read_map -- map.jpg`

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten_imageproc::BoundingRect;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from(std::env::var("ARCLENS_OCR_DIR")?);
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(rten::Model::load_file(dir.join("text-detection.rten"))?),
        recognition_model: Some(rten::Model::load_file(dir.join("text-recognition.rten"))?),
        ..OcrEngineParams::default()
    })?;
    for path in std::env::args().skip(1) {
        let img = image::open(&path)?.into_rgb8();
        let started = std::time::Instant::now();
        let input =
            engine.prepare_input(ImageSource::from_bytes(img.as_raw(), img.dimensions())?)?;
        let words = engine.detect_words(&input)?;
        let lines = engine.find_text_lines(&input, &words);
        let texts = engine.recognize_text(&input, &lines)?;
        println!("== {path} ({:.0?})", started.elapsed());
        for (line, text) in lines.iter().zip(texts) {
            let Some(text) = text else { continue };
            let rect = line
                .iter()
                .map(BoundingRect::bounding_rect)
                .reduce(|a, b| a.union(b));
            if let Some(r) = rect {
                println!(
                    "  ({:5.0},{:5.0}) {}",
                    r.left().midpoint(r.right()),
                    r.top().midpoint(r.bottom()),
                    text
                );
            }
        }
    }
    Ok(())
}
