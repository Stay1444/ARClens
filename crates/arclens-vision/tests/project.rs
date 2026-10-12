//! Project pages: where the selected phase and the rows are on every
//! fixture frame (no OCR), and, with the recognition model, what they read.
//!
//! ```sh
//! ARCLENS_OCR_MODEL=/tmp/text-recognition.rten \
//!   cargo test -p arclens-vision --test project -- --include-ignored
//! ```

#![allow(
    clippy::expect_used,
    reason = "test helpers: a missing fixture should fail loudly"
)]

use arclens_vision::{NameReader, ProjectRow, project_layout, read_project_page};
use std::path::{Path, PathBuf};

fn frame(name: &str) -> image::RgbImage {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/frames")
        .join(format!("{name}.jpg"));
    image::open(path).expect("fixture").into_rgb8()
}

/// The selected phase and which rows are locked.
type Page = (u32, [bool; 4]);

/// Every fixture frame, and what a project page shows on it.
const PAGES: &[(&str, Option<Page>)] = &[
    ("project_trophy_display", Some((1, [false; 4]))),
    ("project_trophy_tracked", Some((1, [false; 4]))),
    ("project_trophy_locked", Some((2, [true; 4]))),
    ("project_ascending_the_mountain", Some((1, [false; 4]))),
    ("projects_overview", None),
    ("logbook", None),
    ("workshop_overview", None),
    ("quest_apollo_battening_down", None),
    ("quest_lance_medical_merchandise", None),
    ("trader_angled_grip_i", None),
    ("trader_none", None),
    ("stash_none_1", None),
    ("raid_none_1", None),
    ("inventory_stash", None),
    ("loading_tip", None),
];

#[test]
fn finds_phase_and_rows_on_project_pages_only() {
    let mut failures = Vec::new();
    for &(name, want) in PAGES {
        let got = project_layout(&frame(name)).map(|layout| {
            let locked: Vec<bool> = layout.rows.iter().map(|r| r.locked).collect();
            (layout.phase, locked)
        });
        let want = want.map(|(phase, locked)| (phase, locked.to_vec()));
        if got != want {
            failures.push(format!("{name}: want {want:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL (see module docs)"]
fn reads_titles_and_counts() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.expect("ARCLENS_OCR_MODEL must point at text-recognition.rten");
    let reader = NameReader::from_model_file(&model).expect("model loads");
    let rows = |counts: [(u32, u32); 4], locked: bool| -> Vec<ProjectRow> {
        counts
            .iter()
            .map(|&(delivered, needed)| ProjectRow {
                delivered,
                needed,
                locked,
            })
            .collect()
    };
    let cases = [
        (
            "project_trophy_display",
            "TROPHY DISPLAY",
            1,
            rows([(0, 3), (0, 15), (0, 15), (0, 5)], false),
        ),
        (
            "project_trophy_locked",
            "TROPHY DISPLAY",
            2,
            rows([(0, 10), (0, 3), (0, 20), (0, 15)], true),
        ),
        (
            "project_ascending_the_mountain",
            "ASCENDING THE MOUNTAIN",
            1,
            rows([(0, 10), (0, 6), (0, 10), (0, 2)], false),
        ),
    ];
    let mut failures = Vec::new();
    for (name, title, phase, want) in cases {
        let page = read_project_page(&reader, &frame(name)).expect("reads");
        let ok = page.as_ref().is_some_and(|p| {
            p.title.eq_ignore_ascii_case(title) && p.phase == phase && p.rows == want
        });
        if !ok {
            failures.push(format!("{name}: got {page:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
