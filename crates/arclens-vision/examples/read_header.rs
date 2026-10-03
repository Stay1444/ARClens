//! Debug helper: read the map panel header (map, time, condition).
use arclens_vision::{NameReader, read_map_header};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let reader = NameReader::from_model_file(&PathBuf::from(std::env::var("ARCLENS_OCR_MODEL")?))?;
    for path in std::env::args().skip(1) {
        let frame = image::open(&path)?.into_rgb8();
        let started = std::time::Instant::now();
        let header = read_map_header(&reader, &frame)?;
        println!("{path}: {header:?} ({:.0?})", started.elapsed());
    }
    Ok(())
}
