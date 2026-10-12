//! The game's named loot zones and what each holds (`data/loot-zones.json`):
//! the loot-type icons the in-game map draws under a zone's name
//! ("Railyard": Industrial, Mechanical), transcribed from screenshots. An
//! item's "found in" names the same kinds, so this says where to look.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// How much loot a zone holds, as its hover card says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LootValue {
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LootZone {
    /// As the game writes it; same text as the zone's map label.
    pub name: String,
    /// Loot kinds, in the game's legend names ("Industrial").
    pub kinds: Vec<String>,
    #[serde(default)]
    pub value: Option<LootValue>,
}

#[derive(Deserialize)]
struct File {
    maps: BTreeMap<String, Vec<LootZone>>,
}

fn all() -> &'static BTreeMap<String, Vec<LootZone>> {
    static ZONES: OnceLock<BTreeMap<String, Vec<LootZone>>> = OnceLock::new();
    ZONES.get_or_init(|| {
        // A unit test keeps the file valid.
        serde_json::from_str::<File>(include_str!("../data/loot-zones.json"))
            .map(|f| f.maps)
            .unwrap_or_default()
    })
}

/// The zones of `map` (MetaForge id), as listed.
pub fn zones(map: &str) -> &'static [LootZone] {
    all().get(map).map_or(&[], Vec::as_slice)
}

/// Zones holding any of `kinds` (an item's "found in"), as `(map, zone)`,
/// on every map, high-value zones first.
pub fn zones_with(kinds: &[String]) -> Vec<(&'static str, &'static LootZone)> {
    let wanted = |kind: &String| kinds.iter().any(|k| k.eq_ignore_ascii_case(kind));
    let mut out: Vec<(&'static str, &'static LootZone)> = all()
        .iter()
        .flat_map(|(map, zones)| zones.iter().map(move |z| (map.as_str(), z)))
        .filter(|(_, zone)| zone.kinds.iter().any(wanted))
        .collect();
    out.sort_by(|a, b| b.1.value.cmp(&a.1.value).then(a.0.cmp(b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's loot kinds (its map legends; RaidTheory's foundIn).
    const KINDS: [&str; 13] = [
        "ARC",
        "Commercial",
        "Electrical",
        "Exodus",
        "Industrial",
        "Mechanical",
        "Medical",
        "Nature",
        "Old World",
        "Raider",
        "Residential",
        "Security",
        "Technological",
    ];

    #[test]
    fn the_file_parses_with_known_maps_and_kinds() {
        assert!(zones("dam").len() >= 8);
        assert!(zones("pendola-pass").len() >= 9);
        for (map, list) in all() {
            assert!(
                crate::metaforge::MAPS.iter().any(|(id, _)| id == map),
                "{map}"
            );
            for zone in list {
                assert!(!zone.kinds.is_empty(), "{map}: {}", zone.name);
                for kind in &zone.kinds {
                    assert!(KINDS.contains(&kind.as_str()), "{map}: {kind}");
                }
            }
        }
    }

    #[test]
    fn zone_names_are_map_labels_where_the_map_has_labels() {
        for (map, list) in all() {
            let labels = crate::labels::map_labels(map);
            if labels.is_empty() {
                continue;
            }
            for zone in list {
                assert!(
                    labels
                        .iter()
                        .any(|l| l.label.as_deref() == Some(&zone.name)),
                    "{map}: {} has no label",
                    zone.name
                );
            }
        }
    }

    #[test]
    fn finds_zones_by_kind_best_first() {
        let found = zones_with(&["industrial".to_owned()]);
        let names: Vec<&str> = found.iter().map(|(_, z)| z.name.as_str()).collect();
        assert!(names.contains(&"Railyard"));
        assert!(names.contains(&"Hydroponic Dome Complex"));
        assert!(!names.contains(&"Old Town"));
        assert_eq!(found[0].1.value, Some(LootValue::High));
        assert_eq!(zones_with(&[]), []);
    }
}
