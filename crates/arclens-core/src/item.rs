//! Item domain model.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier of an item, as used by the upstream data source
/// (for example `"rusted-gear"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Item rarity tiers, ordered from least to most rare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// An amount of a given item, e.g. a recycling output or a crafting input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemQuantity {
    pub item: ItemId,
    pub quantity: u32,
}

impl Item {
    /// Its name, then its [`Item::aliases`].
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }

    /// Blueprints unlock crafting once learned (the data's `type`).
    pub fn is_blueprint(&self) -> bool {
        self.category
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("blueprint"))
    }
}

/// Everything ARClens knows about a single item.
///
/// Fields that upstream sources do not always provide are `Option`al so a
/// partial record is still useful.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    /// In the catalogue's language.
    pub name: String,
    /// Its name in the other languages ARClens speaks, for matching what
    /// the game shows (the game's language can differ from ours).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub rarity: Option<Rarity>,
    /// Free-form category as reported upstream ("Topside Material", "Weapon", ...).
    #[serde(default)]
    pub category: Option<String>,
    /// Trader sell value in coins.
    #[serde(default)]
    pub value: Option<u32>,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub stack_size: Option<u32>,
    /// What the item breaks down into when recycled at the workshop.
    #[serde(default)]
    pub recycles_into: Vec<ItemQuantity>,
    /// What it breaks down into when salvaged during a raid (usually less).
    #[serde(default)]
    pub salvages_into: Vec<ItemQuantity>,
    /// Workshop / hideout upgrades and quests that consume this item.
    #[serde(default)]
    pub required_for: Vec<Requirement>,
    /// Names of items crafted from this one (it is a recipe ingredient).
    #[serde(default)]
    pub ingredient_of: Vec<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    /// Everything else the data source knows, for the detailed view.
    #[serde(default, skip_serializing_if = "ItemDetails::is_empty")]
    pub details: ItemDetails,
}

/// What the item card doesn't need but the detailed view shows: stats,
/// crafting, upgrades, repair, vendors, mods, where it's found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemDetails {
    /// Stats and effects as `(label, value)`, in the catalogue's language
    /// ("Ammo Type", "Medium Ammo"; "Damage", "8").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<(String, String)>,
    /// What crafting it takes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe: Vec<ItemQuantity>,
    /// How many one craft makes (1 when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub craft_quantity: Option<u32>,
    /// Where it is crafted: station ids, or `in_raid`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub craft_bench: Vec<String>,
    /// The station level crafting it needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_level: Option<u32>,
    /// The next tier (weapons).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrades_to: Option<ItemId>,
    /// What upgrading the previous tier into this one costs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrade_cost: Vec<ItemQuantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repair_cost: Vec<ItemQuantity>,
    /// Share of durability a repair restores (0–1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_durability: Option<f32>,
    /// Traders that sell it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendors: Vec<Vendor>,
    /// Kinds of place it drops in ("Residential", "ARC").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub found_in: Vec<String>,
    /// Weapons a mod fits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatible_with: Vec<String>,
    /// A weapon's mod slots and the mods each takes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mod_slots: Vec<(String, Vec<ItemId>)>,
    /// Crafting it needs its blueprint learned.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub blueprint_locked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quest_item: bool,
    /// The game version it came with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_in: Option<String>,
    /// A hint from the data source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}

impl ItemDetails {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A trader's offer of the item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vendor {
    pub trader: String,
    /// The price: items, or the currencies `coins` / `creds` as ids.
    pub cost: Vec<ItemQuantity>,
    /// How many per refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_seconds: Option<u32>,
    /// The trader level it needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_level: Option<u32>,
}

/// A reason to keep an item: something in the game consumes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub kind: RequirementKind,
    /// Human-readable name of the quest / upgrade / project.
    pub name: String,
    pub quantity: u32,
    /// What it is for, by id: the station of a workshop upgrade, the
    /// quest, or the project.
    #[serde(default)]
    pub station: Option<String>,
    /// The level a workshop upgrade unlocks, or a project's phase (from 1).
    #[serde(default)]
    pub level: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementKind {
    Quest,
    WorkshopUpgrade,
    Project,
    Crafting,
}
