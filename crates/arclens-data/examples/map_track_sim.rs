//! Replays a map recording through the app's view pipeline and measures
//! where markers would be drawn against where they belong:
//!
//! - **labels only** (before motion tracking): the view of the last label
//!   read, which arrives `LATENCY` frames after its frame was captured;
//! - **labels + tracking**: that view moved by the motion tracked since.
//!
//! Label reads use the ground truth of `map_truth` (a read's result is the
//! truth of the frame it was started on), so this isolates the timing.
//! With `OUT` set, writes each frame with the game's place-name anchors
//! drawn where the tracked view puts them (green) and where the
//! labels-only view does (red).
//!
//! `cargo run --release -p arclens-data --example map_track_sim -- MAP TRUTH.csv FRAME_DIR [STEP] [LATENCY]`

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

/// Map → frame pixels: `p = scale · m + (tx, ty)`.
#[derive(Debug, Clone, Copy)]
struct View {
    scale: f32,
    tx: f32,
    ty: f32,
}

impl View {
    fn moved(self, m: Motion) -> Self {
        Self {
            scale: m.scale * self.scale,
            tx: m.scale * self.tx + m.dx,
            ty: m.scale * self.ty + m.dy,
        }
    }

    fn apply(self, (x, y): (f32, f32)) -> (f32, f32) {
        (self.scale * x + self.tx, self.scale * y + self.ty)
    }
}

/// Largest distance between where two views put points of the viewport.
fn error(a: View, b: View, size: (f32, f32)) -> f32 {
    let mut worst: f32 = 0.0;
    for fx in [0.3, 0.5, 0.75] {
        for fy in [0.15, 0.5, 0.85] {
            // A screen point, back to the map with `b`, forward with `a`.
            let screen = (fx * size.0, fy * size.1);
            let m = ((screen.0 - b.tx) / b.scale, (screen.1 - b.ty) / b.scale);
            let (x, y) = a.apply(m);
            worst = worst.max((x - screen.0).hypot(y - screen.1));
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

fn mark(frame: &mut image::RgbImage, (x, y): (f32, f32), colour: [u8; 3]) {
    #[allow(clippy::cast_possible_truncation, reason = "pixels")]
    let (cx, cy) = (x.round() as i64, y.round() as i64);
    for r in [9i64, 10, 11] {
        for k in 0..64 {
            let t = f64::from(k) / 64.0 * std::f64::consts::TAU;
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_precision_loss,
                reason = "pixels"
            )]
            let (px, py) = (
                cx + (r as f64 * t.cos()) as i64,
                cy + (r as f64 * t.sin()) as i64,
            );
            if let (Ok(px), Ok(py)) = (u32::try_from(px), u32::try_from(py))
                && px < frame.width()
                && py < frame.height()
            {
                frame.put_pixel(px, py, image::Rgb(colour));
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let map = args.next().ok_or_else(|| anyhow::anyhow!("MAP"))?;
    let truth_path = args.next().ok_or_else(|| anyhow::anyhow!("TRUTH.csv"))?;
    let dir = args.next().ok_or_else(|| anyhow::anyhow!("FRAME_DIR"))?;
    let step: usize = args.next().map_or(Ok(1), |s| s.parse())?;
    let latency: usize = args.next().map_or(Ok(10), |s| s.parse())?;
    let min_agree: u32 = std::env::var("MIN_AGREE").map_or(Ok(5), |s| s.parse())?;
    let out = std::env::var_os("OUT").map(std::path::PathBuf::from);

    let mut truth: HashMap<String, View> = HashMap::new();
    for line in std::fs::read_to_string(&truth_path)?.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if let [name, scale, tx, ty, agree] = f[..]
            && let (Ok(scale), Ok(tx), Ok(ty), Ok(agree)) =
                (scale.parse(), tx.parse(), ty.parse(), agree.parse::<u32>())
            && agree >= min_agree
        {
            truth.insert(name.to_owned(), View { scale, tx, ty });
        }
    }
    let mut frames: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jpg" || e == "png"))
        .collect();
    frames.sort();
    let frames: Vec<_> = frames.into_iter().step_by(step.max(1)).collect();
    let anchors: Vec<(f32, f32)> = arclens_data::labels::labels_for(&map)
        .iter()
        .map(|l| (l.position.x, l.position.y))
        .collect();

    let mut tracker = MotionTracker::new();
    // A read in flight: its frame index, truth, motion from the previous
    // read's frame to it, and motion since it.
    let mut reading: Option<(usize, Option<View>, Option<Motion>, Option<Motion>)> = None;
    // The app's state: view at the last read's frame, motion since, and the
    // labels-only view (no tracking).
    let mut base: Option<View> = None;
    let mut since: Option<Motion> = None;
    let mut labels_only: Option<View> = None;
    let (mut tracked_err, mut plain_err) = (Vec::new(), Vec::new());
    let (mut tracked_moving, mut plain_moving) = (Vec::new(), Vec::new());
    let mut prev_truth: Option<View> = None;

    for (i, path) in frames.iter().enumerate() {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut frame = image::open(path)?.to_rgb8();
        #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
        let size = (frame.width() as f32, frame.height() as f32);
        let step_motion = tracker
            .track_ignoring(&frame, &drawn_badges(&frame))
            .filter(|e| e.confidence >= 0.06)
            .map(|e| e.motion);
        let compose = |m: Option<Motion>| m.zip(step_motion).map(|(m, s)| s.after(&m));
        since = compose(since);
        if let Some((_, _, _, s)) = &mut reading {
            *s = compose(*s);
        }
        // A read finishes `latency` frames after it started.
        if let Some((started, read_truth, moved, since_read)) = reading
            && i >= started + latency
        {
            base = read_truth.or_else(|| base.zip(moved).map(|(b, m)| b.moved(m)));
            labels_only = read_truth.or(labels_only);
            since = since_read;
            reading = None;
        }
        // Start the next read (the app reads whenever the view changes; the
        // worker is the bottleneck).
        if reading.is_none() {
            reading = Some((i, truth.get(&name).copied(), since, Some(Motion::NONE)));
        }

        let tracked = base.zip(since).map(|(b, m)| b.moved(m));
        let now = truth.get(&name).copied();
        let moving = now.zip(prev_truth).is_some_and(|(a, b)| {
            (a.scale / b.scale - 1.0).abs() > 0.002 || (a.tx - b.tx).hypot(a.ty - b.ty) > 2.0
        });
        if let Some(now) = now {
            if let Some(t) = tracked {
                let e = error(t, now, size);
                tracked_err.push(e);
                if moving {
                    tracked_moving.push(e);
                }
            }
            if let Some(p) = labels_only {
                let e = error(p, now, size);
                plain_err.push(e);
                if moving {
                    plain_moving.push(e);
                }
            }
        }
        prev_truth = now;

        if let Some(out) = &out {
            for &a in &anchors {
                if let Some(p) = labels_only {
                    mark(&mut frame, p.apply(a), [255, 60, 60]);
                }
                if let Some(t) = tracked {
                    mark(&mut frame, t.apply(a), [60, 255, 90]);
                }
            }
            frame.save(out.join(format!("{name}.jpg")))?;
        }
    }

    println!(
        "{} frames (step {step}, read latency {latency} frames)",
        frames.len()
    );
    for (label, values) in [
        ("labels only, all", &mut plain_err),
        ("labels only, moving", &mut plain_moving),
        ("labels + tracking, all", &mut tracked_err),
        ("labels + tracking, moving", &mut tracked_moving),
    ] {
        println!(
            "{label:>26}: median {:6.1}px  p90 {:6.1}px  max {:6.1}px  (n={})",
            percentile(values, 0.5),
            percentile(values, 0.9),
            percentile(values, 1.0),
            values.len()
        );
    }
    Ok(())
}
