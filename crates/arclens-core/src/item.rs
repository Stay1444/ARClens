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
    pub name: String,
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
