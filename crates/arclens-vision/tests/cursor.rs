//! The game's cursor on real map captures (2560 × 1440), and the same
//! frames at 1080p.

use arclens_vision::find_cursor;
use image::imageops::FilterType;
use std::path::Path;

/// Map fixtures and where the arrow's tip is (located by eye).
const TIPS: [(&str, (u32, u32)); 6] = [
    ("buried_city_mid", (1533, 883)),
    ("dam_poi_hydroponic_dome", (1303, 549)),
    ("dam_poi_power_generation", (1470, 580)),
    ("dam_zoom_in", (1550, 946)),
    ("dam_zoom_mid", (1372, 1244)),
    ("dam_zoomed_out", (1402, 1032)),
];

fn load(name: &str) -> image::RgbImage {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/map")
        .join(format!("{name}.jpg"));
    image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .to_rgb8()
}

fn near(a: (u32, u32), b: (u32, u32), tolerance: u32) -> bool {
    a.0.abs_diff(b.0) <= tolerance && a.1.abs_diff(b.1) <= tolerance
}

#[test]
fn finds_the_arrow_on_every_map_capture() {
    for (name, tip) in TIPS {
        let found = find_cursor(&load(name));
        assert!(
            found.is_some_and(|at| near(at, tip, 2)),
            "{name}: {found:?}"
        );
    }
}

#[test]
fn finds_it_at_1080p_too() {
    for (name, tip) in TIPS {
        let frame = load(name);
        let small = image::imageops::resize(&frame, 1920, 1080, FilterType::Lanczos3);
        let found = find_cursor(&small);
        let want = (tip.0 * 3 / 4, tip.1 * 3 / 4);
        assert!(
            found.is_some_and(|at| near(at, want, 3)),
            "{name}: {found:?} vs {want:?}"
        );
    }
}
