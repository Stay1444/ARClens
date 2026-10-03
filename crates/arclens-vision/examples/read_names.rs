//! Debug helper: detect tooltips and OCR their names.
//!
//! `ARCLENS_OCR_MODEL=/path/text-recognition.rten \
//!  cargo run -p arclens-vision --release --example read_names -- frames/*.jpg`

use arclens_vision::{NameReader, PanelParams, find_panels, name_lines};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let model = std::env::var_os("ARCLENS_OCR_MODEL")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("set ARCLENS_OCR_MODEL"))?;
    let reader = NameReader::from_model_file(&model)?;
    for path in std::env::args().skip(1) {
        let frame = image::open(&path)?.into_rgb8();
        let started = std::time::Instant::now();
        let mut names = Vec::new();
        for panel in find_panels(&frame, &PanelParams::default()) {
            let lines = name_lines(&frame, panel);
            if let Some(text) = reader.read(&frame, &lines)? {
                names.push(text);
            }
        }
        println!("{path}: {names:?} ({:.1?})", started.elapsed());
    }
    Ok(())
}
