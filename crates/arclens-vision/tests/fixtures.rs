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

/// The map screen is told apart from inventory, raid and trader screens,
/// and the quest panel is seen when open (it is on every map fixture).
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
            if expected {
                assert!(
                    arclens_vision::quest_panel_open(&frame),
                    "{}",
                    path.display()
                );
            }
        }
    }
}

/// Map labels on the mid-zoom Dam frame: every place name is found as one
/// box, and nothing comes from the quest panel on the left.
#[test]
fn finds_map_labels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/map/dam_zoom_mid.jpg");
    let frame = image::open(path).unwrap().to_rgb8();
    let labels = arclens_vision::find_map_labels(&frame, &arclens_vision::LabelParams::default());
    // Checked by eye (2560×1440).
    let expected: &[(&str, [u32; 4])] = &[
        ("Pattern House", [777, 277, 152, 18]),
        ("Generator Hall", [788, 477, 151, 18]),
        ("Raider Outpost East", [1142, 503, 213, 23]),
        ("Power Generation Complex", [862, 561, 292, 23]),
        ("Controlled Access Zone", [699, 645, 255, 18]),
        ("East Broken Bridge", [1314, 686, 207, 23]),
        ("Pipeline Tower", [882, 724, 158, 23]),
        ("The Breach", [707, 815, 120, 18]),
    ];
    for (name, [x, y, w, h]) in expected {
        let want = Rect::new(*x, *y, *w, *h);
        assert!(
            labels.iter().any(|r| iou(*r, want) >= 0.8),
            "{name} not found in {labels:?}"
        );
    }
    let quest_panel_right = frame.width() * 21 / 100;
    assert!(labels.iter().all(|r| r.x > quest_panel_right), "{labels:?}");
}

/// The hovered item's side, checked by eye on each frame: the card must go
/// on the other side of the tooltip so it doesn't cover the item.
#[test]
fn finds_the_hovered_item_side() {
    use arclens_vision::Side::{Left, Right};
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");
    for (name, want) in [
        ("raid_jolt_mine", Right),
        ("raid_light_shield", Left),
        ("raid_medium_ammo", Right),
        ("stash_combat_mk3_aggressive", Left),
        ("stash_energy_clip", Left),
        ("stash_medium_ammo", Right),
        ("stash_osprey_ii", Left),
        ("stash_shield_recharger", Right),
        ("stash_torrente_ii", Left),
    ] {
        let frame = image::open(dir.join(format!("{name}.jpg")))
            .unwrap()
            .to_rgb8();
        let panel = find_panels(&frame, &PanelParams::default())[0];
        assert_eq!(arclens_vision::item_side(&frame, panel), want, "{name}");
    }
}
