//! The game's map place names ("Pattern House", "THE DAM") with their
//! positions, in MetaForge's marker coordinates. Used to locate the in-game
//! map view (`anchors`) and shown as names on the app's map.
//!
//! Extracted once from MetaForge's map page (they ship the game's labels in
//! their front end, not their API) with `scripts/extract-map-labels.py`;
//! see `docs/research/data-sources.md`.

use arclens_core::{MapId, MapPoint, Marker};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const LABELS_JSON: &str = include_str!("../data/map-labels.json");

#[derive(Debug, Deserialize)]
struct File {
    maps: BTreeMap<String, Vec<RawLabel>>,
}

#[derive(Debug, Deserialize)]
struct RawLabel {
    text: String,
    lat: f32,
    lng: f32,
    /// `region` (big area names), `lootZone`, `landmark`.
    #[serde(default)]
    layer: Option<String>,
    /// Which point of the text `lat`/`lng` mark: `top-left` or `center`.
    #[serde(default)]
    anchor: Option<String>,
}

/// A place name and the point of its text that `position` marks.
#[derive(Debug, Clone, PartialEq)]
pub struct MapLabel {
    pub text: String,
    pub position: MapPoint,
    /// `position` is the text's centre; otherwise its top-left corner
    /// (verified on Dam: 0-2 px against the game).
    pub centered: bool,
}

fn file() -> Option<&'static File> {
    static FILE: OnceLock<Option<File>> = OnceLock::new();
    FILE.get_or_init(|| serde_json::from_str(LABELS_JSON).ok())
        .as_ref()
}

/// The place names of `map` (MetaForge id), for locating the view.
pub fn labels_for(map: &str) -> Vec<MapLabel> {
    file()
        .and_then(|f| f.maps.get(map))
        .map(|labels| {
            labels
                .iter()
                .map(|l| MapLabel {
                    text: l.text.clone(),
                    position: MapPoint::new(l.lng, l.lat),
                    centered: l.anchor.as_deref() == Some("center"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The place names of `map` (MetaForge id) as markers in category
/// `labels`, subcategory = the label layer.
pub fn map_labels(map: &str) -> Vec<Marker> {
    file()
        .and_then(|f| f.maps.get(map))
        .map(|labels| {
            labels
                .iter()
                .enumerate()
                .map(|(i, l)| Marker {
                    id: format!("label-{map}-{i}"),
                    map: MapId::new(map),
                    category: "labels".to_owned(),
                    subcategory: l.layer.clone(),
                    position: MapPoint::new(l.lng, l.lat),
                    label: Some(l.text.clone()),
                    locked: false,
                    conditions: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_has_labels() {
        for (map, _) in crate::metaforge::MAPS {
            assert!(map_labels(map).len() >= 15, "{map}");
        }
        assert_eq!(map_labels("nowhere").len(), 0);
    }

    #[test]
    fn dam_labels_sit_where_the_game_shows_them() {
        let labels = map_labels("dam");
        let at = |name: &str| {
            labels
                .iter()
                .find(|m| m.label.as_deref() == Some(name))
                .unwrap()
                .position
        };
        // In game, Victory Ridge is above Formicai Hills and The Dam is
        // left of Red Lakes.
        assert!(at("VICTORY RIDGE").y < at("FORMICAI HILLS").y);
        assert!(at("THE DAM").x < at("RED LAKES").x);
        assert!(
            labels
                .iter()
                .any(|m| m.label.as_deref() == Some("Pattern House"))
        );
    }

    #[test]
    fn anchors_follow_the_source_layers() {
        // Dam's labels mark the text's top-left corner; Spaceport's the centre.
        assert!(labels_for("dam").iter().all(|l| !l.centered));
        assert!(labels_for("spaceport").iter().all(|l| l.centered));
    }
}
