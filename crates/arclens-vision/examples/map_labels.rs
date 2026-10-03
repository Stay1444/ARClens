//! Prints the map labels found in frames and writes `<out>/<name>.png` with
//! their boxes drawn. With `ARCLENS_OCR_MODEL` set it also reads them.
//!
//! `cargo run --release -p arclens-vision --example map_labels -- OUT FRAME...`

use arclens_vision::{LabelParams, NameReader, find_map_labels};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    // With ARCLENS_OCR_MODEL set, also print what each label reads.
    let reader = std::env::var_os("ARCLENS_OCR_MODEL")
        .map(|m| NameReader::from_model_file(std::path::Path::new(&m)))
        .transpose()?;
    for path in args {
        let mut frame = image::open(&path)?.to_rgb8();
        let start = std::time::Instant::now();
        let labels = find_map_labels(&frame, &LabelParams::default());
        println!("{path}: {} labels in {:?}", labels.len(), start.elapsed());
        for r in &labels {
            let text = match &reader {
                Some(reader) => reader.read_free_text(&frame, *r)?.unwrap_or_default(),
                None => String::new(),
            };
            println!("  [{}, {}, {}, {}] {text}", r.x, r.y, r.width, r.height);
            for x in r.x..r.right() {
                frame.put_pixel(x, r.y, image::Rgb([255, 0, 0]));
                frame.put_pixel(x, r.bottom() - 1, image::Rgb([255, 0, 0]));
            }
            for y in r.y..r.bottom() {
                frame.put_pixel(r.x, y, image::Rgb([255, 0, 0]));
                frame.put_pixel(r.right() - 1, y, image::Rgb([255, 0, 0]));
            }
        }
        let name = std::path::Path::new(&path).file_stem().unwrap_or_default();
        frame.save(out.join(name).with_extension("png"))?;
    }
    Ok(())
}
