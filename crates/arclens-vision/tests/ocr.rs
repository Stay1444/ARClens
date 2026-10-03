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
