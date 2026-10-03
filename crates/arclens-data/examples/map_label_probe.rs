//! Checks the place-name anchors on one frame against a view carried there
//! by motion tracking from a frame whose fit is trusted: prints, for each
//! label read, where its anchor lands under that view and how far that is
//! from the text's top-left corner and its centre.
//!
//! `ARCLENS_OCR_MODEL=… cargo run --release -p arclens-data --example map_label_probe -- MAP TRUSTED_FRAME PROBE_FRAME FRAMES_BETWEEN...`
//!
//! The frames are listed from the trusted frame towards the probe frame
//! (either direction in time); the trusted frame's view is fitted from its
//! labels, then moved frame by frame.

#![allow(
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "offline evaluation tool"
)]

use arclens_data::anchors::{ScreenLabel, locate_view, match_labels};
use arclens_vision::{Analyzer, Motion, MotionTracker, NameReader};

fn read(analyzer: &mut Analyzer, frame: &image::RgbImage) -> anyhow::Result<Vec<ScreenLabel>> {
    Ok(analyzer
        .read_map_labels(frame)?
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
        .collect())
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "dam".into());
    let paths: Vec<String> = args.collect();
    anyhow::ensure!(paths.len() >= 2, "need a trusted and a probe frame");
    let model = std::env::var_os("ARCLENS_OCR_MODEL")
        .ok_or_else(|| anyhow::anyhow!("set ARCLENS_OCR_MODEL"))?;
    let mut analyzer = Analyzer::new(NameReader::from_model_file(std::path::Path::new(&model))?);
    let known = arclens_data::labels::labels_for(&map);

    let first = image::open(&paths[0])?.to_rgb8();
    let size = (first.width() as f32, first.height() as f32);
    let (fit, agree) = locate_view(&read(&mut analyzer, &first)?, size, &known)
        .ok_or_else(|| anyhow::anyhow!("trusted frame not located"))?;
    println!("trusted fit: {agree} labels, scale {:.4}", fit.a * size.0);
    // Map → frame pixels.
    let mut view = (fit.a * size.0, fit.tx * size.0, fit.ty * size.1);

    let mut tracker = MotionTracker::new();
    tracker.track(&first);
    let mut last = first;
    for path in &paths[1..] {
        last = image::open(path)?.to_rgb8();
        let m = tracker.track(&last).map_or(Motion::NONE, |e| e.motion);
        view = (
            m.scale * view.0,
            m.scale * view.1 + m.dx,
            m.scale * view.2 + m.dy,
        );
    }
    println!("carried view: scale {:.4}", view.0);

    let labels = read(&mut analyzer, &last)?;
    if let Some((fit, agree)) = locate_view(&labels, size, &known) {
        println!(
            "probe's own fit: {agree} labels, scale {:.4}",
            fit.a * size.0
        );
    }
    for label in &labels {
        let pairs = match_labels(std::slice::from_ref(label), &known);
        let Some(&((mx, my), _)) = pairs.first() else {
            println!("  {:<24} (no match)", label.text);
            continue;
        };
        let (px, py) = (view.0 * mx + view.1, view.0 * my + view.2);
        let (x, y, w, h) = label.rect;
        println!(
            "  {:<24} rect {:>6.0},{:>5.0} {:>4.0}x{:<3.0} anchor→ {:>6.0},{:>5.0}  Δtop-left {:>5.0},{:>4.0}  Δcentre {:>5.0},{:>4.0}",
            label.text,
            x,
            y,
            w,
            h,
            px,
            py,
            px - x,
            py - y,
            px - (x + w / 2.0),
            py - (y + h / 2.0)
        );
    }
    Ok(())
}
