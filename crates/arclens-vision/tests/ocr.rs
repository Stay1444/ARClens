//! End-to-end name reading on the fixture frames.
//!
//! Needs the ocrs recognition model, which is not committed (≈ 10 MB,
//! downloaded at runtime by the app):
//!
//! ```sh
//! curl -o /tmp/text-recognition.rten \
//!   https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten
//! ARCLENS_OCR_MODEL=/tmp/text-recognition.rten \
//!   cargo test -p arclens-vision --release -- --ignored
//! ```

use arclens_vision::{NameReader, PanelParams, find_panels, name_lines};
use std::path::{Path, PathBuf};

const CASES: &[(&str, &[&str])] = &[
    ("raid_jolt_mine", &["JOLT MINE"]),
    ("raid_light_shield", &["LIGHT SHIELD"]),
    ("raid_medium_ammo", &["MEDIUM AMMO"]),
    ("raid_renegade_iv", &["RENEGADE IV"]),
    ("raid_none_1", &[]),
    (
        "stash_combat_mk3_aggressive",
        &["COMBAT MK. 3 (AGGRESSIVE)"],
    ),
    ("stash_energy_clip", &["ENERGY CLIP"]),
    ("stash_medium_ammo", &["MEDIUM AMMO"]),
    // "II" vs "I" is decided by stroke counting, not the OCR model.
    ("stash_osprey_ii", &["OSPREY II"]),
    ("stash_shield_recharger", &["SHIELD RECHARGER"]),
    ("stash_torrente_ii", &["TORRENTE II"]),
    ("stash_none_1", &[]),
    ("trader_none", &["x25 LIGHT AMMO"]),
    ("trader_angled_grip_i", &["x25 LIGHT AMMO", "ANGLED GRIP I"]),
    (
        "trader_extended_medium_mag_i",
        &["x25 LIGHT AMMO", "EXTENDED MEDIUM MAG I"],
    ),
    ("trader_hairpin_i", &["x25 LIGHT AMMO", "HAIRPIN I"]),
];

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL (see module docs)"]
fn reads_every_fixture_name_exactly() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.expect("ARCLENS_OCR_MODEL must point at text-recognition.rten");
    let reader = NameReader::from_model_file(&model).expect("model loads");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");

    let mut failures = Vec::new();
    for (fixture, expected) in CASES {
        let frame = image::open(dir.join(format!("{fixture}.jpg")))
            .expect("fixture")
            .into_rgb8();
        let mut got: Vec<String> = find_panels(&frame, &PanelParams::default())
            .into_iter()
            .filter_map(|panel| {
                reader
                    .read(&frame, &name_lines(&frame, panel))
                    .expect("ocr")
            })
            .collect();
        got.sort();
        let mut want: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        want.sort();
        if got != want {
            failures.push(format!("{fixture}: want {want:?}, got {got:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// (fixture, expected footer: `Some(value)` in the menu, `None` in raid)
const FOOTERS: &[(&str, Option<u32>)] = &[
    ("raid_jolt_mine", None),
    ("raid_light_shield", None),
    ("raid_medium_ammo", None),
    ("raid_renegade_iv", None),
    ("stash_combat_mk3_aggressive", Some(2_000)),
    ("stash_energy_clip", Some(1_000)),
    ("stash_medium_ammo", Some(480)),
    ("stash_osprey_ii", Some(18_431)),
    ("stash_shield_recharger", Some(2_080)),
    ("stash_torrente_ii", Some(23_569)),
    ("trader_hairpin_i", Some(450)),
];

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL (see module docs)"]
fn reads_footer_value_and_raid_context() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.expect("ARCLENS_OCR_MODEL must point at text-recognition.rten");
    let mut analyzer =
        arclens_vision::Analyzer::new(NameReader::from_model_file(&model).expect("model loads"));
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");

    let mut failures = Vec::new();
    for (fixture, expected) in FOOTERS {
        let frame = image::open(dir.join(format!("{fixture}.jpg")))
            .expect("fixture")
            .into_rgb8();
        let footer = analyzer
            .analyze(&frame)
            .expect("analysis")
            .and_then(|hover| hover.footer);
        let ok = match (footer, expected) {
            (Some(f), None) => f.in_raid && f.sell_value.is_none(),
            (Some(f), Some(v)) => !f.in_raid && f.sell_value == Some(*v),
            (None, _) => false,
        };
        if !ok {
            failures.push(format!("{fixture}: want {expected:?}, got {footer:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Map labels found by colour and read by the recognition model alone.
#[test]
#[ignore = "needs ARCLENS_OCR_MODEL"]
fn reads_map_labels() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").map(PathBuf::from);
    let model = model.expect("ARCLENS_OCR_MODEL must point at text-recognition.rten");
    let reader = NameReader::from_model_file(&model).expect("model loads");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/map/dam_zoom_mid.jpg");
    let frame = image::open(path).unwrap().to_rgb8();
    let start = std::time::Instant::now();
    let read: Vec<String> =
        arclens_vision::find_map_labels(&frame, &arclens_vision::LabelParams::default())
            .into_iter()
            .filter_map(|r| reader.read_free_text(&frame, r).unwrap())
            .collect();
    eprintln!("{read:?} in {:?}", start.elapsed());
    for want in [
        "Pattern House",
        "Generator Hall",
        "Raider Outpost East",
        "Power Generation Complex",
        "Controlled Access Zone",
        "East Broken Bridge",
        "Pipeline Tower",
        "The Breach",
    ] {
        assert!(
            read.iter().any(|got| got.trim().eq_ignore_ascii_case(want)),
            "{want} not read: {read:?}"
        );
    }
}
