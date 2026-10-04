//! Quest screens on the fixture frames: which frames show one, where the
//! titles are, and (with the OCR model) what they say.
//!
//! Expected boxes were checked by eye on crops (2026-10-04). The OCR test
//! needs the ocrs recognition model (see `tests/ocr.rs`):
//!
//! ```sh
//! ARCLENS_OCR_MODEL=/tmp/text-recognition.rten \
//!   cargo test -p arclens-vision --release --test quests -- --ignored
//! ```

#![allow(
    clippy::expect_used,
    reason = "test helpers: a missing fixture should fail loudly"
)]

use arclens_vision::{
    NameReader, QuestScreen, Rect, quest_screen, quest_title_boxes, read_active_quests,
};
use image::RgbImage;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn open(path: &Path) -> RgbImage {
    image::open(path).expect("fixture").into_rgb8()
}

/// (fixture, screen, expected title boxes `[x, y, w, h]`, titles)
type Case = (
    &'static str,
    QuestScreen,
    &'static [[u32; 4]],
    &'static [&'static str],
);

const CASES: &[Case] = &[
    (
        "quest_apollo_battening_down",
        QuestScreen::TraderQuests,
        &[[1146, 574, 282, 23]],
        &["BATTENING DOWN"],
    ),
    (
        "quest_apollo_shoring_up_defenses",
        QuestScreen::TraderQuests,
        &[[1146, 574, 342, 23]],
        &["SHORING UP DEFENSES"],
    ),
    // Lance's name is shorter, so his tabs sit further left than Apollo's.
    (
        "quest_lance_medical_merchandise",
        QuestScreen::TraderQuests,
        &[[1146, 574, 374, 23]],
        &["MEDICAL MERCHANDISE"],
    ),
    (
        "logbook",
        QuestScreen::Logbook,
        &[
            [361, 303, 253, 18],
            [361, 728, 240, 18],
            [361, 1050, 298, 18],
        ],
        &["A FIRST FOOTHOLD", "BATTENING DOWN", "SHORING UP DEFENSES"],
    ),
];

/// Every frame fixture, including the map screens.
fn all_fixtures() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for sub in ["frames", "map"] {
        for entry in std::fs::read_dir(fixtures().join(sub)).expect("fixture dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|ext| ext == "jpg") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[test]
fn detects_quest_screens_and_nothing_else() {
    let paths = all_fixtures();
    assert!(paths.len() > 30, "fixtures found: {}", paths.len());
    let mut failures = Vec::new();
    for path in paths {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let want = CASES.iter().find(|case| case.0 == stem).map(|case| case.1);
        let got = quest_screen(&open(&path));
        if got != want {
            failures.push(format!("{stem}: want {want:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn finds_the_title_lines() {
    for (fixture, screen, boxes, _) in CASES {
        let frame = open(&fixtures().join(format!("frames/{fixture}.jpg")));
        let got = quest_title_boxes(&frame, *screen);
        let want: Vec<Rect> = boxes
            .iter()
            .map(|&[x, y, w, h]| Rect::new(x, y, w, h))
            .collect();
        // A couple of pixels of slack for JPEG noise at the glyph edges.
        let close = got.len() == want.len()
            && got.iter().zip(&want).all(|(g, w)| {
                g.x.abs_diff(w.x) <= 3
                    && g.y.abs_diff(w.y) <= 3
                    && g.right().abs_diff(w.right()) <= 3
                    && g.bottom().abs_diff(w.bottom()) <= 3
            });
        assert!(close, "{fixture}: want {want:?}, got {got:?}");
    }
}

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL (see module docs)"]
fn reads_the_active_quests() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.expect("ARCLENS_OCR_MODEL must point at text-recognition.rten");
    let reader = NameReader::from_model_file(&model).expect("model loads");
    let mut failures = Vec::new();
    for (fixture, _, _, titles) in CASES {
        let frame = open(&fixtures().join(format!("frames/{fixture}.jpg")));
        let got = read_active_quests(&reader, &frame).expect("ocr");
        if got != *titles {
            failures.push(format!("{fixture}: want {titles:?}, got {got:?}"));
        }
    }
    // Not a quest screen: nothing read.
    let stash = open(&fixtures().join("frames/inventory_stash.jpg"));
    let got = read_active_quests(&reader, &stash).expect("ocr");
    if !got.is_empty() {
        failures.push(format!("inventory_stash: want [], got {got:?}"));
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
