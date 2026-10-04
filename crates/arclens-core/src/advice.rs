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
    /// A blueprint the player hasn't learned: learn it, don't sell it.
    Learn,
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
    /// Requirements still ahead of the player (all of them without progress).
    #[serde(default)]
    pub needs: Vec<crate::item::Requirement>,
    /// Set when breaking the item down is advised because its parts feed an
    /// upgrade the player still has to build: that upgrade's name.
    #[serde(default)]
    pub parts_for: Option<String>,
}

/// Break an item down for upgrade parts only if the parts are worth at least
/// this share of its sell value. Beyond that the loss is too big to justify
/// without knowing how many parts the player already owns.
pub const PARTS_MIN_VALUE_PERCENT: u64 = 60;

/// [`advise_in`] at the workshop with dataset values and no progress.
pub fn advise<'a>(item: &Item, lookup: impl Fn(&ItemId) -> Option<&'a Item>) -> Advice {
    advise_in(item, Situation::default(), None, lookup)
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
///
/// With `progress`, upgrades already built no longer count, and an item whose
/// parts feed a *remaining* upgrade is advised for breakdown even when
/// selling pays more. Without it, the verdict is purely about value (every
/// material feeds *some* upgrade, so "needed" would be meaningless).
pub fn advise_in<'a>(
    item: &Item,
    situation: Situation,
    progress: Option<&crate::Progress>,
    lookup: impl Fn(&ItemId) -> Option<&'a Item>,
) -> Advice {
    let lookup = &lookup;
    let value_from_game = situation.sell_value.is_some();
    let sell_value = situation.sell_value.or(item.value);
    let outputs = breakdown(item, situation.place);
    let recycle_value = outputs_value(outputs, lookup);
    let still_needed = |req: &&crate::item::Requirement| progress.is_none_or(|p| p.needs(req));
    let needs: Vec<_> = item
        .required_for
        .iter()
        .filter(still_needed)
        .cloned()
        .collect();

    // Parts for a remaining *workshop upgrade* (only meaningful with progress).
    let parts_for = progress.and_then(|p| {
        outputs.iter().find_map(|output| {
            lookup(&output.item)?
                .required_for
                .iter()
                .find(|r| r.kind == crate::RequirementKind::WorkshopUpgrade && p.needs(r))
                .map(|r| r.name.clone())
        })
    });

    // We can't see the stash, so we don't know how many parts are still
    // missing: only trade value for parts when it costs little.
    let cheap_to_break_down = match (recycle_value, sell_value) {
        (Some(r), Some(s)) => u64::from(r) * 100 >= u64::from(s) * PARTS_MIN_VALUE_PERCENT,
        (Some(_), None) => true,
        _ => false,
    };

    let verdict = if item.is_blueprint() && progress.is_none_or(|p| !p.blueprint_learned(&item.id))
    {
        Verdict::Learn
    } else if !needs.is_empty() {
        Verdict::Keep
    } else if parts_for.is_some() && cheap_to_break_down {
        Verdict::Recycle
    } else {
        match (sell_value, recycle_value) {
            (Some(sell), Some(recycle)) if recycle > sell => Verdict::Recycle,
            (Some(_), _) => Verdict::Sell,
            (None, Some(_)) => Verdict::Recycle,
            (None, None) => Verdict::Unknown,
        }
    };
    // Kept as a hint even when the verdict stays SELL ("parts would help").
    let parts_for = parts_for.filter(|_| verdict != Verdict::Keep);

    Advice {
        verdict,
        sell_value,
        recycle_value,
        place: situation.place,
        value_from_game,
        needs,
        parts_for,
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
            aliases: Vec::new(),
            description: None,
            rarity: None,
            category: None,
            value,
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: None,
            details: crate::item::ItemDetails::default(),
        }
    }

    fn catalogue(items: &[Item]) -> HashMap<ItemId, Item> {
        items.iter().map(|i| (i.id.clone(), i.clone())).collect()
    }

    #[test]
    fn blueprints_are_learned_until_marked_learned() {
        let mut canto = item("canto_blueprint", Some(5000));
        canto.category = Some("Blueprint".into());
        assert_eq!(advise(&canto, |_| None).verdict, Verdict::Learn);
        let mut progress = crate::Progress::default();
        assert_eq!(
            advise_in(&canto, Situation::default(), Some(&progress), |_| None).verdict,
            Verdict::Learn
        );
        // A duplicate of one already learned is just worth its price.
        progress.blueprints.insert(canto.id.clone());
        assert_eq!(
            advise_in(&canto, Situation::default(), Some(&progress), |_| None).verdict,
            Verdict::Sell
        );
    }

    #[test]
    fn keeps_items_that_are_required() {
        let mut gear = item("gear", Some(1000));
        gear.required_for.push(upgrade("weapon_bench", 2));
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
        let advice = advise_in(&augment, situation, None, |id| cat.get(id));
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
        let raid = advise_in(&pistol, raid, None, |id| cat.get(id));
        assert_eq!(
            (raid.recycle_value, raid.verdict),
            (Some(200), Verdict::Sell)
        );
        assert_eq!(raid.place, Place::Raid);
    }

    fn upgrade(station: &str, level: u32) -> Requirement {
        Requirement {
            kind: RequirementKind::WorkshopUpgrade,
            name: format!("Gunsmith {level}"),
            quantity: 5,
            station: Some(station.into()),
            level: Some(level),
        }
    }

    #[test]
    fn built_upgrades_no_longer_force_keep() {
        let mut gear = item("gear", Some(1000));
        gear.required_for.push(upgrade("weapon_bench", 2));
        let mut progress = crate::Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);
        let advice = advise_in(&gear, Situation::default(), Some(&progress), |_| None);
        assert_eq!(advice.verdict, Verdict::Sell);
        assert_eq!(advice.needs.len(), 0);
    }

    #[test]
    fn recycles_for_parts_a_remaining_upgrade_needs() {
        // Selling pays more (1000 vs 700), but the parts are needed for
        // Gunsmith 3 and cost only 30 % of the value.
        let mut parts = item("parts", Some(350));
        parts.required_for.push(upgrade("weapon_bench", 3));
        let mut gun = item("gun", Some(1000));
        gun.recycles_into.push(ItemQuantity {
            item: parts.id.clone(),
            quantity: 2,
        });
        let cat = catalogue(&[parts]);
        let mut progress = crate::Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);

        let advice = advise_in(&gun, Situation::default(), Some(&progress), |id| {
            cat.get(id)
        });
        assert_eq!(advice.verdict, Verdict::Recycle);
        assert_eq!(advice.parts_for.as_deref(), Some("Gunsmith 3"));

        // Once Gunsmith 3 is built, it's about value again.
        progress.stations.insert("weapon_bench".into(), 3);
        let advice = advise_in(&gun, Situation::default(), Some(&progress), |id| {
            cat.get(id)
        });
        assert_eq!((advice.verdict, advice.parts_for), (Verdict::Sell, None));

        // Without progress we don't guess.
        assert_eq!(advise(&gun, |id| cat.get(id)).verdict, Verdict::Sell);
    }

    #[test]
    fn expensive_items_are_sold_with_a_parts_hint() {
        // Parts worth 30 % of the price: too costly to scrap blindly.
        let mut parts = item("parts", Some(150));
        parts.required_for.push(upgrade("weapon_bench", 3));
        let mut gun = item("gun", Some(1000));
        gun.recycles_into.push(ItemQuantity {
            item: parts.id.clone(),
            quantity: 2,
        });
        let cat = catalogue(&[parts]);
        let progress = crate::Progress::default();

        let advice = advise_in(&gun, Situation::default(), Some(&progress), |id| {
            cat.get(id)
        });
        assert_eq!(advice.verdict, Verdict::Sell);
        assert_eq!(advice.parts_for.as_deref(), Some("Gunsmith 3"));
    }

    #[test]
    fn unknown_without_any_values() {
        let advice = advise(&item("mystery", None), |_| None);
        assert_eq!(advice.verdict, Verdict::Unknown);
    }
}
