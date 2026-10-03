//! "Keep, sell or recycle?" — the loot verdict shown in the overlay.
//!
//! This is pure logic over the item catalogue so it can be unit tested
//! without any I/O.

use crate::item::{Item, ItemId};
use serde::{Deserialize, Serialize};

/// What the player should do with an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Something the player still needs consumes this item.
    Keep,
    /// Recycling yields more value than selling.
    Recycle,
    /// Selling is the best use.
    Sell,
    /// Not enough data to decide.
    Unknown,
}

/// Where the player is: it changes what breaking an item down yields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Main menu / workshop: items are *recycled* (`recycles_into`).
    #[default]
    Workshop,
    /// In a raid: items are *salvaged* (`salvages_into`, usually less).
    Raid,
}

/// What we know about the moment of the decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Situation {
    pub place: Place,
    /// The game's own current sell value (read from the tooltip). Beats the
    /// dataset's base value, which ignores durability and upgrades.
    pub sell_value: Option<u32>,
}

/// A verdict plus the numbers that justify it, so the UI can explain itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advice {
    pub verdict: Verdict,
    pub sell_value: Option<u32>,
    /// Summed sell value of the breakdown outputs (recycle or salvage,
    /// depending on `place`), if all of them are known.
    pub recycle_value: Option<u32>,
    #[serde(default)]
    pub place: Place,
    /// `sell_value` is the game's own number, not the dataset's base value.
    #[serde(default)]
    pub value_from_game: bool,
}

/// [`advise_in`] at the workshop with dataset values.
pub fn advise<'a>(item: &Item, lookup: impl Fn(&ItemId) -> Option<&'a Item>) -> Advice {
    advise_in(item, Situation::default(), lookup)
}

/// The breakdown outputs that apply in `place`.
pub fn breakdown(item: &Item, place: Place) -> &[crate::item::ItemQuantity] {
    match place {
        Place::Workshop => &item.recycles_into,
        Place::Raid => &item.salvages_into,
    }
}

/// Computes the [`Advice`] for `item` in `situation`.
///
/// `lookup` resolves breakdown outputs to their catalogue entries; outputs it
/// cannot resolve make the recycle value unknown rather than silently low.
pub fn advise_in<'a>(
    item: &Item,
    situation: Situation,
    lookup: impl Fn(&ItemId) -> Option<&'a Item>,
) -> Advice {
    let value_from_game = situation.sell_value.is_some();
    let sell_value = situation.sell_value.or(item.value);
    let recycle_value = outputs_value(breakdown(item, situation.place), lookup);

    let verdict = if item.required_for.is_empty() {
        match (sell_value, recycle_value) {
            (Some(sell), Some(recycle)) if recycle > sell => Verdict::Recycle,
            (Some(_), _) => Verdict::Sell,
            (None, Some(_)) => Verdict::Recycle,
            (None, None) => Verdict::Unknown,
        }
    } else {
        Verdict::Keep
    };

    Advice {
        verdict,
        sell_value,
        recycle_value,
        place: situation.place,
        value_from_game,
    }
}

fn outputs_value<'a>(
    outputs: &[crate::item::ItemQuantity],
    lookup: impl Fn(&ItemId) -> Option<&'a Item>,
) -> Option<u32> {
    if outputs.is_empty() {
        return None;
    }
    outputs.iter().try_fold(0u32, |acc, output| {
        let unit = lookup(&output.item)?.value?;
        Some(acc.saturating_add(unit.saturating_mul(output.quantity)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ItemQuantity, Requirement, RequirementKind};
    use std::collections::HashMap;

    fn item(id: &str, value: Option<u32>) -> Item {
        Item {
            id: ItemId::new(id),
            name: id.to_owned(),
            description: None,
            rarity: None,
            category: None,
            value,
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            image_url: None,
        }
    }

    fn catalogue(items: &[Item]) -> HashMap<ItemId, Item> {
        items.iter().map(|i| (i.id.clone(), i.clone())).collect()
    }

    #[test]
    fn keeps_items_that_are_required() {
        let mut gear = item("gear", Some(1000));
        gear.required_for.push(Requirement {
            kind: RequirementKind::WorkshopUpgrade,
            name: "Gunsmith 2".into(),
            quantity: 3,
        });
        let advice = advise(&gear, |_| None);
        assert_eq!(advice.verdict, Verdict::Keep);
    }

    #[test]
    fn recycles_when_outputs_are_worth_more() {
        let metal = item("metal", Some(75));
        let mut gear = item("gear", Some(100));
        gear.recycles_into.push(ItemQuantity {
            item: metal.id.clone(),
            quantity: 2,
        });
        let cat = catalogue(&[metal]);
        let advice = advise(&gear, |id| cat.get(id));
        assert_eq!(advice.verdict, Verdict::Recycle);
        assert_eq!(advice.recycle_value, Some(150));
    }

    #[test]
    fn sells_when_recycle_value_is_unknown() {
        let mut gear = item("gear", Some(100));
        gear.recycles_into.push(ItemQuantity {
            item: ItemId::new("missing"),
            quantity: 5,
        });
        let advice = advise(&gear, |_| None);
        assert_eq!(advice.verdict, Verdict::Sell);
        assert_eq!(advice.recycle_value, None);
    }

    #[test]
    fn game_value_beats_dataset_value() {
        // Dataset says 2000, but at 41 % durability the game pays 800 —
        // now recycling (1000) wins.
        let metal = item("metal", Some(500));
        let mut augment = item("augment", Some(2000));
        augment.recycles_into.push(ItemQuantity {
            item: metal.id.clone(),
            quantity: 2,
        });
        let cat = catalogue(&[metal]);
        let situation = Situation {
            place: Place::Workshop,
            sell_value: Some(800),
        };
        let advice = advise_in(&augment, situation, |id| cat.get(id));
        assert_eq!(advice.sell_value, Some(800));
        assert!(advice.value_from_game);
        assert_eq!(advice.verdict, Verdict::Recycle);
    }

    #[test]
    fn raid_uses_salvage_outputs() {
        let metal = item("metal", Some(100));
        let rubber = item("rubber", Some(100));
        let mut pistol = item("pistol", Some(250));
        pistol.recycles_into = vec![
            ItemQuantity {
                item: metal.id.clone(),
                quantity: 2,
            },
            ItemQuantity {
                item: rubber.id.clone(),
                quantity: 1,
            },
        ];
        pistol.salvages_into = vec![ItemQuantity {
            item: metal.id.clone(),
            quantity: 2,
        }];
        let cat = catalogue(&[metal, rubber]);

        let workshop = advise(&pistol, |id| cat.get(id));
        assert_eq!(
            (workshop.recycle_value, workshop.verdict),
            (Some(300), Verdict::Recycle)
        );

        let raid = Situation {
            place: Place::Raid,
            sell_value: None,
        };
        let raid = advise_in(&pistol, raid, |id| cat.get(id));
        assert_eq!(
            (raid.recycle_value, raid.verdict),
            (Some(200), Verdict::Sell)
        );
        assert_eq!(raid.place, Place::Raid);
    }

    #[test]
    fn unknown_without_any_values() {
        let advice = advise(&item("mystery", None), |_| None);
        assert_eq!(advice.verdict, Verdict::Unknown);
    }
}
