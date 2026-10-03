//! Golden tests against real captures (see `tests/fixtures/README.md`).
//!
//! Expected name rectangles were checked by eye on crops of each frame. A
//! detection matches when its intersection-over-union with the expected box
//! is at least 0.8, so small threshold tweaks don't churn these numbers.

#![allow(clippy::cast_precision_loss, reason = "ratio of small pixel areas")]

use arclens_vision::{PanelParams, Rect, find_panels, name_line};
use std::path::Path;

/// An expected name box: label for humans, `[x, y, w, h]`.
type Expected = (&'static str, [u32; 4]);

/// (fixture, expected name boxes)
const CASES: &[(&str, &[Expected])] = &[
    ("raid_jolt_mine", &[("JOLT MINE", [1450, 440, 161, 23])]),
    (
        "raid_light_shield",
        &[("LIGHT SHIELD", [1322, 326, 202, 23])],
    ),
    ("raid_medium_ammo", &[("MEDIUM AMMO", [1588, 958, 249, 23])]),
    ("raid_renegade_iv", &[("RENEGADE IV", [1322, 523, 206, 23])]),
    ("raid_none_1", &[]),
    ("raid_none_2", &[]),
    // Two-line name.
    (
        "stash_combat_mk3_aggressive",
        &[("COMBAT MK. 3 (AGGRESSIVE)", [385, 159, 229, 63])],
    ),
    ("stash_energy_clip", &[("ENERGY CLIP", [663, 852, 196, 23])]),
    (
        "stash_medium_ammo",
        &[("MEDIUM AMMO", [1650, 929, 249, 23])],
    ),
    ("stash_osprey_ii", &[("OSPREY II", [801, 337, 144, 23])]),
    (
        "stash_shield_recharger",
        &[("SHIELD RECHARGER", [1650, 316, 297, 23])],
    ),
    (
        "stash_torrente_ii",
        &[("TORRENTE II", [1383, 353, 185, 23])],
    ),
    ("stash_none_1", &[]),
    ("stash_none_2", &[]),
    // Trader: the purchase panel is always there; a hover tooltip may touch
    // or overlap it.
    ("trader_none", &[("×25 LIGHT AMMO", [1906, 231, 270, 23])]),
    (
        "trader_angled_grip_i",
        &[
            ("×25 LIGHT AMMO", [1906, 231, 270, 23]),
            ("ANGLED GRIP I", [1604, 1055, 220, 23]),
        ],
    ),
    (
        "trader_extended_medium_mag_i",
        &[
            ("×25 LIGHT AMMO", [1906, 231, 270, 23]),
            ("EXTENDED MEDIUM MAG I", [1604, 782, 406, 23]),
        ],
    ),
    (
        "trader_hairpin_i",
        &[
            ("×25 LIGHT AMMO", [1906, 231, 270, 23]),
            ("HAIRPIN I", [1391, 305, 138, 23]),
        ],
    ),
];

fn iou(a: Rect, b: Rect) -> f32 {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    if x1 <= x0 || y1 <= y0 {
        return 0.0;
    }
    let inter = u64::from(x1 - x0) * u64::from(y1 - y0);
    inter as f32 / (a.area() + b.area() - inter) as f32
}

#[test]
fn finds_every_name_line_and_nothing_else() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");
    let mut failures = Vec::new();

    for (fixture, expected) in CASES {
        let frame = image::open(dir.join(format!("{fixture}.jpg")))
            .unwrap_or_else(|e| panic!("{fixture}: {e}"))
            .into_rgb8();
        let names: Vec<Rect> = find_panels(&frame, &PanelParams::default())
            .into_iter()
            .filter_map(|panel| name_line(&frame, panel))
            .collect();

        for (label, [x, y, w, h]) in *expected {
            let want = Rect::new(*x, *y, *w, *h);
            if !names.iter().any(|&got| iou(got, want) >= 0.8) {
                failures.push(format!(
                    "{fixture}: missed {label} at {want:?}; got {names:?}"
                ));
            }
        }
        if names.len() != expected.len() {
            failures.push(format!(
                "{fixture}: expected {} name lines, got {}: {names:?}",
                expected.len(),
                names.len()
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The map screen is told apart from inventory, raid and trader screens.
#[test]
fn recognises_the_map_screen() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (sub, expected) in [("map", true), ("frames", false)] {
        for entry in std::fs::read_dir(dir.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            let frame = image::open(&path).unwrap().to_rgb8();
            assert_eq!(
                arclens_vision::is_map_screen(&frame),
                expected,
                "{}",
                path.display()
            );
        }
    }
}
