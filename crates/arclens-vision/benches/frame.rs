//! Per-frame CPU cost of the vision checks the app runs on captured frames.
//!
//! The app samples the screen at 4 fps, so one frame has a 250 ms budget.
//! Frames are real 2560×1440 captures from `tests/fixtures`, decoded once
//! outside the measured loops. OCR (`Analyzer::analyze`, `NameReader`) needs
//! the recognition model and is not measured here.
//!
//! Run: `cargo bench -p arclens-vision --bench frame`. Results and the
//! budget are in `docs/research/game-state-detection.md`.

use std::hint::black_box;
use std::path::Path;

use arclens_vision::{
    MotionTracker, PanelParams, find_panels, is_main_menu, is_map_screen, is_workshop_overview,
    quest_screen, station_header_box,
};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use image::RgbImage;

fn load(path: &str) -> RgbImage {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path);
    image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .to_rgb8()
}

/// `frame` shifted by `(dx, dy)` pixels, edges clamped: a map pan.
fn shifted(frame: &RgbImage, dx: u32, dy: u32) -> RgbImage {
    let (w, h) = frame.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        *frame.get_pixel((x + dx).min(w - 1), (y + dy).min(h - 1))
    })
}

/// Tooltip detection: a frame with a tooltip and one without.
fn panels(c: &mut Criterion) {
    let params = PanelParams::default();
    let mut group = c.benchmark_group("find_panels");
    for name in ["stash_osprey_ii", "raid_jolt_mine", "stash_none_1"] {
        let frame = load(&format!("frames/{name}.jpg"));
        group.bench_function(name, |b| {
            b.iter(|| find_panels(black_box(&frame), black_box(&params)));
        });
    }
    group.finish();
}

/// Screen classifiers, on a typical (negative) frame and their own screen.
fn screens(c: &mut Criterion) {
    let stash = load("frames/stash_osprey_ii.jpg");
    let map = load("map/dam_zoom_mid.jpg");
    let quest = load("frames/quest_apollo_battening_down.jpg");
    let overview = load("frames/workshop_overview.jpg");
    let project = load("frames/project_trophy_display.jpg");

    let mut group = c.benchmark_group("screens");
    group.bench_function("is_map_screen/stash", |b| {
        b.iter(|| is_map_screen(black_box(&stash)));
    });
    group.bench_function("is_map_screen/map", |b| {
        b.iter(|| is_map_screen(black_box(&map)));
    });
    group.bench_function("is_main_menu/stash", |b| {
        b.iter(|| is_main_menu(black_box(&stash)));
    });
    group.bench_function("is_workshop_overview/stash", |b| {
        b.iter(|| is_workshop_overview(black_box(&stash)));
    });
    group.bench_function("is_workshop_overview/overview", |b| {
        b.iter(|| is_workshop_overview(black_box(&overview)));
    });
    group.bench_function("quest_screen/stash", |b| {
        b.iter(|| quest_screen(black_box(&stash)));
    });
    group.bench_function("quest_screen/quest", |b| {
        b.iter(|| quest_screen(black_box(&quest)));
    });
    group.bench_function("station_header_box/stash", |b| {
        b.iter(|| station_header_box(black_box(&stash)));
    });
    group.bench_function("station_header_box/project", |b| {
        b.iter(|| station_header_box(black_box(&project)));
    });
    group.finish();
}

/// Map motion: one `track` call against a primed previous frame.
fn motion(c: &mut Criterion) {
    let mid = load("map/dam_zoom_mid.jpg");
    let zoom_in = load("map/dam_zoom_in.jpg");
    let pan = shifted(&mid, 40, 24);

    let mut group = c.benchmark_group("MotionTracker::track");
    for (name, prev, next) in [("pan", &mid, &pan), ("zoom", &mid, &zoom_in)] {
        group.bench_function(name, |b| {
            b.iter_batched_ref(
                || {
                    let mut tracker = MotionTracker::new();
                    tracker.track(prev);
                    tracker
                },
                |tracker| tracker.track(black_box(next)),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Everything cheap the app may run on one frame, in sequence.
fn all_cheap(c: &mut Criterion) {
    let params = PanelParams::default();
    let frame = load("frames/stash_osprey_ii.jpg");
    c.bench_function("all_cheap_checks/stash", |b| {
        b.iter(|| {
            let frame = black_box(&frame);
            (
                find_panels(frame, &params),
                is_map_screen(frame),
                is_main_menu(frame),
                is_workshop_overview(frame),
                quest_screen(frame),
                station_header_box(frame),
            )
        });
    });
}

criterion_group!(benches, panels, screens, motion, all_cheap);
criterion_main!(benches);
