//! Plans goals (workshop stations' next levels, items to craft), all at
//! once and in no order, down to materials: what they take together, what
//! the stash already holds (counted once across all of them), what can be
//! crafted from parts, what is still missing, and which spare stash items
//! would recycle into the missing parts.

use crate::Progress;
use crate::item::{Item, ItemId, ItemQuantity, RequirementKind};
use std::collections::{BTreeMap, HashMap};

/// What to plan for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// Build `station` up to `level` (from the player's current level).
    Station { station: String, level: u32 },
    /// Craft `count` of `item`.
    Craft { item: ItemId, count: u32 },
    /// Deliver `project`'s phases up to `phase` (from those done; an
    /// expedition is a project too).
    Project { project: String, phase: u32 },
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

/// What `goals` take together from `items`, with the stash's `have`
/// counts used first (each stash item counts once, whichever goal takes
/// it). `progress` says what is built and delivered already. `spare`:
/// how many of a stash item
/// may be broken down (the caller's advice: SELL/RECYCLE items whole, KEEP
/// ones beyond their needs).
pub fn plan(
    goals: &[Goal],
    items: &[Item],
    have: &BTreeMap<ItemId, u32>,
    progress: &Progress,
    spare: impl Fn(&ItemId) -> u32,
) -> Plan {
    let by_id: HashMap<&ItemId, &Item> = items.iter().map(|i| (&i.id, i)).collect();
    let mut direct: BTreeMap<ItemId, u32> = BTreeMap::new();
    for goal in goals {
        for (id, quantity) in direct_needs(goal, items, &by_id, progress) {
            *direct.entry(id).or_default() += quantity;
        }
    }
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

/// What one goal asks for directly.
fn direct_needs(
    goal: &Goal,
    items: &[Item],
    by_id: &HashMap<&ItemId, &Item>,
    progress: &Progress,
) -> BTreeMap<ItemId, u32> {
    let mut needs = BTreeMap::new();
    // Requirements of `kind` on `id` past what is done, up to `upto`,
    // less what is delivered already.
    let mut ahead = |kind: RequirementKind, id: &str, upto: u32| {
        for item in items {
            for req in &item.required_for {
                let wanted = req.kind == kind
                    && req.station.as_deref() == Some(id)
                    && req.level.is_some_and(|l| l <= upto);
                let left = if wanted {
                    progress.still_needed(&item.id, req)
                } else {
                    0
                };
                if left > 0 {
                    *needs.entry(item.id.clone()).or_default() += left;
                }
            }
        }
    };
    match goal {
        Goal::Station { station, level } => {
            ahead(RequirementKind::WorkshopUpgrade, station, *level);
        }
        Goal::Project { project, phase } => ahead(RequirementKind::Project, project, *phase),
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

    /// Progress with `station` at `level` and `project` at `phases` done.
    fn at(levels: &[(&str, u32)]) -> Progress {
        let mut progress = Progress::default();
        for &(id, level) in levels {
            if id == "trophy" {
                progress.projects.insert(id.into(), level);
            } else {
                progress.stations.insert(id.into(), level);
            }
        }
        progress
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
        let plan = plan(&[goal], &items, &have, &at(&[("weapon_bench", 1)]), |_| {
            u32::MAX
        });
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
        let plan = plan(&[goal], &items, &have, &at(&[("weapon_bench", 2)]), |_| 0);
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
        let plan = plan(&[goal], &items, &BTreeMap::new(), &at(&[]), |_| 0);
        assert_eq!(line(&plan, "metal_parts").missing, 6);
        assert_eq!(line(&plan, "rubber_parts").missing, 3);
    }

    #[test]
    fn several_goals_share_the_stash() {
        let mut items = catalogue();
        // A second station that also takes rusted gears.
        let mut gear = items.remove(1);
        gear.required_for.push(Requirement {
            kind: RequirementKind::WorkshopUpgrade,
            name: "Refiner 1".into(),
            quantity: 3,
            station: Some("refiner".into()),
            level: Some(1),
        });
        items.push(gear);
        let have: BTreeMap<ItemId, u32> = [(ItemId::new("rusted_gear"), 5)].into_iter().collect();
        let goals = [
            Goal::Station {
                station: "weapon_bench".into(),
                level: 2,
            },
            Goal::Station {
                station: "refiner".into(),
                level: 1,
            },
        ];
        let level = at(&[("weapon_bench", 1)]);
        let planned = plan(&goals, &items, &have, &level, |_| 0);
        // 4 for Gunsmith 2 + 3 for Refiner 1, the 5 in the stash once.
        let gear = line(&planned, "rusted_gear");
        assert_eq!((gear.needed, gear.from_stash, gear.missing), (7, 5, 2));
        assert_eq!(line(&planned, "mechanical_components").needed, 6);
        // A project's phases past those done count the same way.
        items[0].required_for.push(Requirement {
            kind: RequirementKind::Project,
            name: "Trophy Display: 2".into(),
            quantity: 2,
            station: Some("trophy".into()),
            level: Some(2),
        });
        let project = Goal::Project {
            project: "trophy".into(),
            phase: 2,
        };
        let mut one_done = at(&[("trophy", 1)]);
        let planned = plan(
            std::slice::from_ref(&project),
            &items,
            &BTreeMap::new(),
            &one_done,
            |_| 0,
        );
        assert_eq!(line(&planned, "mechanical_components").needed, 2);
        // What is delivered to the current phase already is not needed.
        one_done.delivered.insert(
            "trophy".into(),
            [(ItemId::new("mechanical_components"), 1)]
                .into_iter()
                .collect(),
        );
        let planned = plan(&[project], &items, &BTreeMap::new(), &one_done, |_| 0);
        assert_eq!(line(&planned, "mechanical_components").needed, 1);
        assert_eq!(plan(&[], &items, &have, &level, |_| 0), Plan::default());
    }
}
