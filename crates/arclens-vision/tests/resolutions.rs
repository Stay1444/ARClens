//! The cheap detectors at other 16:9 resolutions.
//!
//! Every fixture is a 2560×1440 capture. The game's UI scales with the
//! resolution, so a downscaled copy (Lanczos3, in memory) stands in for a
//! 1920×1080 or 1280×720 capture until real ones exist: each detector must
//! give the same answer on the copy as on the original, with boxes compared
//! as fractions of the frame (2026-10-04).
//!
//! This is a proxy: the game renders text natively at each size (crisper
//! than a resampled 1440p frame, and without Lanczos ringing), and other
//! aspect ratios (16:10, 21:9) move UI anchored to the screen edges, which
//! scaling cannot show.

#![allow(
    clippy::cast_precision_loss,
    reason = "pixel coordinates of < 2^16 as fractions"
)]

use arclens_vision::{
    LabelParams, PanelParams, QuestScreen, Rect, Screen, Side, classify, find_map_labels,
    find_panels, footer, is_map_screen, is_workshop_overview, item_side, name_line,
    quest_panel_open, quest_screen, quest_title_boxes, read_workshop_levels, station_header_box,
    value_cells,
};
use image::RgbImage;
use image::imageops::{FilterType, resize};
use std::fmt::Debug;
use std::path::{Path, PathBuf};

/// A box as `[x, y, w, h]` fractions of the frame.
type Norm = [f32; 4];

fn norm(frame: &RgbImage, r: Rect) -> Norm {
    let (w, h) = (frame.width() as f32, frame.height() as f32);
    [
        r.x as f32 / w,
        r.y as f32 / h,
        r.width as f32 / w,
        r.height as f32 / h,
    ]
}

/// What the cheap detectors say about one frame.
#[derive(Debug)]
struct Answers {
    screen: Screen,
    map: bool,
    quest_panel: bool,
    workshop: bool,
    levels: Vec<(usize, u32)>,
    quests: Option<QuestScreen>,
    quest_titles: Vec<Norm>,
    station_header: Option<Norm>,
    /// Cream panels, largest first. On item screens (inventory, trader)
    /// only those with a name line, smallest first, as the app's analyser
    /// takes them: the first is the hovered tooltip. Nameless bits of the
    /// trader's purchase panel beside an overlapping tooltip depend on
    /// where the sampling grid falls and are not compared.
    panels: Vec<PanelAnswers>,
    map_labels: Vec<Norm>,
}

#[derive(Debug)]
struct PanelAnswers {
    rect: Norm,
    name: Option<Norm>,
    side: Side,
    footer: Option<Norm>,
    value_cells: usize,
}

fn answers(frame: &RgbImage) -> Answers {
    let screen = classify(frame);
    let quests = quest_screen(frame);
    let map = is_map_screen(frame);
    let mut panels = find_panels(frame, &PanelParams::default());
    if is_item_screen(screen) {
        panels.retain(|&panel| name_line(frame, panel).is_some());
        panels.sort_by_key(Rect::area);
    }
    Answers {
        screen,
        map,
        quest_panel: quest_panel_open(frame),
        workshop: is_workshop_overview(frame),
        levels: read_workshop_levels(frame),
        quests,
        quest_titles: quests
            .map(|screen| quest_title_boxes(frame, screen))
            .unwrap_or_default()
            .into_iter()
            .map(|r| norm(frame, r))
            .collect(),
        station_header: station_header_box(frame).map(|r| norm(frame, r)),
        panels: panels
            .into_iter()
            .map(|panel| {
                let foot = footer(frame, panel);
                PanelAnswers {
                    rect: norm(frame, panel),
                    name: name_line(frame, panel).map(|r| norm(frame, r)),
                    side: item_side(frame, panel),
                    footer: foot.map(|r| norm(frame, r)),
                    value_cells: foot.map_or(0, |f| value_cells(frame, f).len()),
                }
            })
            .collect(),
        map_labels: if map {
            find_map_labels(frame, &LabelParams::default())
                .into_iter()
                .map(|r| norm(frame, r))
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// Screens where tooltips are hovered and their names read.
fn is_item_screen(screen: Screen) -> bool {
    matches!(screen, Screen::Inventory | Screen::Trader)
}

/// Edge tolerance for panels and text boxes: 0.6 % of the frame (≈ 15 px
/// at 1440p, 4 px at 720p). Panels are found on a sampling grid, and text
/// edges move by a resampled pixel or two.
const TOL: f32 = 0.006;

/// Whether two boxes agree within [`TOL`] on each edge.
fn close(a: Norm, b: Norm) -> bool {
    (a[0] - b[0]).abs() <= TOL
        && (a[1] - b[1]).abs() <= TOL
        && (a[0] + a[2] - b[0] - b[2]).abs() <= TOL
        && (a[1] + a[3] - b[1] - b[3]).abs() <= TOL
}

fn all_close(a: &[Norm], b: &[Norm]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&a, &b)| close(a, b))
}

fn same_box(a: Option<Norm>, b: Option<Norm>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => close(a, b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// Share of the full-size map labels that must be found again, each where
/// it was: small text loses strokes when scaled.
const MIN_LABELS_FOUND: f32 = 0.7;

/// Differences between full-size answers and those on a scaled copy, as
/// `(detector, description)`.
fn compare(full: &Answers, small: &Answers) -> Vec<(&'static str, String)> {
    let mut diffs = Vec::new();
    let mut check = |what: &'static str, same: bool, a: &dyn Debug, b: &dyn Debug| {
        if !same {
            diffs.push((what, format!("{a:?} -> {b:?}")));
        }
    };
    check(
        "classify",
        full.screen == small.screen,
        &full.screen,
        &small.screen,
    );
    check(
        "is_map_screen",
        full.map == small.map,
        &full.map,
        &small.map,
    );
    // Only meaningful on the map screen.
    check(
        "quest_panel_open",
        !full.map || full.quest_panel == small.quest_panel,
        &full.quest_panel,
        &small.quest_panel,
    );
    check(
        "is_workshop_overview",
        full.workshop == small.workshop,
        &full.workshop,
        &small.workshop,
    );
    check(
        "read_workshop_levels",
        !full.workshop || full.levels == small.levels,
        &full.levels,
        &small.levels,
    );
    check(
        "quest_screen",
        full.quests == small.quests,
        &full.quests,
        &small.quests,
    );
    check(
        "quest_title_boxes",
        all_close(&full.quest_titles, &small.quest_titles),
        &full.quest_titles,
        &small.quest_titles,
    );
    check(
        "station_header_box",
        same_box(full.station_header, small.station_header),
        &full.station_header,
        &small.station_header,
    );
    let rects = |a: &Answers| a.panels.iter().map(|p| p.rect).collect::<Vec<_>>();
    check(
        "find_panels",
        all_close(&rects(full), &rects(small)),
        &rects(full),
        &rects(small),
    );
    let items = is_item_screen(full.screen);
    for (i, (a, b)) in full.panels.iter().zip(&small.panels).enumerate() {
        // Names are read on item screens only: the cream panels of quest
        // and project pages start with a checkbox or other thin lines that
        // fade when scaled.
        check(
            "name_line",
            !items || same_box(a.name, b.name),
            &(i, a.name),
            &b.name,
        );
        // The side of the hovered item: the hovered tooltip only (the
        // purchase panel has no hovered item).
        check(
            "item_side",
            (items && i > 0) || a.side == b.side,
            &(i, a.side),
            &b.side,
        );
        check(
            "footer",
            same_box(a.footer, b.footer),
            &(i, a.footer),
            &b.footer,
        );
        check(
            "value_cells",
            a.value_cells == b.value_cells,
            &(i, a.value_cells),
            &b.value_cells,
        );
    }
    let found = full
        .map_labels
        .iter()
        .filter(|&&a| small.map_labels.iter().any(|&b| close(a, b)))
        .count();
    check(
        "find_map_labels",
        found as f32 >= MIN_LABELS_FOUND * full.map_labels.len() as f32,
        &format!("{} labels", full.map_labels.len()),
        &format!("{found} of them found ({} in all)", small.map_labels.len()),
    );
    diffs
}

/// Every fixture frame, as `(name, path)` with names like `frames/logbook`.
fn fixtures() -> Vec<(String, PathBuf)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut out = Vec::new();
    for sub in ["frames", "map"] {
        for entry in std::fs::read_dir(dir.join(sub)).unwrap_or_else(|e| panic!("{sub}: {e}")) {
            let path = entry.unwrap_or_else(|e| panic!("{sub}: {e}")).path();
            if path.extension().is_some_and(|ext| ext == "jpg") {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                out.push((format!("{sub}/{stem}"), path));
            }
        }
    }
    out.sort();
    out
}

/// Runs every fixture at `width`×`height`: `(fixture, detector, diff)`.
fn differences(width: u32, height: u32) -> Vec<(String, &'static str, String)> {
    let fixtures = fixtures();
    assert!(fixtures.len() > 30, "fixtures found: {}", fixtures.len());
    let mut out = Vec::new();
    for (name, path) in fixtures {
        let frame = image::open(&path)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .into_rgb8();
        let small = resize(&frame, width, height, FilterType::Lanczos3);
        for (detector, diff) in compare(&answers(&frame), &answers(&small)) {
            out.push((name.clone(), detector, diff));
        }
    }
    out
}

/// Fails on any difference not in `expected` (`(fixture, detector)`; a
/// fixture of `"*"` matches all), and on expected ones that no longer
/// happen, so a fixed limitation is noticed and its entry removed.
fn assert_same_answers(width: u32, height: u32, expected: &[(&str, &str)]) {
    let diffs = differences(width, height);
    let is_expected = |name: &str, detector: &str| {
        expected
            .iter()
            .any(|&(n, d)| d == detector && (n == "*" || n == name))
    };
    let mut failures: Vec<String> = diffs
        .iter()
        .filter(|(name, detector, _)| !is_expected(name, detector))
        .map(|(name, detector, diff)| format!("{name}: {detector}: {diff}"))
        .collect();
    for &(name, detector) in expected {
        let happens = diffs
            .iter()
            .any(|(n, d, _)| *d == detector && (name == "*" || n == name));
        if !happens {
            failures.push(format!("{name}: {detector} now matches; drop it"));
        }
    }
    for (name, detector, diff) in &diffs {
        if is_expected(name, detector) {
            eprintln!("{width}x{height} {name}: {detector}: {diff} (expected limitation)");
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The hovered slot's outline here is purple, not white: the side check
/// sees nothing on that side, and below 1440p the white "EQUIPMENT"
/// heading beside the tooltip outweighs it. A tallest-column test fixes
/// this frame but loses the trader's blurred outline at 720p.
const LOOTING_SIDE: (&str, &str) = ("frames/stash_looting_mk2", "item_side");

#[test]
fn same_answers_at_1080p() {
    assert_same_answers(1920, 1080, &[LOOTING_SIDE]);
}

#[test]
fn same_answers_at_720p() {
    assert_same_answers(
        1280,
        720,
        &[
            LOOTING_SIDE,
            // Map labels are ≈ 9 px tall at 720p, with 1-px strokes: few
            // pixels stay near-white after scaling (≈ 1/3 of the share at
            // 1440p), so most labels are lost. Lowering the white
            // threshold would change 1440p results and still found only
            // ≈ half of them. OCR on such small text is doubtful anyway.
            ("map/buried_city_mid", "find_map_labels"),
            ("map/dam_poi_hydroponic_dome", "find_map_labels"),
            ("map/dam_poi_power_generation", "find_map_labels"),
            ("map/dam_zoom_in", "find_map_labels"),
            ("map/dam_zoom_mid", "find_map_labels"),
            ("map/dam_zoomed_out", "find_map_labels"),
            // No station page among the fixtures: what the header box finds
            // on the overview is the WORKSHOP tab's outline, a 3-px line at
            // 1440p that falls below the brightness threshold at 720p.
            ("frames/workshop_overview", "station_header_box"),
        ],
    );
}

/// Tooltip names and footer values read by OCR (the app's `Analyzer`):
/// at 1080p they must match full size, except the known misreads below;
/// 720p is only printed. Needs the recognition model (see `tests/ocr.rs`):
///
/// ```sh
/// ARCLENS_OCR_MODEL=/tmp/text-recognition.rten \
///   cargo test -p arclens-vision --release --test resolutions -- --ignored --nocapture
/// ```
///
/// Results 2026-10-04 (debug build, Lanczos3 copies): 15 of 18 tooltips
/// read the same at 1080p (all footers do); at 720p also "SPREY II" and
/// "ORRENTE".
#[test]
#[ignore = "needs ARCLENS_OCR_MODEL"]
fn reads_tooltips_at_1080p() {
    // Misread at 1080p only: OCR on ≈ 17-px text, not detection (the
    // name boxes match full size).
    const KNOWN_MISREADS: &[&str] = &[
        "frames/raid_light_shield", // "LLIGHT SHIELD"
        "frames/stash_torrente_ii", // "TORRENTE": the numeral is dropped
        "frames/trader_none",       // ".25 LIGHT AMMO" for "x25 …"
    ];
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.unwrap_or_else(|| panic!("ARCLENS_OCR_MODEL must point at the model"));
    let mut failures = Vec::new();
    for (name, path) in fixtures() {
        let stem = name.trim_start_matches("frames/");
        if !["stash_", "raid_", "trader_"]
            .iter()
            .any(|prefix| stem.starts_with(prefix))
        {
            continue;
        }
        let frame = image::open(&path)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .into_rgb8();
        let mut read = Vec::new();
        for (width, height) in [(2560, 1440), (1920, 1080), (1280, 720)] {
            // A fresh analyser per size: its cache must not answer.
            let reader = arclens_vision::NameReader::from_model_file(&model)
                .unwrap_or_else(|e| panic!("model: {e}"));
            let mut analyzer = arclens_vision::Analyzer::new(reader);
            let small = resize(&frame, width, height, FilterType::Lanczos3);
            let hover = analyzer
                .analyze(&small)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            read.push(hover.map(|h| (h.name, h.footer)));
        }
        eprintln!("{name}: {read:?}");
        let known = KNOWN_MISREADS.contains(&name.as_str());
        if (read[0] == read[1]) == known {
            failures.push(format!(
                "{name} (known misread: {known}): {:?} at 1440p, {:?} at 1080p",
                read[0], read[1]
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
