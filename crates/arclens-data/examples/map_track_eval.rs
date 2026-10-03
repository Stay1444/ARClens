//! Scores `MotionTracker` against ground truth from `map_truth`.
//!
//! `cargo run --release -p arclens-data --example map_track_eval -- TRUTH.csv FRAME_DIR [STEP] [OCR_EVERY]`
//!
//! Frames are taken every `STEP` frames of the directory (30 fps clips:
//! 1 = 30 fps, 3 = 10 fps). Prints, per step, the error of the tracked
//! frame-to-frame motion against the truth's, and the drift of the
//! accumulated motion when the view is only re-anchored (as by a label
//! read) every `OCR_EVERY` tracked frames.

#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::type_complexity,
    reason = "offline evaluation tool"
)]

use arclens_vision::{Footprint, Motion, MotionTracker};

/// With `FOOTPRINT=orange`: the recording shows ARClens's own orange
/// marker badges; find them by colour, standing in for the footprint the
/// app knows exactly.
fn drawn_badges(frame: &image::RgbImage) -> Footprint {
    let mut footprint = Footprint::default();
    if std::env::var("FOOTPRINT").as_deref() != Ok("orange") {
        return footprint;
    }
    for y in (0..frame.height()).step_by(6) {
        for x in (0..frame.width()).step_by(6) {
            let [r, g, b] = frame.get_pixel(x, y).0;
            if r > 200 && (130..220).contains(&g) && b < 110 {
                #[allow(clippy::cast_precision_loss, reason = "pixels")]
                footprint.circles.push((x as f32, y as f32, 8.0));
            }
        }
    }
    footprint
}
use std::collections::HashMap;

/// Map → frame-pixel view: `p = scale · m + (tx, ty)`.
#[derive(Debug, Clone, Copy)]
struct View {
    scale: f32,
    tx: f32,
    ty: f32,
    agree: u32,
}

impl View {
    /// The motion taking this view's frame to `next`'s.
    fn to(self, next: Self) -> Motion {
        let scale = next.scale / self.scale;
        Motion {
            scale,
            dx: next.tx - scale * self.tx,
            dy: next.ty - scale * self.ty,
        }
    }
}

/// Largest disagreement of two motions over the tracked region, in pixels.
fn error(a: Motion, b: Motion, size: (f32, f32)) -> f32 {
    let mut worst: f32 = 0.0;
    for fx in [0.3, 0.5, 0.75] {
        for fy in [0.15, 0.5, 0.85] {
            let p = (fx * size.0, fy * size.1);
            let (pa, pb) = (a.apply(p), b.apply(p));
            worst = worst.max((pa.0 - pb.0).hypot(pa.1 - pb.1));
        }
    }
    worst
}

fn percentile(values: &mut [f32], p: f32) -> f32 {
    if values.is_empty() {
        return f32::NAN;
    }
    values.sort_by(f32::total_cmp);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "index"
    )]
    let i = ((values.len() - 1) as f32 * p).round() as usize;
    values[i]
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let truth_path = args.next().ok_or_else(|| anyhow::anyhow!("TRUTH.csv"))?;
    let dir = args.next().ok_or_else(|| anyhow::anyhow!("FRAME_DIR"))?;
    let step: usize = args.next().map_or(Ok(1), |s| s.parse())?;
    let ocr_every: usize = args.next().map_or(Ok(4), |s| s.parse())?;
    let min_agree: u32 = std::env::var("MIN_AGREE").map_or(Ok(5), |s| s.parse())?;

    let mut truth: HashMap<String, View> = HashMap::new();
    for line in std::fs::read_to_string(&truth_path)?.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if let [name, scale, tx, ty, agree] = f[..]
            && let (Ok(scale), Ok(tx), Ok(ty), Ok(agree)) =
                (scale.parse(), tx.parse(), ty.parse(), agree.parse())
        {
            truth.insert(
                name.to_owned(),
                View {
                    scale,
                    tx,
                    ty,
                    agree,
                },
            );
        }
    }
    let mut frames: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jpg" || e == "png"))
        .collect();
    frames.sort();
    let frames: Vec<_> = frames.into_iter().step_by(step.max(1)).collect();

    let mut tracker = MotionTracker::new();
    let mut step_errors = Vec::new();
    let mut drift_errors = Vec::new();
    let mut moving_errors = Vec::new();
    let mut times = Vec::new();
    let mut prev_view: Option<View> = None;
    // Re-anchoring: the truth view at the last anchor, and the motion since.
    let mut anchor: Option<View> = None;
    let mut since_anchor = Motion::NONE;
    let mut since_anchor_frames = 0;
    let mut size;
    let mut lost = 0;

    for path in &frames {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let frame = image::open(path)?.to_rgb8();
        #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
        {
            size = (frame.width() as f32, frame.height() as f32);
        }
        let start = std::time::Instant::now();
        let estimate = tracker.track_ignoring(&frame, &drawn_badges(&frame));
        times.push(start.elapsed().as_secs_f32() * 1000.0);
        let view = truth.get(&name).copied().filter(|v| v.agree >= min_agree);

        if let Some(e) = estimate {
            if std::env::var_os("ALL").is_some() {
                println!(
                    "{name}: conf {:.3} est s{:.4} d({:7.1},{:7.1})",
                    e.confidence, e.motion.scale, e.motion.dx, e.motion.dy
                );
            }
            since_anchor = e.motion.after(&since_anchor);
            since_anchor_frames += 1;
            if let (Some(a), Some(b)) = (prev_view, view) {
                let true_motion = a.to(b);
                let err = error(e.motion, true_motion, size);
                step_errors.push(err);
                if !true_motion.is_still(size, 2.0) {
                    moving_errors.push(err);
                }
                if std::env::var_os("VERBOSE").is_some() {
                    println!(
                        "{name}: err {err:6.1}px conf {:.3} est s{:.4} d({:7.1},{:7.1}) true s{:.4} d({:7.1},{:7.1})",
                        e.confidence,
                        e.motion.scale,
                        e.motion.dx,
                        e.motion.dy,
                        true_motion.scale,
                        true_motion.dx,
                        true_motion.dy
                    );
                }
            }
            if let (Some(a), Some(b)) = (anchor, view) {
                let drift = error(since_anchor, a.to(b), size);
                drift_errors.push(drift);
                if std::env::var_os("VERBOSE").is_some() && drift > 20.0 {
                    println!("{name}: drift {drift:.1}px over {since_anchor_frames} frames");
                }
            } else if std::env::var_os("VERBOSE").is_some() {
                println!(
                    "{name}: (no truth) conf {:.3} est s{:.4} d({:7.1},{:7.1})",
                    e.confidence, e.motion.scale, e.motion.dx, e.motion.dy
                );
            }
        } else {
            lost += 1;
        }
        // Re-anchor like a label read would.
        if (since_anchor_frames >= ocr_every || anchor.is_none())
            && let Some(v) = view
        {
            anchor = Some(v);
            since_anchor = Motion::NONE;
            since_anchor_frames = 0;
        }
        prev_view = view;
    }

    let n = step_errors.len();
    println!(
        "{} frames (step {step}), {n} scored ({} moving), {lost} without estimate",
        frames.len(),
        moving_errors.len()
    );
    for (label, values) in [
        ("step error, all", &mut step_errors),
        ("step error, moving", &mut moving_errors),
        ("drift before re-anchor", &mut drift_errors),
    ] {
        println!(
            "{label:>24}: median {:6.1}px  p90 {:6.1}px  max {:6.1}px",
            percentile(values, 0.5),
            percentile(values, 0.9),
            percentile(values, 1.0)
        );
    }
    println!(
        "{:>24}: median {:6.1}ms  max {:6.1}ms",
        "time per frame",
        percentile(&mut times, 0.5),
        percentile(&mut times, 1.0)
    );
    Ok(())
}
