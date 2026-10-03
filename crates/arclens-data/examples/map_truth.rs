//! Ground truth for map-view tracking: reads the place names on each frame
//! and fits the view, printing `frame,scale,tx,ty,agree` (map → frame
//! pixels; empty fields when the view can't be located).
//!
//! `ARCLENS_OCR_MODEL=… cargo run --release -p arclens-data --example map_truth -- MAP FRAME...`

#![allow(clippy::many_single_char_names, reason = "offline evaluation tool")]

use arclens_data::anchors::{ScreenLabel, locate_view};
use arclens_vision::{Analyzer, NameReader};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "dam".into());
    let model = std::env::var_os("ARCLENS_OCR_MODEL")
        .ok_or_else(|| anyhow::anyhow!("set ARCLENS_OCR_MODEL"))?;
    let mut analyzer = Analyzer::new(NameReader::from_model_file(std::path::Path::new(&model))?);
    let known = arclens_data::labels::labels_for(&map);
    println!("frame,scale,tx,ty,agree");
    for path in args {
        let frame = image::open(&path)?.to_rgb8();
        #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
        let size = (frame.width() as f32, frame.height() as f32);
        #[allow(clippy::cast_precision_loss, reason = "pixel coordinates")]
        let labels: Vec<ScreenLabel> = analyzer
            .read_map_labels(&frame)?
            .into_iter()
            .map(|l| ScreenLabel {
                text: l.text,
                rect: (
                    l.rect.x as f32,
                    l.rect.y as f32,
                    l.rect.width as f32,
                    l.rect.height as f32,
                ),
            })
            .collect();
        let name = std::path::Path::new(&path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if std::env::var_os("VERBOSE").is_some() {
            let fit = locate_view(&labels, size, &known).map(|(t, _)| t);
            for label in &labels {
                let pair = arclens_data::anchors::match_labels(std::slice::from_ref(label), &known);
                match (pair.first(), fit) {
                    (Some(&(m, s)), Some(t)) => {
                        let (x, y) = t.apply(m);
                        let (px, py) = (x * size.0, y * size.1);
                        eprintln!(
                            "  {:<28} screen ({:6.0},{:6.0}) fit ({px:6.0},{py:6.0}) off {:5.0}px",
                            label.text,
                            s.0,
                            s.1,
                            (px - s.0).hypot(py - s.1)
                        );
                    }
                    (Some(&(m, s)), None) => eprintln!(
                        "  {:<28} screen ({:6.0},{:6.0}) map {m:?}",
                        label.text, s.0, s.1
                    ),
                    (None, _) => eprintln!("  {:<28} (no match)", label.text),
                }
            }
        }
        match locate_view(&labels, size, &known) {
            // Back to pixels: x_px = a·x·W + tx·W.
            Some((t, agree)) => println!(
                "{name},{},{},{},{agree}",
                t.a * size.0,
                t.tx * size.0,
                t.ty * size.1
            ),
            None => println!("{name},,,,0"),
        }
    }
    Ok(())
}
