//! Where an item turns up on a map: markers that are the item itself (a
//! plant, a pickup: MetaForge names them by kind, e.g. `mushroom`), and
//! containers in the loot areas the item drops in (its "found in", against
//! the markers' loot areas).

use crate::{Item, Marker};

/// How a marker relates to the item looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// The marker is the item (a plant to pick, a pickup).
    Pickup,
    /// A container in a loot area the item drops in.
    LootArea,
    /// A named zone whose loot kinds include the item's (its map label).
    Zone,
}

/// Matches markers against one item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemSpots {
    /// The item's id and names, squashed ([`key`]).
    names: Vec<String>,
    /// Its loot areas, squashed.
    areas: Vec<String>,
    /// Named zones holding its kinds, squashed (see [`Self::with_zones`]).
    zones: Vec<String>,
}

/// Categories whose kinds are never items.
const NOT_ITEMS: [&str; 3] = ["quests", "labels", "locations"];

/// Lowercase letters and digits only, a plural `s` dropped:
/// "Candleberries" → "candleberrie", `candleberry`'s "candleberry"; both
/// sides go through it.
fn key(text: &str) -> String {
    let mut k: String = text
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if k.len() > 3 && k.ends_with('s') {
        k.pop();
    }
    if let Some(stem) = k.strip_suffix("ie") {
        k = format!("{stem}y");
    }
    k
}

impl ItemSpots {
    pub fn new(item: &Item) -> Self {
        let mut names: Vec<String> = std::iter::once(item.id.as_str())
            .chain(item.names())
            .map(key)
            .filter(|k| !k.is_empty())
            .collect();
        names.sort();
        names.dedup();
        let mut areas: Vec<String> = item.details.found_in.iter().map(|a| key(a)).collect();
        areas.retain(|a| !a.is_empty());
        areas.sort();
        areas.dedup();
        Self {
            names,
            areas,
            zones: Vec::new(),
        }
    }

    /// Also match the map labels of these zones (ones whose loot kinds
    /// include the item's; the caller knows the map's zones).
    #[must_use]
    pub fn with_zones<'a>(mut self, zones: impl IntoIterator<Item = &'a str>) -> Self {
        self.zones = zones.into_iter().map(key).collect();
        self
    }

    /// Whether the item has any way to be found ([`Self::spot`] can match).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.areas.is_empty() && self.zones.is_empty()
    }

    /// How `marker` relates to the item, if it does.
    pub fn spot(&self, marker: &Marker) -> Option<Spot> {
        if marker.category == "labels" {
            return marker
                .label
                .as_deref()
                .filter(|label| self.zones.contains(&key(label)))
                .map(|_| Spot::Zone);
        }
        if !NOT_ITEMS.contains(&marker.category.as_str())
            && marker
                .subcategory
                .iter()
                .chain(marker.label.iter())
                .any(|kind| self.names.contains(&key(kind)))
        {
            return Some(Spot::Pickup);
        }
        (marker.category == "containers"
            && marker
                .loot_areas
                .iter()
                .any(|area| self.areas.contains(&key(area))))
        .then_some(Spot::LootArea)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemDetails, ItemId, MapId, MapPoint};

    fn item(id: &str, name: &str, found_in: &[&str]) -> Item {
        Item {
            id: ItemId::new(id),
            name: name.into(),
            aliases: Vec::new(),
            description: None,
            rarity: None,
            category: None,
            value: None,
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: None,
            details: ItemDetails {
                found_in: found_in.iter().map(|s| (*s).to_owned()).collect(),
                ..ItemDetails::default()
            },
        }
    }

    fn marker(category: &str, sub: &str, areas: &[&str]) -> Marker {
        Marker {
            id: sub.into(),
            map: MapId::new("dam"),
            category: category.into(),
            subcategory: Some(sub.into()),
            position: MapPoint::new(0.0, 0.0),
            label: None,
            locked: false,
            conditions: None,
            floors: None,
            loot_areas: areas.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn plants_are_the_item_itself() {
        let berries = ItemSpots::new(&item("candleberries", "Candleberries", &[]));
        assert_eq!(
            berries.spot(&marker("nature", "candleberry", &[])),
            Some(Spot::Pickup)
        );
        let mushroom = ItemSpots::new(&item("mushroom", "Mushroom", &["Nature"]));
        assert_eq!(
            mushroom.spot(&marker("nature", "mushroom", &[])),
            Some(Spot::Pickup)
        );
        assert_eq!(mushroom.spot(&marker("nature", "agave", &[])), None);
        // A quest named like an item is not the item.
        assert_eq!(mushroom.spot(&marker("quests", "mushroom", &[])), None);
    }

    #[test]
    fn containers_in_its_loot_areas() {
        let gear = ItemSpots::new(&item("rusted_gear", "Rusted Gear", &["Industrial"]));
        assert!(!gear.is_empty());
        assert_eq!(
            gear.spot(&marker(
                "containers",
                "locker",
                &["Residential", "industrial"]
            )),
            Some(Spot::LootArea)
        );
        assert_eq!(
            gear.spot(&marker("containers", "locker", &["Residential"])),
            None
        );
        // Only containers hold loot.
        assert_eq!(gear.spot(&marker("arc", "tick", &["Industrial"])), None);
    }

    #[test]
    fn zone_labels_when_given() {
        let mut railyard = marker("labels", "lootZone", &[]);
        railyard.label = Some("Railyard".into());
        let gear = ItemSpots::new(&item("rusted_gear", "Rusted Gear", &["Industrial"]));
        assert_eq!(gear.spot(&railyard), None);
        let gear = gear.with_zones(["Railyard"]);
        assert_eq!(gear.spot(&railyard), Some(Spot::Zone));
    }
}
