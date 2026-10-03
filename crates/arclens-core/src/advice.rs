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

/// A verdict plus the numbers that justify it, so the UI can explain itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advice {
    pub verdict: Verdict,
    pub sell_value: Option<u32>,
    /// Summed sell value of the recycling outputs, if all of them are known.
    pub recycle_value: Option<u32>,
}

/// Computes the [`Advice`] for `item`.
///
/// `lookup` resolves recycling outputs to their catalogue entries; outputs it
/// cannot resolve make the recycle value unknown rather than silently low.
pub fn advise<'a>(item: &Item, lookup: impl Fn(&ItemId) -> Option<&'a Item>) -> Advice {
    let sell_value = item.value;
    let recycle_value = recycle_value(item, lookup);

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
    }
}

fn recycle_value<'a>(item: &Item, lookup: impl Fn(&ItemId) -> Option<&'a Item>) -> Option<u32> {
    if item.recycles_into.is_empty() {
        return None;
    }
    item.recycles_into.iter().try_fold(0u32, |acc, output| {
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
    fn unknown_without_any_values() {
        let advice = advise(&item("mystery", None), |_| None);
        assert_eq!(advice.verdict, Verdict::Unknown);
    }
}
