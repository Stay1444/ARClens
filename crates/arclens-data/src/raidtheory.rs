//! Provider for the [RaidTheory/arcraiders-data] community dataset (MIT).
//!
//! The dataset is a directory of JSON files:
//!
//! ```text
//! items/<id>.json      one file per item
//! hideout/<id>.json    workshop stations and their upgrade requirements
//! quests/<id>.json     quests and the items they require
//! projects.json        long-running projects and their phase requirements
//! ```
//!
//! Localised strings are objects keyed by language (`{"en": "...", "de": ...}`),
//! but some files use plain strings; [`Localized`] accepts both.
//!
//! [RaidTheory/arcraiders-data]: https://github.com/RaidTheory/arcraiders-data

use crate::{Catalog, Error};
use arclens_core::{Item, ItemId, ItemQuantity, Rarity, Requirement, RequirementKind};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Attribution string stored in [`Catalog::source`].
pub const SOURCE: &str = "RaidTheory/arcraiders-data (MIT)";

/// Language used when resolving [`Localized`] strings.
const LANG: &str = "en";

/// Loads a catalog from a local checkout / extracted archive of the dataset.
#[derive(Debug, Clone)]
pub struct RaidTheoryDir {
    root: PathBuf,
}

impl RaidTheoryDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Reads and joins items, hideout, quests and projects into a [`Catalog`].
    ///
    /// Blocking: call from `spawn_blocking` in async contexts.
    pub fn load(&self) -> Result<Catalog, Error> {
        let mut items: Vec<Item> = read_dir_json::<RawItem>(&self.root.join("items"))?
            .into_iter()
            .map(RawItem::into_item)
            .collect();
        items.sort_by(|a, b| a.id.cmp(&b.id));

        let mut requirements: HashMap<ItemId, Vec<Requirement>> = HashMap::new();
        let mut push = |item: &str, kind, name: String, quantity| {
            requirements
                .entry(ItemId::new(item))
                .or_default()
                .push(Requirement {
                    kind,
                    name,
                    quantity,
                });
        };

        for station in read_dir_json::<RawStation>(&self.root.join("hideout"))? {
            let station_name = station.name.resolve();
            for level in station.levels {
                for req in level.requirement_item_ids {
                    push(
                        &req.item_id,
                        RequirementKind::WorkshopUpgrade,
                        format!("{station_name} {}", level.level),
                        req.quantity,
                    );
                }
            }
        }

        for quest in read_dir_json::<RawQuest>(&self.root.join("quests"))? {
            let name = quest.name.resolve();
            for req in quest.required_item_ids {
                push(
                    &req.item_id,
                    RequirementKind::Quest,
                    name.clone(),
                    req.quantity,
                );
            }
        }

        let projects_path = self.root.join("projects.json");
        if projects_path.exists() {
            let projects: Vec<RawProject> = read_json(&projects_path)?;
            for project in projects.into_iter().filter(|p| !p.disabled) {
                let project_name = project.name.resolve();
                for phase in project.phases {
                    for req in phase.requirement_item_ids {
                        push(
                            &req.item_id,
                            RequirementKind::Project,
                            format!("{project_name}: {}", phase.name.resolve()),
                            req.quantity,
                        );
                    }
                }
            }
        }

        for item in &mut items {
            if let Some(reqs) = requirements.remove(&item.id) {
                item.required_for = reqs;
            }
        }

        Ok(Catalog::new(SOURCE, items, Vec::new()))
    }
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
    fn resolve(&self) -> String {
        match self {
            Self::Plain(s) => s.clone(),
            Self::ByLang(map) => map
                .get(LANG)
                .or_else(|| map.values().next())
                .cloned()
                .unwrap_or_default(),
        }
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
    image_filename: Option<String>,
}

impl RawItem {
    fn into_item(self) -> Item {
        Item {
            id: ItemId::new(self.id),
            name: self.name.resolve(),
            description: self
                .description
                .map(|d| d.resolve())
                .filter(|d| !d.is_empty()),
            rarity: self.rarity.as_deref().and_then(parse_rarity),
            category: self.kind,
            value: self.value,
            weight: self.weight_kg,
            stack_size: self.stack_size,
            recycles_into: self
                .recycles_into
                .into_iter()
                .map(|(item, quantity)| ItemQuantity {
                    item: ItemId::new(item),
                    quantity,
                })
                .collect(),
            required_for: Vec::new(),
            image_url: self.image_filename.filter(|u| u.starts_with("https://")),
        }
    }
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
    name: Localized,
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
    name: Localized,
    #[serde(default)]
    required_item_ids: Vec<RawItemQuantity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProject {
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
