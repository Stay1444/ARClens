//! Prints the map labels found in frames and writes `<out>/<name>.png` with
//! their boxes drawn.
//!
//! `cargo run --release -p arclens-vision --example map_labels -- OUT FRAME...`

use arclens_vision::{LabelParams, find_map_labels};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    for path in args {
        let mut frame = image::open(&path)?.to_rgb8();
        let start = std::time::Instant::now();
        let labels = find_map_labels(&frame, &LabelParams::default());
        println!("{path}: {} labels in {:?}", labels.len(), start.elapsed());
        for r in &labels {
            println!("  [{}, {}, {}, {}]", r.x, r.y, r.width, r.height);
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
