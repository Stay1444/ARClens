//! Stash grid, slot tiers and stack sizes, and icon matching, on frames
//! from the maintainer's 2026-10-05 stash recording
//! (`tests/fixtures/stash/`, labels in `labels.tsv`).
//!
//! The labels come from the item tooltips in a second recording, hovering
//! each slot; the frames here have nothing hovered.
//!
//! Icon matching needs the dataset's item images:
//! `ARCLENS_RAIDTHEORY_DIR=/path/to/arcraiders-data cargo test -p
//! arclens-vision --release --test stash -- --ignored`. Stack sizes need
//! `ARCLENS_OCR_MODEL` too.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test helpers: a missing fixture should fail loudly; pixel maths on small frames"
)]

use arclens_vision::{
    IconIndex, NameReader, Rect, SlotBadge, read_badge, slot_features, slot_tier, stash_slots,
};
use image::RgbImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stash")
}

/// (frame, slot top-left, item id)
fn labels() -> Vec<(String, (u32, u32), String)> {
    std::fs::read_to_string(fixtures().join("labels.tsv"))
        .expect("labels")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (
                f[0].to_owned(),
                (f[1].parse().unwrap(), f[2].parse().unwrap()),
                f[3].to_owned(),
            )
        })
        .collect()
}

fn frame(name: &str) -> RgbImage {
    image::open(fixtures().join(name))
        .expect("frame")
        .into_rgb8()
}

/// The detected slot at a labelled position (within a few pixels).
fn slot_at(slots: &[Rect], (x, y): (u32, u32)) -> Option<Rect> {
    slots
        .iter()
        .copied()
        .find(|s| s.x.abs_diff(x) <= 3 && s.y.abs_diff(y) <= 3)
}

#[test]
fn finds_every_labelled_slot() {
    let mut frames: HashMap<String, Vec<Rect>> = HashMap::new();
    for (name, at, item) in labels() {
        let slots = frames
            .entry(name.clone())
            .or_insert_with(|| stash_slots(&frame(&name)));
        assert!(
            slot_at(slots, at).is_some(),
            "{name} {at:?} ({item}): {slots:?}"
        );
    }
    for slots in frames.values() {
        // Five to seven full rows of four are visible.
        assert!((20..=28).contains(&slots.len()), "{} slots", slots.len());
        assert!(slots.iter().all(|s| (115..=130).contains(&s.height)));
    }
}

#[test]
fn no_stash_grid_elsewhere() {
    let frames = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");
    for name in [
        "raid_medium_ammo",
        "raid_none_1",
        "trader_none",
        "workshop_overview",
        "projects_overview",
    ] {
        let f = image::open(frames.join(format!("{name}.jpg")))
            .unwrap()
            .into_rgb8();
        assert!(stash_slots(&f).is_empty(), "{name}");
    }
}

#[test]
fn same_slots_at_lower_resolutions() {
    let full = frame("scroll_0.jpg");
    let expected = stash_slots(&full);
    for (w, h) in [(1920, 1080), (1280, 720)] {
        let small = image::imageops::resize(&full, w, h, image::imageops::FilterType::Triangle);
        let slots = stash_slots(&small);
        let s = h as f32 / 1440.0;
        assert_eq!(slots.len(), expected.len(), "{w}x{h}");
        for (a, b) in slots.iter().zip(&expected) {
            let (bx, by) = ((b.x as f32 * s) as u32, (b.y as f32 * s) as u32);
            assert!(
                a.x.abs_diff(bx) <= 2 && a.y.abs_diff(by) <= 2,
                "{w}x{h}: {a:?} vs {b:?}"
            );
        }
    }
}

/// The tier a weapon id ends in (`osprey_ii` → 2).
fn id_tier(id: &str) -> Option<u8> {
    match id.rsplit('_').next()? {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        _ => None,
    }
}

#[test]
fn reads_weapon_tiers() {
    let mut checked = 0;
    for (name, at, item) in labels() {
        let f = frame(&name);
        let slot = slot_at(&stash_slots(&f), at).unwrap();
        let tier = slot_tier(&f, slot);
        assert_eq!(tier, id_tier(&item), "{item} at {name} {at:?}");
        checked += usize::from(tier.is_some());
    }
    assert!(checked >= 6, "only {checked} tiered slots");
}

#[test]
fn reads_tier_iv() {
    // `inventory_stash`: the stash's top-left 4×6 slots, a Renegade IV at
    // row 3, column 4.
    let f = image::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames/inventory_stash.jpg"),
    )
    .unwrap()
    .into_rgb8();
    let slots = stash_slots(&f);
    let tiers: Vec<Option<u8>> = slots[8..12].iter().map(|s| slot_tier(&f, *s)).collect();
    assert_eq!(tiers, [Some(1), Some(2), Some(1), Some(4)]);
}

/// Stack sizes as printed on the labelled slots.
const QUANTITIES: &[(&str, u32)] = &[
    ("energy_clip", 5),
    ("launcher_ammo", 24),
    ("light_ammo", 100),
    ("medium_ammo", 80),
    ("battery", 13),
    ("chemicals", 50),
    ("assorted_seeds", 92),
    ("bandage", 1),
];

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL"]
fn reads_stack_sizes() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").expect("ARCLENS_OCR_MODEL");
    let reader = NameReader::from_model_file(Path::new(&model)).unwrap();
    for (name, at, item) in labels() {
        let Some(&(_, n)) = QUANTITIES.iter().find(|(id, _)| *id == item) else {
            continue;
        };
        let f = frame(&name);
        let slot = slot_at(&stash_slots(&f), at).unwrap();
        let badge = read_badge(&reader, &f, slot).unwrap();
        assert_eq!(badge, Some(SlotBadge::Quantity(n)), "{item}");
    }
}

/// An item's family: tiers and blueprints of one item share an image, so
/// the icon alone can't tell them apart (the tier numeral can).
fn family(id: &str) -> &str {
    let id = id.strip_suffix("_blueprint").unwrap_or(id);
    match id.rsplit_once('_') {
        Some((stem, "i" | "ii" | "iii" | "iv")) => stem,
        _ => id,
    }
}

#[test]
#[ignore = "needs ARCLENS_RAIDTHEORY_DIR"]
fn icon_matching_accuracy() {
    let root = std::env::var_os("ARCLENS_RAIDTHEORY_DIR").expect("ARCLENS_RAIDTHEORY_DIR");
    let images = Path::new(&root).join("images/items");
    let mut index = IconIndex::default();
    for entry in std::fs::read_dir(&images).unwrap() {
        let path = entry.unwrap().path();
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(img) = image::open(&path) {
            index.add(id.to_owned(), &img.into_rgba8());
        }
    }
    assert!(index.len() > 500, "{} icons", index.len());

    let (mut n, mut first, mut top3) = (0, 0, 0);
    for (name, at, item) in labels() {
        let f = frame(&name);
        let slot = slot_at(&stash_slots(&f), at).unwrap();
        let Some(features) = slot_features(&f, slot) else {
            eprintln!("{item}: no object found");
            n += 1;
            continue;
        };
        let ranked = index.rank(&features, 10);
        let mut families: Vec<&str> = Vec::new();
        for (id, _) in &ranked {
            if !families.contains(&family(id)) {
                families.push(family(id));
            }
        }
        n += 1;
        first += usize::from(families[0] == family(&item));
        top3 += usize::from(families.iter().take(3).any(|f| *f == family(&item)));
        if families[0] != family(&item) {
            eprintln!("{item}: {:?}", &families[..3]);
        }
    }
    let (first, top3) = (first as f32 / n as f32, top3 as f32 / n as f32);
    eprintln!("{n} slots: first {first:.2}, top 3 {top3:.2}");
    // The Python prototype: 0.81 / 0.90.
    assert!(
        first >= 0.75 && top3 >= 0.85,
        "first {first:.2}, top 3 {top3:.2}"
    );
}

#[test]
fn finds_the_hovered_slot() {
    use arclens_vision::{PanelParams, find_panels, hovered_slot};
    let frames = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames");
    // (fixture, hovered slot's top-left)
    for (name, at) in [
        ("stash_heavy_ammo", (342, 922)),
        ("stash_looting_mk2", (342, 367)),
    ] {
        let f = image::open(frames.join(format!("{name}.jpg")))
            .unwrap()
            .into_rgb8();
        let slots = stash_slots(&f);
        let panel = find_panels(&f, &PanelParams::default()).first().copied();
        let hovered = hovered_slot(&f, &slots, panel).expect(name);
        assert!(slot_at(&[hovered], at).is_some(), "{name}: {hovered:?}");
    }
    // Nothing hovered.
    let f = frame("scroll_0.jpg");
    assert_eq!(hovered_slot(&f, &stash_slots(&f), None), None);
}

#[test]
fn the_same_slot_looks_the_same_in_another_frame() {
    use arclens_vision::{slot_thumb, thumb_distance};
    // The top of the stash in both recordings (one hovering a slot further
    // down): the first slot holds the same Combat Mk. 2.
    let a = frame("scroll_0.jpg");
    let b = image::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frames/stash_heavy_ammo.jpg"),
    )
    .unwrap()
    .into_rgb8();
    let (sa, sb) = (stash_slots(&a), stash_slots(&b));
    let same = thumb_distance(&slot_thumb(&a, sa[0]), &slot_thumb(&b, sb[0]));
    assert!(same < arclens_vision::SAME_SLOT, "same {same}");
    // Different items (Combat Mk. 2 and Looting Mk. 2) are not.
    let other = thumb_distance(&slot_thumb(&a, sa[0]), &slot_thumb(&a, sa[1]));
    assert!(other > arclens_vision::SAME_SLOT * 1.5, "other {other}");
}

#[test]
fn empty_slots() {
    use arclens_vision::slot_is_empty;
    // The stash ends partway down scroll_4: its last rows are empty.
    let f = frame("scroll_4.jpg");
    let slots = stash_slots(&f);
    let empty: Vec<bool> = slots.iter().map(|s| slot_is_empty(&f, *s)).collect();
    assert!(!empty[0], "first slot holds an item");
    assert!(*empty.last().unwrap(), "last slot is empty");
    // Every labelled slot holds an item.
    for (name, at, item) in labels() {
        let f = frame(&name);
        let slot = slot_at(&stash_slots(&f), at).unwrap();
        assert!(!slot_is_empty(&f, slot), "{item}");
    }
}

#[test]
#[ignore = "needs ARCLENS_OCR_MODEL"]
fn reads_the_slot_count() {
    let model = std::env::var_os("ARCLENS_OCR_MODEL").expect("ARCLENS_OCR_MODEL");
    let reader = NameReader::from_model_file(Path::new(&model)).unwrap();
    for name in ["scroll_0.jpg", "scroll_4.jpg"] {
        let count = arclens_vision::read_stash_count(&reader, &frame(name)).unwrap();
        assert_eq!(count, Some((75, 280)), "{name}");
    }
}

#[test]
fn the_scrollbar_moves_down_with_the_grid() {
    use arclens_vision::stash_scroll;
    // scroll_0 is the top of the stash; the others are further down, in
    // recording order.
    let tops: Vec<f32> = (0..5)
        .map(|i| stash_scroll(&frame(&format!("scroll_{i}.jpg"))).expect("scrollbar"))
        .collect();
    assert!((tops[0] - 362.0).abs() <= 2.0, "{tops:?}");
    assert!(tops.windows(2).all(|w| w[1] >= w[0]), "{tops:?}");
    assert!(tops[4] > tops[0] + 100.0, "{tops:?}");
}
