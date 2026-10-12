//! Plans a goal (a workshop station's next levels, or crafting an item)
//! down to materials: what it takes, what the stash already holds, what can
//! be crafted from parts, what is still missing, and which spare stash
//! items would recycle into the missing parts.

use crate::item::{Item, ItemId, ItemQuantity, RequirementKind};
use std::collections::{BTreeMap, HashMap};

/// What to plan for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// Build `station` up to `level` (from the player's current level).
    Station { station: String, level: u32 },
    /// Craft `count` of `item`.
    Craft { item: ItemId, count: u32 },
}

/// One material of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLine {
    pub item: ItemId,
    /// How many the goal takes, all paths together.
    pub needed: u32,
    /// Taken from the stash.
    pub from_stash: u32,
    /// Crafted from parts (listed as lines of their own).
    pub crafted: u32,
    /// Still to find.
    pub missing: u32,
    /// 0 for what the goal asks directly, 1 for parts of those, …
    pub depth: u8,
}

/// Breaking down a spare stash item for parts the plan misses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleHint {
    pub from: ItemId,
    pub count: u32,
    pub gives: Vec<ItemQuantity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Direct needs first, then parts; by name within a depth is the
    /// caller's business.
    pub lines: Vec<PlanLine>,
    pub recycle: Vec<RecycleHint>,
}

impl Plan {
    pub fn missing(&self) -> u32 {
        self.lines.iter().map(|l| l.missing).sum()
    }
}

/// Parts are crafted at most this deep (Refiner chains are 2-3 deep).
const MAX_DEPTH: u8 = 4;

/// What `goal` takes from `items`, with the stash's `have` counts used
/// first. `current_level`: the station's level now (station goals).
/// `spare`: how many of a stash item may be broken down (the caller's
/// advice: SELL/RECYCLE items whole, KEEP ones beyond their needs).
pub fn plan(
    goal: &Goal,
    items: &[Item],
    have: &BTreeMap<ItemId, u32>,
    current_level: u32,
    spare: impl Fn(&ItemId) -> u32,
) -> Plan {
    let by_id: HashMap<&ItemId, &Item> = items.iter().map(|i| (&i.id, i)).collect();
    let direct = direct_needs(goal, items, &by_id, current_level);
    let mut stock = have.clone();
    let mut lines: BTreeMap<ItemId, PlanLine> = BTreeMap::new();
    let mut queue: Vec<(ItemId, u32, u8)> = direct.into_iter().map(|(id, q)| (id, q, 0)).collect();
    while let Some((id, quantity, depth)) = queue.pop() {
        let line = lines.entry(id.clone()).or_insert_with(|| PlanLine {
            item: id.clone(),
            needed: 0,
            from_stash: 0,
            crafted: 0,
            missing: 0,
            depth,
        });
        line.depth = line.depth.min(depth);
        line.needed += quantity;
        let in_stock = stock.entry(id.clone()).or_default();
        let take = quantity.min(*in_stock);
        *in_stock -= take;
        line.from_stash += take;
        let short = quantity - take;
        if short == 0 {
            continue;
        }
        let recipe = by_id
            .get(&id)
            .map(|i| {
                (
                    &i.details.recipe,
                    i.details.craft_quantity.unwrap_or(1).max(1),
                )
            })
            .filter(|(recipe, _)| !recipe.is_empty() && depth < MAX_DEPTH);
        match recipe {
            Some((recipe, makes)) => {
                let crafts = short.div_ceil(makes);
                line.crafted += short;
                for part in recipe {
                    queue.push((part.item.clone(), part.quantity * crafts, depth + 1));
                }
            }
            None => line.missing += short,
        }
    }
    let mut lines: Vec<PlanLine> = lines.into_values().collect();
    lines.sort_by_key(|l| l.depth);
    let recycle = recycle_hints(&lines, items, &stock, spare);
    Plan { lines, recycle }
}

/// What the goal asks for directly.
fn direct_needs(
    goal: &Goal,
    items: &[Item],
    by_id: &HashMap<&ItemId, &Item>,
    current_level: u32,
) -> BTreeMap<ItemId, u32> {
    let mut needs = BTreeMap::new();
    match goal {
        Goal::Station { station, level } => {
            for item in items {
                for req in &item.required_for {
                    let wanted = req.kind == RequirementKind::WorkshopUpgrade
                        && req.station.as_deref() == Some(station.as_str())
                        && req.level.is_some_and(|l| l > current_level && l <= *level);
                    if wanted {
                        *needs.entry(item.id.clone()).or_default() += req.quantity;
                    }
                }
            }
        }
        Goal::Craft { item, count } => {
            if let Some(item) = by_id.get(item) {
                let makes = item.details.craft_quantity.unwrap_or(1).max(1);
                let crafts = count.div_ceil(makes);
                for part in &item.details.recipe {
                    *needs.entry(part.item.clone()).or_default() += part.quantity * crafts;
                }
            }
        }
    }
    needs
}

/// For each missing part, the spare stash items that recycle into it,
/// greedily: as many as cover the gap or as are spare.
fn recycle_hints(
    lines: &[PlanLine],
    items: &[Item],
    stock: &BTreeMap<ItemId, u32>,
    spare: impl Fn(&ItemId) -> u32,
) -> Vec<RecycleHint> {
    let in_plan: Vec<&ItemId> = lines.iter().map(|l| &l.item).collect();
    let mut left: BTreeMap<&ItemId, u32> = stock
        .iter()
        .filter(|(id, n)| **n > 0 && !in_plan.contains(id))
        .map(|(id, &n)| (id, n.min(spare(id))))
        .filter(|(_, n)| *n > 0)
        .collect();
    let mut hints: BTreeMap<ItemId, u32> = BTreeMap::new();
    for line in lines.iter().filter(|l| l.missing > 0) {
        let mut gap = line.missing;
        for item in items {
            if gap == 0 {
                break;
            }
            let Some(yields) = item
                .recycles_into
                .iter()
                .find(|q| q.item == line.item)
                .map(|q| q.quantity)
                .filter(|&y| y > 0)
            else {
                continue;
            };
            let Some(available) = left.get_mut(&item.id) else {
                continue;
            };
            let count = gap.div_ceil(yields).min(*available);
            if count == 0 {
                continue;
            }
            *available -= count;
            gap = gap.saturating_sub(count * yields);
            *hints.entry(item.id.clone()).or_default() += count;
        }
    }
    let by_id: HashMap<&ItemId, &Item> = items.iter().map(|i| (&i.id, i)).collect();
    hints
        .into_iter()
        .map(|(from, count)| {
            let gives = by_id
                .get(&from)
                .map(|i| {
                    i.recycles_into
                        .iter()
                        .map(|q| ItemQuantity {
                            item: q.item.clone(),
                            quantity: q.quantity * count,
                        })
                        .collect()
                })
                .unwrap_or_default();
            RecycleHint { from, count, gives }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemDetails, Requirement};

    fn item(id: &str) -> Item {
        Item {
            id: ItemId::new(id),
            name: id.into(),
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
            details: ItemDetails::default(),
        }
    }

    fn q(id: &str, quantity: u32) -> ItemQuantity {
        ItemQuantity {
            item: ItemId::new(id),
            quantity,
        }
    }

    fn upgrade(level: u32, quantity: u32) -> Requirement {
        Requirement {
            kind: RequirementKind::WorkshopUpgrade,
            name: format!("Gunsmith {level}"),
            quantity,
            station: Some("weapon_bench".into()),
            level: Some(level),
        }
    }

    /// Gunsmith 2 takes 6 mechanical components and 4 rusted gears;
    /// Gunsmith 3 another 2 gears. A component is 2 metal parts + 1 rubber.
    /// An alarm clock recycles into 3 metal parts.
    fn catalogue() -> Vec<Item> {
        let mut mech = item("mechanical_components");
        mech.required_for = vec![upgrade(2, 6)];
        mech.details.recipe = vec![q("metal_parts", 2), q("rubber_parts", 1)];
        let mut gear = item("rusted_gear");
        gear.required_for = vec![upgrade(2, 4), upgrade(3, 2)];
        let mut clock = item("alarm_clock");
        clock.recycles_into = vec![q("metal_parts", 3)];
        vec![mech, gear, item("metal_parts"), item("rubber_parts"), clock]
    }

    fn line<'a>(plan: &'a Plan, id: &str) -> &'a PlanLine {
        plan.lines.iter().find(|l| l.item.as_str() == id).unwrap()
    }

    #[test]
    fn station_levels_use_the_stash_then_craft_then_miss() {
        let items = catalogue();
        let have: BTreeMap<ItemId, u32> = [
            (ItemId::new("mechanical_components"), 2),
            (ItemId::new("rusted_gear"), 5),
            (ItemId::new("metal_parts"), 3),
            (ItemId::new("alarm_clock"), 4),
        ]
        .into_iter()
        .collect();
        let goal = Goal::Station {
            station: "weapon_bench".into(),
            level: 3,
        };
        let plan = plan(&goal, &items, &have, 1, |_| u32::MAX);
        let mech = line(&plan, "mechanical_components");
        assert_eq!((mech.needed, mech.from_stash, mech.crafted), (6, 2, 4));
        let gear = line(&plan, "rusted_gear");
        assert_eq!((gear.needed, gear.from_stash, gear.missing), (6, 5, 1));
        // 4 crafts: 8 metal parts (3 in the stash) and 4 rubber parts.
        let metal = line(&plan, "metal_parts");
        assert_eq!(
            (metal.needed, metal.from_stash, metal.missing, metal.depth),
            (8, 3, 5, 1)
        );
        assert_eq!(line(&plan, "rubber_parts").missing, 4);
        assert_eq!(plan.missing(), 10);
        // Two clocks cover the 5 metal parts.
        assert_eq!(
            plan.recycle,
            [RecycleHint {
                from: ItemId::new("alarm_clock"),
                count: 2,
                gives: vec![q("metal_parts", 6)],
            }]
        );
    }

    #[test]
    fn built_levels_are_skipped_and_spares_respected() {
        let items = catalogue();
        let have: BTreeMap<ItemId, u32> = [(ItemId::new("alarm_clock"), 4)].into_iter().collect();
        let goal = Goal::Station {
            station: "weapon_bench".into(),
            level: 3,
        };
        // Level 2 built: only Gunsmith 3's gears.
        let plan = plan(&goal, &items, &have, 2, |_| 0);
        assert_eq!(plan.lines.len(), 1);
        assert_eq!(line(&plan, "rusted_gear").missing, 2);
        assert_eq!(plan.recycle, []);
    }

    #[test]
    fn crafting_goals_multiply_the_recipe() {
        let items = catalogue();
        let goal = Goal::Craft {
            item: ItemId::new("mechanical_components"),
            count: 3,
        };
        let plan = plan(&goal, &items, &BTreeMap::new(), 0, |_| 0);
        assert_eq!(line(&plan, "metal_parts").missing, 6);
        assert_eq!(line(&plan, "rubber_parts").missing, 3);
    }
}
