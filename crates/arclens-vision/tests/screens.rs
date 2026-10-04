//! Screen classification on every fixture frame (see `tests/fixtures/README.md`),
//! with per-class precision and recall, and the map-title matcher.
//!
//! Labels were assigned by looking at each frame (2026-10-04):
//!
//! - `stash_*` and `inventory_stash` show the pause menu's INVENTORY tab
//!   outlined (INVENTORY · LOGBOOK · SYSTEM; STASH + LOADOUT), with or
//!   without a hovered tooltip: [`Screen::Inventory`].
//! - `raid_*` show the in-raid backpack, which is the same INVENTORY tab
//!   outlined (INVENTORY · CRAFTING · MAP · LOGBOOK · SYSTEM; LOADOUT only,
//!   no STASH): also [`Screen::Inventory`]. Callers that need "in raid"
//!   can tell from the tooltip footer (no sell value) or the tab set.
//! - `trader_*`: a trader's TRADES tab ([`Screen::Trader`]).
//! - `projects_overview`, `project_*`: projects, which no detector
//!   covers ([`Screen::Unknown`]).

use arclens_vision::{Screen, classify, map_from_title};
use std::collections::BTreeMap;
use std::path::Path;

/// Every fixture frame (path below `tests/fixtures/`) and what it shows.
const LABELS: &[(&str, Screen)] = &[
    ("frames/inventory_stash.jpg", Screen::Inventory),
    ("frames/logbook.jpg", Screen::Logbook),
    ("frames/project_ascending_the_mountain.jpg", Screen::Unknown),
    ("frames/project_trophy_display.jpg", Screen::Unknown),
    ("frames/projects_overview.jpg", Screen::Unknown),
    (
        "frames/quest_apollo_battening_down.jpg",
        Screen::TraderQuests,
    ),
    (
        "frames/quest_apollo_shoring_up_defenses.jpg",
        Screen::TraderQuests,
    ),
    (
        "frames/quest_lance_medical_merchandise.jpg",
        Screen::TraderQuests,
    ),
    ("frames/raid_jolt_mine.jpg", Screen::Inventory),
    ("frames/raid_light_shield.jpg", Screen::Inventory),
    ("frames/raid_medium_ammo.jpg", Screen::Inventory),
    ("frames/raid_none_1.jpg", Screen::Inventory),
    ("frames/raid_none_2.jpg", Screen::Inventory),
    ("frames/raid_renegade_iv.jpg", Screen::Inventory),
    ("frames/stash_combat_mk3_aggressive.jpg", Screen::Inventory),
    ("frames/stash_energy_clip.jpg", Screen::Inventory),
    ("frames/stash_medium_ammo.jpg", Screen::Inventory),
    ("frames/stash_none_1.jpg", Screen::Inventory),
    ("frames/stash_none_2.jpg", Screen::Inventory),
    ("frames/stash_osprey_ii.jpg", Screen::Inventory),
    ("frames/stash_shield_recharger.jpg", Screen::Inventory),
    ("frames/stash_torrente_ii.jpg", Screen::Inventory),
    ("frames/trader_angled_grip_i.jpg", Screen::Trader),
    ("frames/trader_extended_medium_mag_i.jpg", Screen::Trader),
    ("frames/trader_hairpin_i.jpg", Screen::Trader),
    ("frames/trader_none.jpg", Screen::Trader),
    ("frames/workshop_overview.jpg", Screen::Workshop),
    ("map/buried_city_mid.jpg", Screen::Map),
    ("map/dam_poi_hydroponic_dome.jpg", Screen::Map),
    ("map/dam_poi_power_generation.jpg", Screen::Map),
    ("map/dam_zoom_in.jpg", Screen::Map),
    ("map/dam_zoom_mid.jpg", Screen::Map),
    ("map/dam_zoomed_out.jpg", Screen::Map),
];

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

#[test]
fn every_fixture_frame_is_labelled() {
    let mut found = Vec::new();
    for sub in ["frames", "map"] {
        for entry in std::fs::read_dir(fixtures().join(sub)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "jpg") {
                let name = path.file_name().unwrap().to_string_lossy();
                found.push(format!("{sub}/{name}"));
            }
        }
    }
    found.sort();
    let labelled: Vec<String> = LABELS.iter().map(|(path, _)| (*path).to_owned()).collect();
    assert_eq!(found, labelled, "add new fixtures to LABELS");
}

#[test]
fn classifies_every_fixture_frame() {
    // (expected, got) → count
    let mut confusion: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut wrong = Vec::new();
    for &(path, expected) in LABELS {
        let frame = image::open(fixtures().join(path)).unwrap().into_rgb8();
        let got = classify(&frame);
        *confusion
            .entry((format!("{expected:?}"), format!("{got:?}")))
            .or_default() += 1;
        if got != expected {
            wrong.push(format!("{path}: expected {expected:?}, got {got:?}"));
        }
    }

    let classes = [
        Screen::MainMenu,
        Screen::Map,
        Screen::Inventory,
        Screen::Workshop,
        Screen::Trader,
        Screen::TraderQuests,
        Screen::Logbook,
        Screen::Unknown,
    ];
    eprintln!("expected -> got: count");
    for ((expected, got), count) in &confusion {
        eprintln!("  {expected:>12} -> {got:<12} {count}");
    }
    eprintln!("class         precision  recall  (n)");
    for class in classes {
        let name = format!("{class:?}");
        let count = |want: Option<&str>, got: Option<&str>| -> u32 {
            confusion
                .iter()
                .filter(|((e, g), _)| want.is_none_or(|w| w == e) && got.is_none_or(|w| w == g))
                .map(|(_, n)| n)
                .sum()
        };
        let hits = count(Some(&name), Some(&name));
        let (predicted, actual) = (count(None, Some(&name)), count(Some(&name), None));
        let ratio = |n: u32, d: u32| {
            if d == 0 {
                1.0
            } else {
                f64::from(n) / f64::from(d)
            }
        };
        let (precision, recall) = (ratio(hits, predicted), ratio(hits, actual));
        eprintln!("  {name:<12} {precision:>9.2} {recall:>7.2}  ({actual})");
        assert!(
            (precision - 1.0).abs() < f64::EPSILON && (recall - 1.0).abs() < f64::EPSILON,
            "{name}: precision {precision:.2}, recall {recall:.2}"
        );
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn matches_map_titles() {
    let cases = [
        ("DAM BATTLEGROUNDS - 18:55", Some("dam")),
        ("DAM BATTLEGR0UNDS — 9:02", Some("dam")),
        ("DAM BATTLEGROUNDS-18:55", Some("dam")),
        ("dam battlegrounds 19:02", Some("dam")),
        ("DAM", Some("dam")),
        ("THE BLUE GATE — 12:01", Some("blue-gate")),
        ("BLUE GATE", Some("blue-gate")),
        ("THE BLUE 6ATE - 3:10", Some("blue-gate")),
        ("THE SPACEPORT - 12:00", Some("spaceport")),
        ("THE SPACEP0RT – 0:59", Some("spaceport")),
        ("BURIED CITY 26:03", Some("buried-city")),
        ("BURIED CITY -25:57", Some("buried-city")),
        ("Buried Clty", Some("buried-city")),
        ("STELLA MONTIS - 1:00", Some("stella-montis")),
        ("STELLA M0NTIS", Some("stella-montis")),
        ("RIVEN TIDES — 30:00", Some("riven-tides")),
        ("R1VEN TIDES", Some("riven-tides")),
    ];
    for (title, expected) in cases {
        assert_eq!(map_from_title(title), expected, "{title}");
    }
}

#[test]
fn rejects_non_map_titles() {
    for title in [
        "",
        "18:55",
        "— 12:01",
        "THE",
        "Stay1444",
        "Matriarch",
        "DAN",
        "INVENTORY",
        "QUESTS",
    ] {
        assert_eq!(map_from_title(title), None, "{title}");
    }
}
