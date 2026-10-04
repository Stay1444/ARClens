//! Provider for the [RaidTheory/arcraiders-data] community dataset (MIT).
//!
//! The dataset is a directory of JSON files:
//!
//! ```text
//! items/<id>.json      one file per item
//! hideout/<id>.json    workshop stations and their upgrade requirements
//! quests/<id>.json     quests and the items they require
//! projects.json        long-running projects and their phase requirements
//! map-events/map-events.json   map condition types (name, icon URL)
//! ```
//!
//! Localised strings are objects keyed by language (`{"en": "...", "de": ...}`),
//! but some files use plain strings; [`Localized`] accepts both.
//!
//! [RaidTheory/arcraiders-data]: https://github.com/RaidTheory/arcraiders-data

use crate::{Catalog, Error};
use arclens_core::{
    Item, ItemDetails, ItemId, ItemQuantity, Project, Quest, Rarity, Requirement, RequirementKind,
    Station, Vendor,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Attribution string stored in [`Catalog::source`].
pub const SOURCE: &str = "RaidTheory/arcraiders-data (MIT)";

/// The dataset's languages ARClens speaks (its keys: `en`, `es`). Names
/// in the ones not shown become aliases, for matching the game's text.
pub const LANGUAGES: [&str; 2] = ["en", "es"];

/// Loads a catalog from a local checkout / extracted archive of the dataset.
#[derive(Debug, Clone)]
pub struct RaidTheoryDir {
    root: PathBuf,
    /// Language of names and descriptions (a [`LANGUAGES`] key).
    lang: String,
}

impl RaidTheoryDir {
    /// Names in English; see [`Self::in_language`].
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lang: "en".to_owned(),
        }
    }

    /// Names and descriptions in `lang` (a [`LANGUAGES`] key), English
    /// where the dataset has no translation.
    #[must_use]
    pub fn in_language(mut self, lang: &str) -> Self {
        lang.clone_into(&mut self.lang);
        self
    }

    /// Reads and joins items, hideout, quests and projects into a [`Catalog`].
    ///
    /// Blocking: call from `spawn_blocking` in async contexts.
    pub fn load(&self) -> Result<Catalog, Error> {
        let raw_items = read_dir_json::<RawItem>(&self.root.join("items"))?;
        let recipes = recipes(&raw_items);
        let mut items: Vec<Item> = raw_items
            .into_iter()
            .map(|raw| raw.into_item(&self.lang))
            .collect();
        items.sort_by(|a, b| a.id.cmp(&b.id));
        self.add_trades(&mut items)?;

        let station_names = self.station_names()?;
        let (mut stations, mut requirements) = self.stations()?;
        let (quests, projects) = self.quests_and_projects(&mut requirements)?;
        let mut ingredient_of = ingredient_map(&items, &recipes);

        for item in &mut items {
            if let Some(reqs) = requirements.remove(&item.id) {
                item.required_for = reqs;
            }
            if let Some(mut products) = ingredient_of.remove(&item.id) {
                products.sort();
                products.dedup();
                item.ingredient_of = products;
            }
        }

        stations.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Catalog::new(SOURCE, items, Vec::new())
            .in_language(&self.lang)
            .with_station_names(station_names)
            .with_stations(stations)
            .with_quests_and_projects(quests, projects)
            .with_event_icons(self.event_icons()?))
    }

    /// Icon URL per map-condition name ([`crate::event_key`]). Optional
    /// file: missing means no icons.
    fn event_icons(&self) -> Result<BTreeMap<String, String>, Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct File {
            #[serde(default)]
            event_types: BTreeMap<String, EventType>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct EventType {
            #[serde(default)]
            display_name: Option<String>,
            #[serde(default)]
            icon: Option<String>,
        }
        let path = self.root.join("map-events").join("map-events.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(e.into()),
        };
        let file: File = serde_json::from_slice(&bytes)?;
        Ok(file
            .event_types
            .into_iter()
            .filter_map(|(id, t)| {
                let icon = t.icon.filter(|u| !u.is_empty())?;
                Some((
                    crate::event_key(t.display_name.as_deref().unwrap_or(&id)),
                    icon,
                ))
            })
            .collect())
    }

    /// Every station's name by id, crafting benches included.
    fn station_names(&self) -> Result<BTreeMap<String, String>, Error> {
        Ok(read_dir_json::<RawStation>(&self.root.join("hideout"))?
            .into_iter()
            .map(|s| (s.id, s.name.resolve(&self.lang)))
            .collect())
    }

    /// Adds `trades.json`'s barter offers to the items' vendors (a trader
    /// already listing the item keeps its own entry). Optional file.
    fn add_trades(&self, items: &mut [Item]) -> Result<(), Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Trade {
            trader: String,
            item_id: String,
            #[serde(default)]
            quantity: Option<u32>,
            cost: RawItemQuantity,
            #[serde(default)]
            daily_limit: Option<u32>,
        }
        let path = self.root.join("trades.json");
        if !path.exists() {
            return Ok(());
        }
        let trades: Vec<Trade> = read_json(&path)?;
        for trade in trades {
            let Some(item) = items.iter_mut().find(|i| i.id.as_str() == trade.item_id) else {
                continue;
            };
            let vendors = &mut item.details.vendors;
            if vendors.iter().any(|v| v.trader == trade.trader) {
                continue;
            }
            let _ = trade.quantity;
            vendors.push(Vendor {
                trader: trade.trader,
                cost: vec![ItemQuantity {
                    item: ItemId::new(trade.cost.item_id),
                    quantity: trade.cost.quantity,
                }],
                limit: trade.daily_limit,
                refresh_seconds: trade.daily_limit.map(|_| 86_400),
                required_level: None,
            });
        }
        Ok(())
    }

    /// Workshop stations, and the upgrade requirements per item.
    fn stations(&self) -> Result<(Vec<Station>, Requirements), Error> {
        let mut stations = Vec::new();
        let mut requirements = Requirements::new();
        for station in read_dir_json::<RawStation>(&self.root.join("hideout"))? {
            let station_name = station.name.resolve(&self.lang);
            for level in &station.levels {
                for req in &level.requirement_item_ids {
                    requirements
                        .entry(ItemId::new(&req.item_id))
                        .or_default()
                        .push(Requirement {
                            kind: RequirementKind::WorkshopUpgrade,
                            name: format!("{station_name} {}", level.level),
                            quantity: req.quantity,
                            station: Some(station.id.clone()),
                            level: Some(level.level),
                        });
                }
            }
            // Upgrades bought with coins only (the stash) never make an
            // item worth keeping: nothing to track.
            let needs_items = station
                .levels
                .iter()
                .any(|level| !level.requirement_item_ids.is_empty());
            if station.max_level > 0 && needs_items {
                stations.push(Station {
                    aliases: station.name.aliases(&self.lang),
                    id: station.id,
                    name: station_name,
                    max_level: station.max_level,
                });
            }
        }
        Ok((stations, requirements))
    }

    /// Adds quest and project requirements to `requirements`, and lists
    /// the quests and (enabled) projects for the progress editor.
    fn quests_and_projects(
        &self,
        requirements: &mut Requirements,
    ) -> Result<(Vec<Quest>, Vec<Project>), Error> {
        let mut push = |item: &str, kind, name: String, quantity, id: &str, level| {
            requirements
                .entry(ItemId::new(item))
                .or_default()
                .push(Requirement {
                    kind,
                    name,
                    quantity,
                    station: Some(id.to_owned()),
                    level,
                });
        };

        let mut quests = Vec::new();
        for quest in read_dir_json::<RawQuest>(&self.root.join("quests"))? {
            let name = quest.name.resolve(&self.lang);
            for req in &quest.required_item_ids {
                push(
                    &req.item_id,
                    RequirementKind::Quest,
                    name.clone(),
                    req.quantity,
                    &quest.id,
                    None,
                );
            }
            quests.push(Quest {
                aliases: quest.name.aliases(&self.lang),
                needs_items: !quest.required_item_ids.is_empty(),
                id: quest.id,
                name,
                trader: quest.trader,
                previous: quest.previous_quest_ids,
            });
        }
        // Per trader, in the order the game hands them out.
        let depth = quest_depths(&quests);
        quests.sort_by(|a, b| {
            (&a.trader, depth.get(&a.id), &a.name).cmp(&(&b.trader, depth.get(&b.id), &b.name))
        });

        let mut projects = Vec::new();
        let projects_path = self.root.join("projects.json");
        if projects_path.exists() {
            let raw: Vec<RawProject> = read_json(&projects_path)?;
            for project in raw.into_iter().filter(|p| !p.disabled) {
                let project_name = project.name.resolve(&self.lang);
                let mut phases = Vec::new();
                // Phases are listed in order; their number is their place.
                for (phase, number) in project.phases.into_iter().zip(1u32..) {
                    let phase_name = phase.name.resolve(&self.lang);
                    for req in phase.requirement_item_ids {
                        push(
                            &req.item_id,
                            RequirementKind::Project,
                            format!("{project_name}: {phase_name}"),
                            req.quantity,
                            &project.id,
                            Some(number),
                        );
                    }
                    phases.push(phase_name);
                }
                projects.push(Project {
                    id: project.id,
                    name: project_name,
                    phases,
                });
            }
        }
        Ok((quests, projects))
    }
}

/// How many quests come before each one (its longest chain of
/// prerequisites). Cycles, which the data shouldn't have, are cut.
fn quest_depths(quests: &[Quest]) -> HashMap<String, u32> {
    fn depth<'a>(
        id: &'a str,
        by_id: &HashMap<&'a str, &'a Quest>,
        memo: &mut HashMap<String, u32>,
        visiting: &mut Vec<&'a str>,
    ) -> u32 {
        if let Some(&d) = memo.get(id) {
            return d;
        }
        if visiting.contains(&id) {
            return 0;
        }
        visiting.push(id);
        let d = by_id.get(id).map_or(0, |q| {
            q.previous
                .iter()
                .map(|p| depth(p, by_id, memo, visiting) + 1)
                .max()
                .unwrap_or(0)
        });
        visiting.pop();
        memo.insert(id.to_owned(), d);
        d
    }
    let by_id: HashMap<&str, &Quest> = quests.iter().map(|q| (q.id.as_str(), q)).collect();
    let mut memo = HashMap::new();
    for quest in quests {
        depth(&quest.id, &by_id, &mut memo, &mut Vec::new());
    }
    memo
}

type Requirements = HashMap<ItemId, Vec<Requirement>>;

/// Product id → ingredient ids, from crafting recipes and weapon tier
/// upgrades (the cost sits on the source tier; the product is `upgradesTo`).
fn recipes(raw_items: &[RawItem]) -> Vec<(ItemId, Vec<ItemId>)> {
    raw_items
        .iter()
        .flat_map(|raw| {
            let craft = raw
                .recipe
                .as_ref()
                .map(|r| (ItemId::new(&raw.id), r.keys().map(ItemId::new).collect()));
            let upgrade =
                raw.upgrade_cost
                    .as_ref()
                    .zip(raw.upgrades_to.as_ref())
                    .map(|(cost, target)| {
                        (ItemId::new(target), cost.keys().map(ItemId::new).collect())
                    });
            craft.into_iter().chain(upgrade)
        })
        .collect()
}

/// Ingredient id → names of the items it is used to make.
fn ingredient_map(
    items: &[Item],
    recipes: &[(ItemId, Vec<ItemId>)],
) -> HashMap<ItemId, Vec<String>> {
    let names: HashMap<&ItemId, &str> = items.iter().map(|i| (&i.id, i.name.as_str())).collect();
    let mut map: HashMap<ItemId, Vec<String>> = HashMap::new();
    for (product, inputs) in recipes {
        let Some(name) = names.get(product) else {
            continue;
        };
        for input in inputs {
            map.entry(input.clone())
                .or_default()
                .push((*name).to_owned());
        }
    }
    map
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, Error> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Parse {
        path: path.to_owned(),
        source,
    })
}

/// Parses every `*.json` file in `dir`, in file-name order. A missing
/// directory yields an empty list so partial datasets still load.
fn read_dir_json<T: for<'de> Deserialize<'de>>(dir: &Path) -> Result<Vec<T>, Error> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::Io(e)),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths.iter().map(|p| read_json(p)).collect()
}

/// A string that is either plain or a per-language map.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Localized {
    Plain(String),
    ByLang(BTreeMap<String, String>),
}

impl Localized {
    /// The text in `lang`, else English, else any.
    fn resolve(&self, lang: &str) -> String {
        match self {
            Self::Plain(s) => s.clone(),
            Self::ByLang(map) => map
                .get(lang)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| map.get("en"))
                .or_else(|| map.values().next())
                .cloned()
                .unwrap_or_default(),
        }
    }

    /// The text in the other [`LANGUAGES`], where it differs.
    fn aliases(&self, lang: &str) -> Vec<String> {
        let shown = self.resolve(lang);
        let mut out: Vec<String> = LANGUAGES
            .iter()
            .filter(|&&other| other != lang)
            .map(|other| self.resolve(other))
            .filter(|name| !name.is_empty() && *name != shown)
            .collect();
        out.dedup();
        out
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawItem {
    id: String,
    name: Localized,
    #[serde(default)]
    description: Option<Localized>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    value: Option<u32>,
    #[serde(default)]
    weight_kg: Option<f32>,
    #[serde(default)]
    stack_size: Option<u32>,
    #[serde(default)]
    recycles_into: BTreeMap<String, u32>,
    #[serde(default)]
    salvages_into: BTreeMap<String, u32>,
    #[serde(default)]
    recipe: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    upgrade_cost: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    upgrades_to: Option<String>,
    #[serde(default)]
    image_filename: Option<String>,
    /// Effects and stats: label per language plus `value`.
    #[serde(default, deserialize_with = "null_as_default")]
    effects: BTreeMap<String, Option<BTreeMap<String, serde_json::Value>>>,
    #[serde(default)]
    craft_bench: Option<serde_json::Value>,
    #[serde(default)]
    station_level_required: Option<u32>,
    #[serde(default)]
    craft_quantity: Option<u32>,
    #[serde(default, deserialize_with = "null_as_default")]
    repair_cost: BTreeMap<String, u32>,
    #[serde(default)]
    repair_durability: Option<f32>,
    #[serde(default, deserialize_with = "null_as_default")]
    vendors: Vec<RawVendor>,
    #[serde(default)]
    found_in: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    compatible_with: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    mod_slots: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "null_as_default")]
    blueprint_locked: bool,
    #[serde(default, deserialize_with = "null_as_default")]
    quest_item: bool,
    #[serde(default)]
    added_in: Option<String>,
    #[serde(default)]
    tip: Option<Localized>,
    /// Weapon stats (numbers; some null).
    #[serde(flatten)]
    numbers: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawVendor {
    trader: String,
    #[serde(default)]
    cost: BTreeMap<String, u32>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    refresh_seconds: Option<u32>,
    #[serde(default)]
    required_level: Option<u32>,
}

/// Numeric stats outside `effects`, in display order: data key and the
/// label in each language ARClens speaks.
const NUMBER_STATS: [(&str, &str, &str); 13] = [
    ("damage", "Damage", "Daño"),
    ("fireRate", "Fire Rate", "Cadencia"),
    ("range", "Range", "Alcance"),
    ("stability", "Stability", "Estabilidad"),
    ("agility", "Agility", "Agilidad"),
    ("stealth", "Stealth", "Sigilo"),
    ("durability", "Durability", "Durabilidad"),
    (
        "damageMitigation",
        "Damage Mitigation",
        "Mitigación de daño",
    ),
    ("shieldCharge", "Shield Charge", "Carga de escudo"),
    (
        "movementSpeedModifier",
        "Movement Speed",
        "Velocidad de movimiento",
    ),
    (
        "increasedFireRate",
        "Increased Fire Rate",
        "Cadencia aumentada",
    ),
    (
        "reducedReloadTime",
        "Reduced Reload Time",
        "Recarga más rápida",
    ),
    (
        "reducedDurabilityBurnRate",
        "Reduced Durability Burn",
        "Menor desgaste de durabilidad",
    ),
];

/// `null` read as the type's default (the dataset writes `null` for some
/// empty lists and maps).
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A JSON number or string as display text (`8`, `33.33`, `"10s"`).
fn value_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(|f| {
            if f.fract() == 0.0 {
                format!("{f:.0}")
            } else {
                format!("{}", (f * 100.0).round() / 100.0)
            }
        }),
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

impl RawItem {
    /// The detailed view's data, labels in `lang`.
    fn details(&mut self, lang: &str) -> ItemDetails {
        let mut stats: Vec<(String, String)> = std::mem::take(&mut self.effects)
            .into_iter()
            .filter_map(|(key, labels)| {
                let labels = labels?;
                let value = labels.get("value").and_then(value_text)?;
                let label = labels
                    .get(lang)
                    .or_else(|| labels.get("en"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&key)
                    .to_owned();
                Some((label, value))
            })
            .collect();
        for (key, en, es) in NUMBER_STATS {
            if let Some(value) = self.numbers.get(key).and_then(value_text) {
                let label = if lang == "es" { es } else { en };
                stats.push((label.to_owned(), value));
            }
        }
        let craft_bench = match self.craft_bench.take() {
            Some(serde_json::Value::String(s)) => vec![s],
            Some(serde_json::Value::Array(list)) => list
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        ItemDetails {
            stats,
            recipe: quantities(self.recipe.clone().unwrap_or_default()),
            craft_quantity: self.craft_quantity.filter(|&q| q > 1),
            craft_bench,
            station_level: self.station_level_required,
            upgrades_to: self.upgrades_to.clone().map(ItemId::new),
            upgrade_cost: quantities(self.upgrade_cost.clone().unwrap_or_default()),
            repair_cost: quantities(std::mem::take(&mut self.repair_cost)),
            repair_durability: self.repair_durability,
            vendors: std::mem::take(&mut self.vendors)
                .into_iter()
                .map(|v| Vendor {
                    trader: v.trader,
                    cost: quantities(v.cost),
                    limit: v.limit,
                    refresh_seconds: v.refresh_seconds,
                    required_level: v.required_level,
                })
                .collect(),
            found_in: self
                .found_in
                .as_deref()
                .map(|f| {
                    f.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            compatible_with: std::mem::take(&mut self.compatible_with),
            mod_slots: std::mem::take(&mut self.mod_slots)
                .into_iter()
                .map(|(slot, mods)| (slot, mods.into_iter().map(ItemId::new).collect()))
                .collect(),
            blueprint_locked: self.blueprint_locked,
            quest_item: self.quest_item,
            added_in: self.added_in.take(),
            tip: self
                .tip
                .as_ref()
                .map(|t| t.resolve(lang))
                .filter(|t| !t.is_empty()),
        }
    }
}

impl RawItem {
    fn into_item(mut self, lang: &str) -> Item {
        let details = self.details(lang);
        Item {
            details,
            id: ItemId::new(self.id),
            name: self.name.resolve(lang),
            aliases: self.name.aliases(lang),
            description: self
                .description
                .map(|d| d.resolve(lang))
                .filter(|d| !d.is_empty()),
            rarity: self.rarity.as_deref().and_then(parse_rarity),
            category: self.kind,
            value: self.value,
            weight: self.weight_kg,
            stack_size: self.stack_size,
            recycles_into: quantities(self.recycles_into),
            salvages_into: quantities(self.salvages_into),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: self.image_filename.filter(|u| u.starts_with("https://")),
        }
    }
}

fn quantities(map: BTreeMap<String, u32>) -> Vec<ItemQuantity> {
    map.into_iter()
        .map(|(item, quantity)| ItemQuantity {
            item: ItemId::new(item),
            quantity,
        })
        .collect()
}

fn parse_rarity(raw: &str) -> Option<Rarity> {
    match raw.to_ascii_lowercase().as_str() {
        "common" => Some(Rarity::Common),
        "uncommon" => Some(Rarity::Uncommon),
        "rare" => Some(Rarity::Rare),
        "epic" => Some(Rarity::Epic),
        "legendary" => Some(Rarity::Legendary),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawItemQuantity {
    item_id: String,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStation {
    id: String,
    name: Localized,
    #[serde(default)]
    max_level: u32,
    #[serde(default)]
    levels: Vec<RawStationLevel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStationLevel {
    level: u32,
    #[serde(default)]
    requirement_item_ids: Vec<RawItemQuantity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawQuest {
    id: String,
    name: Localized,
    #[serde(default)]
    trader: String,
    #[serde(default)]
    previous_quest_ids: Vec<String>,
    #[serde(default)]
    required_item_ids: Vec<RawItemQuantity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProject {
    id: String,
    name: Localized,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    phases: Vec<RawProjectPhase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProjectPhase {
    name: Localized,
    #[serde(default)]
    requirement_item_ids: Vec<RawItemQuantity>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::{Verdict, advise};

    fn fixture() -> Catalog {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raidtheory");
        RaidTheoryDir::new(root).load().expect("fixture loads")
    }

    #[test]
    fn names_come_in_the_chosen_language_with_the_others_as_aliases() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raidtheory");
        let es = RaidTheoryDir::new(&root).in_language("es").load().unwrap();
        assert_eq!(es.lang, "es");
        let guitar = es.item(&ItemId::new("acoustic_guitar")).unwrap();
        assert_eq!(guitar.name, "Guitarra acústica");
        assert_eq!(guitar.aliases, ["Acoustic Guitar"]);
        assert_eq!(
            guitar.description.as_deref(),
            Some("Una guitarra acústica que se puede tocar.")
        );
        // No Spanish name: English, and no alias.
        let wires = es.item(&ItemId::new("wires")).unwrap();
        assert_eq!(wires.name, "Wires");
        assert_eq!(wires.aliases, Vec::<String>::new());
        // The game's text matches in either language, accents or not.
        for text in ["ACOUSTIC GUITAR", "GUITARRA ACUSTICA", "GUITARRA ACÚSTICA"] {
            let (item, _) = crate::match_name(text, &es.items).unwrap();
            assert_eq!(item.id.as_str(), "acoustic_guitar", "{text}");
        }

        let en = fixture();
        let guitar = en.item(&ItemId::new("acoustic_guitar")).unwrap();
        assert_eq!(guitar.name, "Acoustic Guitar");
        assert_eq!(guitar.aliases, ["Guitarra acústica"]);
    }

    #[test]
    fn reads_the_details_for_the_detailed_view() {
        let catalog = fixture();
        let anvil = &catalog.item(&ItemId::new("anvil_i")).unwrap().details;
        assert_eq!(
            anvil.stats,
            [
                ("ARC Armor Penetration".to_owned(), "Strong".to_owned()),
                ("Ammo Type".to_owned(), "Heavy Ammo".to_owned()),
                ("Damage".to_owned(), "40".to_owned()),
                ("Fire Rate".to_owned(), "16.67".to_owned()),
            ]
        );
        assert_eq!(anvil.craft_bench, ["weapon_bench"]);
        assert_eq!(anvil.station_level, Some(1));
        assert_eq!(anvil.recipe.len(), 2);
        assert_eq!(anvil.upgrades_to, Some(ItemId::new("anvil_ii")));
        assert!(anvil.blueprint_locked);
        assert_eq!(anvil.mod_slots[0].0, "muzzle");
        assert_eq!(anvil.added_in.as_deref(), Some("base"));

        let guitar = &catalog
            .item(&ItemId::new("acoustic_guitar"))
            .unwrap()
            .details;
        assert_eq!(guitar.repair_cost.len(), 2);
        assert_eq!(guitar.repair_durability, Some(0.5));
        assert_eq!(guitar.found_in, ["Residential", "Commercial"]);
        let shani = &guitar.vendors[0];
        assert_eq!(shani.trader, "Shani");
        assert_eq!(shani.cost[0].item.as_str(), "creds");
        assert_eq!((shani.limit, shani.required_level), (Some(1), Some(12)));

        // Labels follow the language.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raidtheory");
        let es = RaidTheoryDir::new(root).in_language("es").load().unwrap();
        let stats = &es.item(&ItemId::new("anvil_i")).unwrap().details.stats;
        assert!(stats.contains(&("Tipo de munición".to_owned(), "Heavy Ammo".to_owned())));
        assert!(stats.contains(&("Daño".to_owned(), "40".to_owned())));
    }

    /// The whole dataset parses: `ARCLENS_RAIDTHEORY_DIR=… cargo test -p
    /// arclens-data -- --ignored loads_the_real_dataset`.
    #[test]
    #[ignore = "needs a checkout of RaidTheory/arcraiders-data"]
    fn loads_the_real_dataset() {
        let dir = std::env::var("ARCLENS_RAIDTHEORY_DIR").expect("ARCLENS_RAIDTHEORY_DIR");
        for lang in LANGUAGES {
            let catalog = RaidTheoryDir::new(&dir)
                .in_language(lang)
                .load()
                .unwrap_or_else(|e| panic!("{lang}: {e}"));
            assert!(catalog.items.len() > 400, "{lang}");
        }
    }

    #[test]
    fn reads_event_icons() {
        let catalog = fixture();
        assert_eq!(
            catalog.event_icon("Night Raid"),
            Some("https://cdn.arctracker.io/map-events/night_raid.png")
        );
        assert!(catalog.event_icon("cold snap").is_some());
        assert_eq!(catalog.event_icon("No Icon"), None);
    }

    #[test]
    fn quest_depth_follows_the_longest_chain() {
        let quest = |id: &str, previous: &[&str]| Quest {
            id: id.into(),
            name: id.into(),
            aliases: Vec::new(),
            trader: String::new(),
            previous: previous.iter().map(|&p| p.into()).collect(),
            needs_items: false,
        };
        let quests = [
            quest("a", &[]),
            quest("b", &["a"]),
            quest("c", &["a", "b"]),
            quest("loop1", &["loop2"]),
            quest("loop2", &["loop1"]),
        ];
        let depths = quest_depths(&quests);
        assert_eq!((depths["a"], depths["b"], depths["c"]), (0, 1, 2));
        assert!(depths["loop1"] <= 2 && depths["loop2"] <= 2);
    }

    #[test]
    fn lists_quests_and_enabled_projects_with_ids_on_requirements() {
        let catalog = fixture();
        assert_eq!(catalog.quests.len(), 1);
        assert_eq!(catalog.quests[0].id, "wired_up");
        assert_eq!(catalog.quests[0].trader, "Shani");
        assert!(catalog.quests[0].needs_items);
        assert_eq!(catalog.projects.len(), 1);
        assert_eq!(catalog.projects[0].phases, ["Foundation"]);

        let wires = catalog.item(&ItemId::new("wires")).unwrap();
        let quest = wires
            .required_for
            .iter()
            .find(|r| r.kind == RequirementKind::Quest)
            .unwrap();
        assert_eq!(quest.station.as_deref(), Some("wired_up"));
        let project = wires
            .required_for
            .iter()
            .find(|r| r.kind == RequirementKind::Project)
            .unwrap();
        assert_eq!(
            (project.station.as_deref(), project.level),
            (Some("expedition"), Some(1))
        );

        // Once both are done, wires are no longer needed for them.
        let mut progress = arclens_core::Progress::default();
        progress.quests_done.insert("wired_up".into());
        progress.projects.insert("expedition".into(), 1);
        assert!(!progress.needs(quest));
        assert!(!progress.needs(project));
    }

    #[test]
    fn parses_localized_and_plain_names() {
        let catalog = fixture();
        let guitar = catalog.item(&ItemId::new("acoustic_guitar")).unwrap();
        assert_eq!(guitar.name, "Acoustic Guitar");
        assert_eq!(guitar.rarity, Some(Rarity::Legendary));
        assert_eq!(guitar.value, Some(7000));
        assert_eq!(guitar.recycles_into.len(), 2);
    }

    #[test]
    fn joins_requirements_from_hideout_quests_and_projects() {
        let catalog = fixture();
        let wires = catalog.item(&ItemId::new("wires")).unwrap();
        let kinds: Vec<_> = wires.required_for.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&RequirementKind::WorkshopUpgrade));
        assert!(kinds.contains(&RequirementKind::Quest));
        assert!(kinds.contains(&RequirementKind::Project));
        assert!(
            wires
                .required_for
                .iter()
                .any(|r| r.name == "Gear Bench 1" && r.quantity == 5)
        );
    }

    #[test]
    fn advice_over_loaded_catalog() {
        let catalog = fixture();
        let guitar = catalog.item(&ItemId::new("acoustic_guitar")).unwrap();
        // 7000 sell vs. 4×50 + 6×200 = 1400 recycle.
        let advice = advise(guitar, |id| catalog.item(id));
        assert_eq!(advice.verdict, Verdict::Sell);
        assert_eq!(advice.recycle_value, Some(1400));
    }
}
