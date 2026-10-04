//! Dumps the place names read on each frame, for offline analysis of the
//! label anchors: prints `frame\ttext\tx\ty\tw\th\tmatch` (one line per label
//! read; `match` is the known label it names, empty when none).
//!
//! `ARCLENS_OCR_MODEL=… cargo run --release -p arclens-data --example map_label_dump -- MAP FRAME...`

use arclens_data::anchors::{ScreenLabel, match_labels};
use arclens_vision::{Analyzer, NameReader};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "dam".into());
    let model = std::env::var_os("ARCLENS_OCR_MODEL")
        .ok_or_else(|| anyhow::anyhow!("set ARCLENS_OCR_MODEL"))?;
    let mut analyzer = Analyzer::new(NameReader::from_model_file(std::path::Path::new(&model))?);
    let known = arclens_data::labels::labels_for(&map);
    for path in args {
        let frame = image::open(&path)?.to_rgb8();
        let name = std::path::Path::new(&path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        for l in analyzer.read_map_labels(&frame)? {
            #[allow(clippy::cast_precision_loss, reason = "pixel coordinates")]
            let label = ScreenLabel {
                text: l.text,
                rect: (
                    l.rect.x as f32,
                    l.rect.y as f32,
                    l.rect.width as f32,
                    l.rect.height as f32,
                ),
            };
            // The known label it names: the one whose position matched.
            let matched = match_labels(std::slice::from_ref(&label), &known)
                .first()
                .and_then(|&(m, _)| {
                    known
                        .iter()
                        .find(|k| (k.position.x, k.position.y) == m)
                        .map(|k| k.text.clone())
                })
                .unwrap_or_default();
            let (x, y, w, h) = label.rect;
            println!("{name}\t{}\t{x}\t{y}\t{w}\t{h}\t{matched}", label.text);
        }
    }
    Ok(())
}
