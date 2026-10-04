//! Game names in the interface language: maps, conditions, regions,
//! marker kinds, item types and rarities. Each is a message keyed by the
//! data's id (`map-dam`, `condition-night-raid`, `marker-weapon-case`); an
//! id without a message is shown as the data has it.

use arclens_core::{Marker, Rarity, humanize};
use arclens_i18n::{slug, try_tr};

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

/// A marker category or subcategory (`containers`, `weapon_case`).
pub fn marker_kind(id: &str) -> String {
    lookup("marker", id).unwrap_or_else(|| humanize(id))
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
