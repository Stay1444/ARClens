//! The overlay's detailed item window, assembled from the catalogue and the
//! player's progress: every fact the data has, ready to draw.

use crate::icons::Icons;
use arclens_core::{Advice, Item, ItemId, ItemQuantity, Progress, RequirementKind};
use arclens_data::Catalog;
use arclens_i18n::{t, try_tr};
use arclens_ipc::{DetailRow, DetailSection, ItemDetail};
use arclens_ui::format::thousands;

/// Everything about `item` for the overlay's window.
pub fn build(
    item: &Item,
    advice: Advice,
    catalog: &Catalog,
    progress: Option<&Progress>,
    icons: &Icons,
) -> ItemDetail {
    let rows = Rows { catalog, icons };
    let mut sections = Vec::new();
    let mut push = |title: String, rows: Vec<DetailRow>| {
        if !rows.is_empty() {
            sections.push(DetailSection { title, rows });
        }
    };
    push(t!("card-needed-for"), needed_for(item, progress));
    push(
        t!("card-recycles-into"),
        rows.items(&item.recycles_into, true),
    );
    push(
        t!("card-salvages-into"),
        rows.items(&item.salvages_into, true),
    );
    push(
        t!("detail-crafting"),
        crafting(item, catalog, progress, &rows),
    );
    push(
        t!("card-used-to-craft"),
        used_to_craft(item, catalog, &rows),
    );
    push(t!("detail-upgrades"), upgrades(item, catalog, &rows));
    push(
        match item.details.repair_durability {
            Some(share) => t!("detail-repair-restores", percent = percent(share)),
            None => t!("detail-repair"),
        },
        rows.items(&item.details.repair_cost, false),
    );
    push(t!("detail-sold-by"), vendors(item, &rows));
    push(t!("detail-unlocks"), unlocks(item, catalog, &rows));
    push(t!("detail-mod-slots"), mod_slots(item, catalog));
    if !item.details.compatible_with.is_empty() {
        push(
            t!("detail-fits"),
            vec![DetailRow {
                name: item.details.compatible_with.join(", "),
                ..DetailRow::default()
            }],
        );
    }
    ItemDetail {
        facts: facts(item, &advice, catalog, progress),
        stats: item.details.stats.clone(),
        icon: icons.get(&item.id).map(|i| i.path.clone()),
        item: item.clone(),
        advice,
        sections,
    }
}

/// Every item the window shows an icon for, to fetch.
pub fn referenced(item: &Item, catalog: &Catalog) -> Vec<ItemId> {
    let d = &item.details;
    let mut ids: Vec<ItemId> = std::iter::once(&item.id)
        .chain(item.recycles_into.iter().map(|q| &q.item))
        .chain(item.salvages_into.iter().map(|q| &q.item))
        .chain(d.recipe.iter().map(|q| &q.item))
        .chain(d.upgrade_cost.iter().map(|q| &q.item))
        .chain(d.repair_cost.iter().map(|q| &q.item))
        .chain(
            d.vendors
                .iter()
                .flat_map(|v| v.cost.iter().map(|q| &q.item)),
        )
        .chain(d.upgrades_to.iter())
        .cloned()
        .collect();
    ids.extend(products(item, catalog).map(|p| p.id.clone()));
    ids.extend(previous_tier(item, catalog).map(|p| p.id.clone()));
    ids.extend(unlocked_by(item, catalog).map(|p| p.id.clone()));
    if let Some(next) = d.upgrades_to.as_ref().and_then(|id| catalog.item(id)) {
        ids.extend(next.details.upgrade_cost.iter().map(|q| q.item.clone()));
    }
    ids
}

/// Item rows: name in the catalogue's language, icon, amount.
struct Rows<'a> {
    catalog: &'a Catalog,
    icons: &'a Icons,
}

impl Rows<'_> {
    fn item(&self, id: &ItemId, quantity: Option<u32>, note: Option<String>) -> DetailRow {
        DetailRow {
            name: self
                .catalog
                .item(id)
                .map_or_else(|| currency(id.as_str()), |i| i.name.clone()),
            icon: self.icons.get(id).map(|i| i.path.clone()),
            quantity,
            note,
            done: false,
        }
    }

    /// One row per quantity; `with_value` notes what they sell for.
    fn items(&self, list: &[ItemQuantity], with_value: bool) -> Vec<DetailRow> {
        list.iter()
            .map(|q| {
                let note = with_value
                    .then(|| self.catalog.item(&q.item).and_then(|i| i.value))
                    .flatten()
                    .map(|v| format!("{} ¢", thousands(v.saturating_mul(q.quantity))));
                self.item(&q.item, Some(q.quantity), note)
            })
            .collect()
    }
}

/// A currency id's name (`coins`, `creds`), else the id.
fn currency(id: &str) -> String {
    try_tr(&format!("detail-currency-{id}")).unwrap_or_else(|| id.to_owned())
}

fn percent(share: f32) -> String {
    format!("{:.0}", share * 100.0)
}

fn facts(
    item: &Item,
    advice: &Advice,
    catalog: &Catalog,
    progress: Option<&Progress>,
) -> Vec<(String, String)> {
    let d = &item.details;
    let coins = |v: u32| format!("{} ¢", thousands(v));
    let mut facts = Vec::new();
    if let Some(value) = item.value {
        facts.push((t!("card-sell"), coins(value)));
    }
    if let Some(value) = advice.recycle_value {
        facts.push((t!("card-recycle"), coins(value)));
    }
    if let Some(weight) = item.weight.filter(|w| *w > 0.0) {
        facts.push((t!("detail-weight"), format!("{weight} kg")));
        if let Some(value) = item.value {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "coin amounts"
            )]
            let per_kg = (value as f32 / weight).round() as u32;
            facts.push((t!("detail-value-per-kg"), coins(per_kg)));
        }
    }
    if let Some(stack) = item.stack_size.filter(|&s| s > 1) {
        facts.push((t!("detail-stack"), stack.to_string()));
    }
    if !d.found_in.is_empty() {
        let places: Vec<String> = d
            .found_in
            .iter()
            .map(|p| {
                try_tr(&format!("zone-{}", arclens_i18n::slug(p))).unwrap_or_else(|| p.clone())
            })
            .collect();
        facts.push((t!("detail-found-in"), places.join(", ")));
    }
    if d.blueprint_locked {
        let state = match (blueprint_of(item, catalog), progress) {
            (Some(bp), Some(p)) if p.blueprint_learned(&bp.id) => t!("detail-blueprint-learned"),
            (Some(_), Some(_)) => t!("detail-blueprint-missing"),
            _ => t!("detail-blueprint-needed"),
        };
        facts.push((t!("detail-blueprint"), state));
    }
    if d.quest_item {
        facts.push((t!("detail-quest-item"), t!("detail-yes")));
    }
    if let Some(version) = &d.added_in {
        let version = if version == "base" {
            t!("detail-base-game")
        } else {
            version.clone()
        };
        facts.push((t!("detail-added-in"), version));
    }
    facts
}

fn needed_for(item: &Item, progress: Option<&Progress>) -> Vec<DetailRow> {
    item.required_for
        .iter()
        .map(|req| DetailRow {
            name: req.name.clone(),
            quantity: Some(req.quantity),
            note: Some(match req.kind {
                RequirementKind::Quest => t!("kind-quest"),
                RequirementKind::WorkshopUpgrade => t!("kind-workshop"),
                RequirementKind::Project => t!("kind-project"),
                RequirementKind::Crafting => t!("kind-crafting"),
            }),
            done: progress.is_some_and(|p| !p.needs(req)),
            icon: None,
        })
        .collect()
}

fn crafting(
    item: &Item,
    catalog: &Catalog,
    progress: Option<&Progress>,
    rows: &Rows<'_>,
) -> Vec<DetailRow> {
    let d = &item.details;
    if d.recipe.is_empty() {
        return Vec::new();
    }
    let benches: Vec<String> = d
        .craft_bench
        .iter()
        .map(|b| {
            if b == "in_raid" {
                t!("detail-in-raid")
            } else {
                catalog
                    .station_names
                    .get(b)
                    .cloned()
                    .unwrap_or_else(|| arclens_core::humanize(b))
            }
        })
        .collect();
    let mut place = if benches.is_empty() {
        t!("detail-crafting")
    } else {
        benches.join(" / ")
    };
    if let Some(level) = d.station_level {
        place = format!("{place} · {}", t!("detail-level", level = level));
    }
    let built = d.craft_bench.iter().any(|b| b == "in_raid")
        || match (d.craft_bench.first(), d.station_level, progress) {
            (Some(bench), Some(level), Some(p)) => p.level(bench) >= level,
            _ => false,
        };
    let mut out = vec![DetailRow {
        name: place,
        note: d.craft_quantity.map(|q| t!("detail-makes", count = q)),
        done: built,
        ..DetailRow::default()
    }];
    out.extend(rows.items(&d.recipe, false));
    out
}

/// Items whose recipe takes `item`.
fn products<'a>(item: &'a Item, catalog: &'a Catalog) -> impl Iterator<Item = &'a Item> + 'a {
    catalog
        .items
        .iter()
        .filter(move |p| p.details.recipe.iter().any(|q| q.item == item.id))
}

fn used_to_craft(item: &Item, catalog: &Catalog, rows: &Rows<'_>) -> Vec<DetailRow> {
    products(item, catalog)
        .map(|p| {
            let needed = p
                .details
                .recipe
                .iter()
                .find(|q| q.item == item.id)
                .map(|q| q.quantity);
            rows.item(&p.id, None, needed.map(|n| format!("×{n}")))
        })
        .collect()
}

/// The tier below: the item that upgrades into `item`.
fn previous_tier<'a>(item: &'a Item, catalog: &'a Catalog) -> Option<&'a Item> {
    catalog
        .items
        .iter()
        .find(|p| p.details.upgrades_to.as_ref() == Some(&item.id))
}

fn upgrades(item: &Item, catalog: &Catalog, rows: &Rows<'_>) -> Vec<DetailRow> {
    let mut out = Vec::new();
    if let Some(previous) = previous_tier(item, catalog) {
        out.push(rows.item(&previous.id, None, Some(t!("detail-upgrade-from"))));
        out.extend(rows.items(&item.details.upgrade_cost, false));
    }
    if let Some(next) = item
        .details
        .upgrades_to
        .as_ref()
        .and_then(|id| catalog.item(id))
    {
        out.push(rows.item(&next.id, None, Some(t!("detail-upgrade-to"))));
        out.extend(rows.items(&next.details.upgrade_cost, false));
    }
    out
}

fn vendors(item: &Item, rows: &Rows<'_>) -> Vec<DetailRow> {
    item.details
        .vendors
        .iter()
        .map(|v| {
            let price: Vec<String> = v
                .cost
                .iter()
                .map(|q| {
                    let name = rows.item(&q.item, None, None).name;
                    if q.item.as_str() == "coins" {
                        format!("{} ¢", thousands(q.quantity))
                    } else {
                        format!("{} {name}", thousands(q.quantity))
                    }
                })
                .collect();
            let mut note = price.join(" + ");
            if let Some(limit) = v.limit {
                let per = match v.refresh_seconds {
                    Some(86_400) => t!("detail-per-day", count = limit),
                    _ => t!("detail-limit", count = limit),
                };
                note = format!("{note} · {per}");
            }
            if let Some(level) = v.required_level {
                note = format!("{note} · {}", t!("detail-level", level = level));
            }
            DetailRow {
                name: v.trader.clone(),
                note: Some(note),
                ..DetailRow::default()
            }
        })
        .collect()
}

/// The blueprint that unlocks crafting `item`: `<id>_blueprint`, or the
/// id without its tier (`anvil_i` → `anvil_blueprint`).
fn blueprint_of<'a>(item: &Item, catalog: &'a Catalog) -> Option<&'a Item> {
    let id = item.id.as_str();
    let base = id
        .rsplit_once('_')
        .filter(|(_, tier)| matches!(*tier, "i" | "ii" | "iii" | "iv"))
        .map_or(id, |(base, _)| base);
    [format!("{id}_blueprint"), format!("{base}_blueprint")]
        .iter()
        .find_map(|bp| catalog.item(&ItemId::new(bp.as_str())))
}

/// For a blueprint: the items it unlocks.
fn unlocked_by<'a>(item: &'a Item, catalog: &'a Catalog) -> impl Iterator<Item = &'a Item> + 'a {
    let blueprint = item.is_blueprint();
    catalog.items.iter().filter(move |i| {
        blueprint
            && i.details.blueprint_locked
            && blueprint_of(i, catalog).is_some_and(|bp| bp.id == item.id)
    })
}

fn unlocks(item: &Item, catalog: &Catalog, rows: &Rows<'_>) -> Vec<DetailRow> {
    unlocked_by(item, catalog)
        .map(|i| rows.item(&i.id, None, None))
        .collect()
}

fn mod_slots(item: &Item, catalog: &Catalog) -> Vec<DetailRow> {
    item.details
        .mod_slots
        .iter()
        .map(|(slot, mods)| {
            let names: Vec<String> = mods
                .iter()
                .map(|m| {
                    catalog
                        .item(m)
                        .map_or_else(|| m.to_string(), |i| i.name.clone())
                })
                .collect();
            DetailRow {
                name: try_tr(&format!("detail-slot-{slot}"))
                    .unwrap_or_else(|| arclens_core::humanize(slot)),
                note: Some(names.len().to_string()),
                ..DetailRow::default()
            }
            .with_detail(names.join(", "))
        })
        .collect()
}

/// A row with its text continued on the next line.
trait WithDetail {
    fn with_detail(self, detail: String) -> Self;
}

impl WithDetail for DetailRow {
    fn with_detail(mut self, detail: String) -> Self {
        if !detail.is_empty() {
            self.name = format!("{}\n{detail}", self.name);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let item = |id: &str, details: serde_json::Value| -> Item {
            let mut v = serde_json::json!({"id": id, "name": id.to_uppercase(), "value": 100});
            v["details"] = details;
            serde_json::from_value(v).unwrap_or_else(|e| panic!("{id}: {e}"))
        };
        Catalog::new(
            "test",
            vec![
                item(
                    "anvil_i",
                    serde_json::json!({
                        "recipe": [{"item": "gear", "quantity": 2}],
                        "craft_bench": ["weapon_bench"], "station_level": 1,
                        "upgrades_to": "anvil_ii", "blueprint_locked": true
                    }),
                ),
                item(
                    "anvil_ii",
                    serde_json::json!({
                        "upgrade_cost": [{"item": "gear", "quantity": 3}]
                    }),
                ),
                item("anvil_blueprint", serde_json::json!({})),
                item("gear", serde_json::json!({})),
            ],
            Vec::new(),
        )
    }

    #[test]
    fn finds_tiers_products_and_blueprints() {
        let catalog = catalog();
        let anvil = catalog.item(&ItemId::new("anvil_i")).unwrap();
        let gear = catalog.item(&ItemId::new("gear")).unwrap();
        let anvil_ii = catalog.item(&ItemId::new("anvil_ii")).unwrap();
        assert_eq!(
            products(gear, &catalog)
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["anvil_i"]
        );
        assert_eq!(
            previous_tier(anvil_ii, &catalog).unwrap().id.as_str(),
            "anvil_i"
        );
        assert_eq!(
            blueprint_of(anvil, &catalog).unwrap().id.as_str(),
            "anvil_blueprint"
        );
        let refs = referenced(anvil, &catalog);
        assert!(refs.iter().any(|id| id.as_str() == "gear"));
        assert!(refs.iter().any(|id| id.as_str() == "anvil_ii"));
    }
}
