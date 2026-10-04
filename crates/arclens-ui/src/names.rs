//! Game names in the interface language: maps, conditions, regions,
//! marker kinds, item types and rarities. Each is a message keyed by the
//! data's id (`map-dam`, `condition-night-raid`, `marker-weapon-case`); an
//! id without a message is shown as the data has it.

use arclens_core::{Marker, Rarity, humanize};
use arclens_i18n::{slug, try_tr};
use std::collections::BTreeMap;
use std::sync::RwLock;

/// Names from the game data, by [`slug`] of their id (quests: `a-bad-feeling`
/// → "Un mal presentimiento"), in the interface language. The app fills it
/// from its catalogue and passes it on to the overlay.
static DATA_NAMES: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// Replaces the names from the game data ([`data_names`]).
pub fn set_data_names(names: BTreeMap<String, String>) {
    if let Ok(mut current) = DATA_NAMES.write() {
        *current = names;
    }
}

/// The names [`set_data_names`] last set.
pub fn data_names() -> BTreeMap<String, String> {
    DATA_NAMES.read().map(|n| n.clone()).unwrap_or_default()
}

fn data_name(id: &str) -> Option<String> {
    DATA_NAMES.read().ok()?.get(&slug(id)).cloned()
}

fn lookup(prefix: &str, id: &str) -> Option<String> {
    try_tr(&format!("{prefix}-{}", slug(id)))
}

/// A map, by MetaForge id (`blue-gate`) or schedule name (`Blue Gate`).
pub fn map(id_or_name: &str) -> String {
    lookup("map", id_or_name).unwrap_or_else(|| id_or_name.to_owned())
}

/// A map condition / event (`Night Raid`).
pub fn condition(name: &str) -> String {
    lookup("condition", name).unwrap_or_else(|| name.to_owned())
}

/// A server region, by id (`north-america`).
pub fn region(id: &str) -> String {
    lookup("region", id).unwrap_or_else(|| humanize(id))
}

/// A marker category or subcategory (`containers`, `weapon_case`; a
/// quest marker's subcategory is the quest's id).
pub fn marker_kind(id: &str) -> String {
    lookup("marker", id)
        .or_else(|| data_name(id))
        .unwrap_or_else(|| humanize(id))
}

/// What to call a marker: its name, else its kind.
pub fn marker_title(marker: &Marker) -> String {
    match (&marker.label, &marker.subcategory) {
        (Some(label), _) if !label.trim().is_empty() => label.trim().to_owned(),
        (_, Some(sub)) => marker_kind(sub),
        _ => marker_kind(&marker.category),
    }
}

/// An item type as RaidTheory names it (`Topside Material`).
pub fn item_type(name: &str) -> String {
    lookup("item-type", name).unwrap_or_else(|| name.to_owned())
}

pub fn rarity(rarity: Rarity) -> String {
    let id = match rarity {
        Rarity::Common => "common",
        Rarity::Uncommon => "uncommon",
        Rarity::Rare => "rare",
        Rarity::Epic => "epic",
        Rarity::Legendary => "legendary",
    };
    lookup("rarity", id).unwrap_or_else(|| humanize(id))
}
