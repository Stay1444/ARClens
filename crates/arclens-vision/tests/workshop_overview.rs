//! The Workshop overview against real captures (see `tests/fixtures/README.md`).

use arclens_vision::{WORKSHOP_TILES, is_workshop_overview, read_workshop_levels};
use image::RgbImage;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn load(path: &Path) -> RgbImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .to_rgb8()
}

fn overview() -> RgbImage {
    load(&fixtures().join("frames/workshop_overview.jpg"))
}

#[test]
fn detects_the_workshop_overview() {
    assert!(is_workshop_overview(&overview()));
}

#[test]
fn other_screens_are_not_the_workshop_overview() {
    let mut checked = 0;
    for sub in ["frames", "map"] {
        for entry in std::fs::read_dir(fixtures().join(sub)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "jpg")
                || path
                    .file_stem()
                    .is_some_and(|stem| stem == "workshop_overview")
            {
                continue;
            }
            assert!(!is_workshop_overview(&load(&path)), "{}", path.display());
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} fixtures checked");
}

#[test]
fn reads_station_levels() {
    let levels: Vec<(&str, u32)> = read_workshop_levels(&overview())
        .into_iter()
        .map(|(tile, level)| (WORKSHOP_TILES[tile], level))
        .collect();
    assert_eq!(
        levels,
        [
            ("workbench", 1),
            ("weapon_bench", 2),
            ("equipment_bench", 1),
            ("utility_bench", 1),
            ("med_station", 1),
            ("explosives_bench", 1),
            ("refiner", 1),
        ]
    );
}
