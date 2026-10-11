//! MetaForge's item and quest database (`/items`, `/quests`), used to fill
//! what RaidTheory's dataset doesn't have yet: items and quests from a
//! patch it hasn't caught up with (Frozen Trail, 2.0, came out on
//! 2026-10-08; RaidTheory's last update was for 1.42), and where an item
//! drops when RaidTheory doesn't say.
//!
//! RaidTheory stays the primary source: its records win, MetaForge only
//! adds. Terms as in [`crate::metaforge`]: credited in the README, every
//! response cached (the result is stored in the catalogue cache).
//!
//! Shapes from recorded responses in
//! [Getty/p5-www-metaforge](https://github.com/Getty/p5-www-metaforge)
//! `t/fixtures/{items,quests}.json` (2026-10-03):
//! `{"data": [...], "pagination": {"page", "limit", "total", "totalPages",
//! "hasNextPage"}}`. The endpoints take `page` and `limit`.

use crate::{Catalog, Error, normalize_name};
use arclens_core::{Item, ItemDetails, ItemId, ItemQuantity, Quest, Requirement, RequirementKind};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub const ITEMS_URL: &str = "https://metaforge.app/api/arc-raiders/items";
pub const QUESTS_URL: &str = "https://metaforge.app/api/arc-raiders/quests";
/// What items filled from MetaForge carry in [`ItemDetails::source`].
pub const SOURCE: &str = "MetaForge";
/// Records asked for per page.
const PAGE_SIZE: u32 = 100;
/// Stop after this many pages, whatever `hasNextPage` says (~600 items
/// and ~120 quests in October 2026).
const MAX_PAGES: u32 = 40;

/// One page of a list endpoint.
#[derive(Debug, Deserialize)]
struct Page<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    #[serde(default)]
    has_next_page: bool,
}

/// An item as MetaForge lists it (the fields ARClens uses).
#[derive(Debug, Clone, Deserialize)]
pub struct RawItem {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    item_type: Option<String>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    value: Option<u32>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    loot_area: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    dropped_by: Vec<Dropper>,
    #[serde(default, deserialize_with = "null_as_default")]
    recycle_components: Vec<Component>,
    #[serde(default, deserialize_with = "null_as_default")]
    components: Vec<Component>,
    #[serde(default, deserialize_with = "null_as_default")]
    sold_by: Vec<Seller>,
    #[serde(default)]
    workbench: Option<String>,
    #[serde(default)]
    flavor_text: Option<String>,
    #[serde(default)]
    stat_block: Option<StatBlock>,
    /// When MetaForge added it (RFC 3339).
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Dropper {
    arc: Option<Named>,
}

#[derive(Debug, Clone, Deserialize)]
struct Named {
    #[serde(default)]
    id: Option<String>,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Component {
    component: Named,
    quantity: Quantity,
}

#[derive(Debug, Clone, Deserialize)]
struct Seller {
    trader_name: String,
    #[serde(default)]
    price: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatBlock {
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default)]
    stack_size: Option<u32>,
    #[serde(default)]
    value: Option<u32>,
}

/// A quest as MetaForge lists it.
#[derive(Debug, Clone, Deserialize)]
pub struct RawQuest {
    id: String,
    name: String,
    #[serde(default)]
    trader_name: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    required_items: Vec<QuestItem>,
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestItem {
    item: Named,
    quantity: Quantity,
}

/// A count, sent as a number or a numeric string (`"5"` in quest rewards).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Quantity {
    Number(u32),
    Text(String),
}

impl Quantity {
    fn get(&self) -> u32 {
        match self {
            Self::Number(n) => *n,
            Self::Text(s) => s.trim().parse().unwrap_or(1),
        }
    }
}

fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Parses one `/items` page: its items, and whether another page follows.
pub fn parse_items_page(bytes: &[u8]) -> Result<(Vec<RawItem>, bool), Error> {
    let page: Page<RawItem> = serde_json::from_slice(bytes)?;
    let more = page.pagination.is_some_and(|p| p.has_next_page);
    Ok((page.data, more))
}

/// Parses one `/quests` page: its quests, and whether another page follows.
pub fn parse_quests_page(bytes: &[u8]) -> Result<(Vec<RawQuest>, bool), Error> {
    let page: Page<RawQuest> = serde_json::from_slice(bytes)?;
    let more = page.pagination.is_some_and(|p| p.has_next_page);
    Ok((page.data, more))
}

/// URL of page `page` (from 1) of a list endpoint.
pub fn page_url(base: &str, page: u32) -> String {
    format!("{base}?page={page}&limit={PAGE_SIZE}")
}

/// Everything MetaForge lists, fetched page by page.
#[derive(Debug, Clone, Default)]
pub struct Database {
    pub items: Vec<RawItem>,
    pub quests: Vec<RawQuest>,
}

/// A page parser: the records and whether another page follows.
type ParsePage<T> = fn(&[u8]) -> Result<(Vec<T>, bool), Error>;

/// Fetches every page of items and quests, one request at a time.
pub async fn fetch(client: &reqwest::Client) -> Result<Database, Error> {
    async fn all<T>(
        client: &reqwest::Client,
        base: &str,
        parse: ParsePage<T>,
    ) -> Result<Vec<T>, Error> {
        let mut out = Vec::new();
        for page in 1..=MAX_PAGES {
            let bytes = client
                .get(page_url(base, page))
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            let (records, more) = parse(&bytes)?;
            let empty = records.is_empty();
            out.extend(records);
            if !more || empty {
                break;
            }
        }
        Ok(out)
    }
    Ok(Database {
        items: all(client, ITEMS_URL, parse_items_page).await?,
        quests: all(client, QUESTS_URL, parse_quests_page).await?,
    })
}

/// MetaForge id → RaidTheory-style id: `heavy-gun-parts` → `heavy_gun_parts`.
/// MetaForge calls some plain materials `…-recipe` (`wires-recipe` is
/// Wires) and blueprints `…-recipe` too (`anvil-recipe`, RaidTheory's
/// `anvil_blueprint`).
fn local_id(id: &str, blueprint: bool) -> ItemId {
    let id = id.trim().to_lowercase();
    let id = match id.strip_suffix("-recipe") {
        Some(base) if blueprint => format!("{base}-blueprint"),
        Some(base) => base.to_owned(),
        None => id,
    };
    ItemId::new(id.replace('-', "_"))
}

fn is_blueprint(item_type: Option<&str>) -> bool {
    item_type.is_some_and(|t| t.eq_ignore_ascii_case("blueprint"))
}

fn parse_rarity(raw: &str) -> Option<arclens_core::Rarity> {
    use arclens_core::Rarity;
    match raw.trim().to_ascii_lowercase().as_str() {
        "common" => Some(Rarity::Common),
        "uncommon" => Some(Rarity::Uncommon),
        "rare" => Some(Rarity::Rare),
        "epic" => Some(Rarity::Epic),
        "legendary" => Some(Rarity::Legendary),
        _ => None,
    }
}

/// The last game update RaidTheory's dataset covers (1.42, its update of
/// 2026-08-19). MetaForge records created before it that RaidTheory has
/// under another name would otherwise show twice, so only newer ones are
/// added. Records RaidTheory adds later match by name and stop being added.
pub const NEW_SINCE: &str = "2026-08-19";

/// Whether a record created at `created_at` (RFC 3339; compared as text,
/// dates first) is from `since` (`YYYY-MM-DD`) or later. Records without a
/// date count as new.
fn is_new(created_at: Option<&str>, since: Option<&str>) -> bool {
    match (created_at.map(str::trim).filter(|c| c.len() >= 10), since) {
        (Some(created), Some(since)) => created[..10] >= *since,
        _ => true,
    }
}

/// How many records [`Catalog::supplement`] added or completed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Supplemented {
    pub items: usize,
    pub quests: usize,
    /// Catalogue items that got a "where to find it" from MetaForge.
    pub found_in: usize,
}

/// Resolves item references to catalogue ids, by name first, then by id.
struct Resolver {
    by_name: HashMap<String, ItemId>,
    ids: HashSet<ItemId>,
}

impl Resolver {
    fn new(items: &[Item]) -> Self {
        let mut by_name = HashMap::new();
        for item in items {
            for name in item.names() {
                by_name
                    .entry(normalize_name(name))
                    .or_insert_with(|| item.id.clone());
            }
        }
        Self {
            by_name,
            ids: items.iter().map(|i| i.id.clone()).collect(),
        }
    }

    /// The catalogue item named `name` or with MetaForge id `id`.
    fn find(&self, name: &str, id: Option<&str>, blueprint: bool) -> Option<ItemId> {
        self.by_name
            .get(&normalize_name(name))
            .cloned()
            .or_else(|| {
                let id = local_id(id?, blueprint);
                self.ids.contains(&id).then_some(id)
            })
    }

    /// Like [`Self::find`], falling back to the local form of the id.
    fn resolve(&self, named: &Named) -> ItemId {
        let blueprint = named.name.to_lowercase().ends_with(" blueprint");
        self.find(&named.name, named.id.as_deref(), blueprint)
            .unwrap_or_else(|| local_id(named.id.as_deref().unwrap_or(&named.name), blueprint))
    }

    fn add(&mut self, item: &Item) {
        self.by_name
            .entry(normalize_name(&item.name))
            .or_insert_with(|| item.id.clone());
        self.ids.insert(item.id.clone());
    }
}

fn quantities(resolver: &Resolver, list: &[Component]) -> Vec<ItemQuantity> {
    list.iter()
        .map(|c| ItemQuantity {
            item: resolver.resolve(&c.component),
            quantity: c.quantity.get(),
        })
        .collect()
}

fn droppers(raw: &RawItem) -> Vec<String> {
    let mut out: Vec<String> = raw
        .dropped_by
        .iter()
        .filter_map(|d| d.arc.as_ref())
        .map(|arc| arc.name.trim().to_owned())
        .filter(|n| !n.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn found_in(raw: &RawItem) -> Vec<String> {
    raw.loot_area
        .as_deref()
        .map(|a| {
            a.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

impl RawItem {
    fn into_item(self, resolver: &Resolver) -> Item {
        let blueprint = is_blueprint(self.item_type.as_deref());
        let stats = self.stat_block.clone().unwrap_or_default();
        let workbench = self.workbench.as_deref().map_or("", str::trim);
        let (bench, level) = match workbench.rsplit_once(' ') {
            Some((name, level)) if level.parse::<u32>().is_ok() => (name, level.parse().ok()),
            _ => (workbench, None),
        };
        let details = ItemDetails {
            recipe: quantities(resolver, &self.components),
            craft_bench: (!bench.is_empty())
                .then(|| bench.to_owned())
                .into_iter()
                .collect(),
            station_level: level,
            vendors: self
                .sold_by
                .iter()
                .map(|s| arclens_core::Vendor {
                    trader: s.trader_name.clone(),
                    cost: s
                        .price
                        .map(|price| ItemQuantity {
                            item: ItemId::new("coins"),
                            quantity: price,
                        })
                        .into_iter()
                        .collect(),
                    limit: None,
                    refresh_seconds: None,
                    required_level: None,
                })
                .collect(),
            found_in: found_in(&self),
            dropped_by: droppers(&self),
            tip: self.flavor_text.clone().filter(|t| !t.trim().is_empty()),
            source: Some(SOURCE.to_owned()),
            ..ItemDetails::default()
        };
        Item {
            id: local_id(&self.id, blueprint),
            name: self.name.trim().to_owned(),
            aliases: Vec::new(),
            description: self.description.filter(|d| !d.trim().is_empty()),
            rarity: self.rarity.as_deref().and_then(parse_rarity),
            category: self.item_type.filter(|t| !t.trim().is_empty()),
            value: self.value.or(stats.value).filter(|&v| v > 0),
            weight: stats.weight.filter(|&w| w > 0.0),
            stack_size: stats.stack_size.filter(|&s| s > 0),
            recycles_into: quantities(resolver, &self.recycle_components),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: self.icon.filter(|u| u.starts_with("https://")),
            details,
        }
    }
}

fn squash(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

impl Catalog {
    /// Adds what MetaForge has and the catalogue lacks: items and quests
    /// it doesn't know (matched by name in any language, then by id), the
    /// items those quests need, and where an item drops when the catalogue
    /// doesn't say. Never changes a record the catalogue already has
    /// otherwise. With `new_since` (`YYYY-MM-DD`, e.g. [`NEW_SINCE`]), only
    /// items and quests MetaForge added from that day on are added.
    pub fn supplement(&mut self, db: Database, new_since: Option<&str>) -> Supplemented {
        let mut done = Supplemented::default();
        let mut resolver = Resolver::new(&self.items);
        self.supplement_items(db.items, new_since, &mut resolver, &mut done);
        self.supplement_quests(db.quests, new_since, &resolver, &mut done);
        if done.items > 0 {
            self.items.sort_by(|a, b| a.id.cmp(&b.id));
        }
        self.reindex();
        self.fill_ingredient_of();
        done
    }

    fn supplement_items(
        &mut self,
        raw_items: Vec<RawItem>,
        new_since: Option<&str>,
        resolver: &mut Resolver,
        done: &mut Supplemented,
    ) {
        // Where to find what we already have.
        let mut new_items = Vec::new();
        for raw in raw_items {
            let blueprint = is_blueprint(raw.item_type.as_deref());
            match resolver.find(&raw.name, Some(&raw.id), blueprint) {
                Some(id) => {
                    let Some(&index) = self.index.get(&id) else {
                        continue;
                    };
                    let details = &mut self.items[index].details;
                    let mut completed = false;
                    if details.found_in.is_empty() {
                        details.found_in = found_in(&raw);
                        completed |= !details.found_in.is_empty();
                    }
                    if details.dropped_by.is_empty() {
                        details.dropped_by = droppers(&raw);
                        completed |= !details.dropped_by.is_empty();
                    }
                    done.found_in += usize::from(completed);
                }
                None if !raw.name.trim().is_empty()
                    && is_new(raw.created_at.as_deref(), new_since) =>
                {
                    new_items.push(raw);
                }
                None => {}
            }
        }
        // Names first, so new items can refer to each other.
        let mut seen = HashSet::new();
        new_items.retain(|raw| seen.insert(normalize_name(&raw.name)));
        for raw in &new_items {
            let blueprint = is_blueprint(raw.item_type.as_deref());
            let mut stub = Item {
                id: local_id(&raw.id, blueprint),
                name: raw.name.trim().to_owned(),
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
            };
            if resolver.ids.contains(&stub.id) {
                // Same id, other name: keep both apart.
                stub.id = ItemId::new(format!("{}_mf", stub.id));
            }
            resolver.add(&stub);
        }
        for raw in new_items {
            let mut item = raw.into_item(resolver);
            item.id = resolver
                .find(&item.name, None, false)
                .unwrap_or(item.id.clone());
            self.items.push(item);
            done.items += 1;
        }
    }

    /// Quests the catalogue doesn't have, and what they need.
    fn supplement_quests(
        &mut self,
        raw_quests: Vec<RawQuest>,
        new_since: Option<&str>,
        resolver: &Resolver,
        done: &mut Supplemented,
    ) {
        let known: HashSet<String> = self
            .quests
            .iter()
            .flat_map(|q| q.names().map(squash).collect::<Vec<_>>())
            .chain(self.quests.iter().map(|q| squash(&q.id)))
            .collect();
        let mut requirements: HashMap<ItemId, Vec<Requirement>> = HashMap::new();
        for raw in raw_quests {
            let name = raw.name.trim().to_owned();
            if name.is_empty()
                || known.contains(&squash(&name))
                || known.contains(&squash(&raw.id))
                || !is_new(raw.created_at.as_deref(), new_since)
            {
                continue;
            }
            let id = raw.id.trim().replace('-', "_");
            for need in &raw.required_items {
                requirements
                    .entry(resolver.resolve(&need.item))
                    .or_default()
                    .push(Requirement {
                        kind: RequirementKind::Quest,
                        name: name.clone(),
                        quantity: need.quantity.get(),
                        station: Some(id.clone()),
                        level: None,
                    });
            }
            self.quests.push(Quest {
                id,
                name,
                aliases: Vec::new(),
                trader: raw.trader_name.unwrap_or_default(),
                previous: Vec::new(),
                needs_items: !raw.required_items.is_empty(),
            });
            done.quests += 1;
        }
        for item in &mut self.items {
            if let Some(reqs) = requirements.remove(&item.id) {
                item.required_for.extend(reqs);
            }
        }
        if done.quests > 0 {
            self.quests
                .sort_by(|a, b| (&a.trader, &a.name).cmp(&(&b.trader, &b.name)));
        }
    }

    /// Names of the items each item is an ingredient of, for items whose
    /// recipes came from MetaForge (RaidTheory's are filled on load).
    fn fill_ingredient_of(&mut self) {
        let mut uses: HashMap<ItemId, Vec<String>> = HashMap::new();
        for item in &self.items {
            if item.details.source.is_none() {
                continue;
            }
            for input in &item.details.recipe {
                uses.entry(input.item.clone())
                    .or_default()
                    .push(item.name.clone());
            }
        }
        for item in &mut self.items {
            if let Some(products) = uses.remove(&item.id) {
                item.ingredient_of.extend(products);
                item.ingredient_of.sort();
                item.ingredient_of.dedup();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixtures/metaforge/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).unwrap()
    }

    fn item(id: &str, name: &str) -> Item {
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
            details: ItemDetails::default(),
        }
    }

    #[test]
    fn reads_the_recorded_pages() {
        let (items, more) = parse_items_page(&fixture("items.json")).unwrap();
        assert_eq!(items.len(), 25);
        assert!(more);
        let shot = items.iter().find(|i| i.id == "adrenaline-shot").unwrap();
        assert_eq!(shot.components.len(), 2);
        assert_eq!(shot.workbench.as_deref(), Some("Med Stations 1"));
        let (quests, more) = parse_quests_page(&fixture("quests.json")).unwrap();
        assert_eq!(quests.len(), 20);
        assert!(more);
        assert_eq!(
            page_url(ITEMS_URL, 2),
            format!("{ITEMS_URL}?page=2&limit=100")
        );
    }

    #[test]
    fn maps_ids_to_raidtheory_style() {
        assert_eq!(
            local_id("heavy-gun-parts", false).as_str(),
            "heavy_gun_parts"
        );
        assert_eq!(local_id("wires-recipe", false).as_str(), "wires");
        assert_eq!(local_id("anvil-recipe", true).as_str(), "anvil_blueprint");
    }

    #[test]
    fn adds_only_what_is_missing() {
        let (items, _) = parse_items_page(&fixture("items.json")).unwrap();
        let (quests, _) = parse_quests_page(&fixture("quests.json")).unwrap();
        // The catalogue knows two of the fixture's items, one by its
        // Spanish name only, and one quest.
        let mut shot = item("adrenaline_shot", "Inyección de adrenalina");
        shot.aliases = vec!["Adrenaline Shot".into()];
        let cell = item("advanced_arc_powercell", "Advanced ARC Powercell");
        let mut catalog = Catalog::new("test", vec![shot, cell], Vec::new())
            .with_quests_and_projects(
                vec![Quest {
                    id: "a_bad_feeling".into(),
                    name: "A Bad Feeling".into(),
                    aliases: Vec::new(),
                    trader: "Celeste".into(),
                    previous: Vec::new(),
                    needs_items: false,
                }],
                Vec::new(),
            );
        let done = catalog.supplement(Database { items, quests }, None);
        assert_eq!(done.items, 23);
        assert_eq!(done.quests, 19);
        assert_eq!(catalog.items.len(), 25);

        // Known items keep their record; the cell learns where it drops.
        let shot = catalog.item(&ItemId::new("adrenaline_shot")).unwrap();
        assert_eq!(shot.name, "Inyección de adrenalina");
        assert!(shot.details.source.is_none());
        let cell = catalog
            .item(&ItemId::new("advanced_arc_powercell"))
            .unwrap();
        assert_eq!(cell.details.found_in, ["ARC"]);
        assert!(!cell.details.dropped_by.is_empty());
        assert!(done.found_in >= 1);

        // New ones carry what MetaForge knows, with references resolved.
        let guitar = catalog.item(&ItemId::new("acoustic_guitar")).unwrap();
        assert_eq!(guitar.value, Some(7000));
        assert_eq!(guitar.rarity, Some(arclens_core::Rarity::Legendary));
        assert_eq!(guitar.details.source.as_deref(), Some(SOURCE));
        assert_eq!(guitar.details.vendors[0].trader, "Shani");
        let wires = catalog.item(&ItemId::new("wires")).unwrap();
        assert_eq!(wires.name, "Wires");
        assert!(catalog.item(&ItemId::new("anvil_blueprint")).is_some());

        // A new quest's needs show on the items.
        assert!(catalog.quests.iter().all(|q| q.id != "a-bad-feeling"));
        assert!(catalog.items.iter().any(|i| {
            i.required_for
                .iter()
                .any(|r| r.kind == RequirementKind::Quest)
        }));
    }

    #[test]
    fn supplementing_twice_adds_nothing_more() {
        let (items, _) = parse_items_page(&fixture("items.json")).unwrap();
        let mut catalog = Catalog::new("test", Vec::new(), Vec::new());
        let db = Database {
            items,
            quests: Vec::new(),
        };
        let first = catalog.supplement(db.clone(), None);
        assert_eq!(first.items, 25);
        let again = catalog.supplement(db, None);
        assert_eq!(again.items, 0);
        assert_eq!(catalog.items.len(), 25);
    }

    #[test]
    fn adds_only_records_newer_than_the_dataset() {
        let (items, _) = parse_items_page(&fixture("items.json")).unwrap();
        let mut catalog = Catalog::new("test", Vec::new(), Vec::new());
        let db = Database {
            items,
            quests: Vec::new(),
        };
        // The recorded items are all from 2025.
        assert_eq!(catalog.supplement(db, Some(NEW_SINCE)).items, 0);
        assert!(is_new(Some("2026-10-08T09:00:00+00:00"), Some(NEW_SINCE)));
        assert!(!is_new(Some("2025-12-17T10:08:15+00:00"), Some(NEW_SINCE)));
        assert!(is_new(None, Some(NEW_SINCE)));
        assert!(is_new(Some("2025-12-17"), None));
    }
}
