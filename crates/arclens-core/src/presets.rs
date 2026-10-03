//! Map presets: named marker selections ("First Wave caches") meant for a
//! map, a map condition, or both. The player's current context (map plus
//! condition) picks one; the player can switch, edit and add their own.

use crate::map::{MarkerFilter, sub_key};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Marker counts per category, then subcategory, as
/// [`marker_counts`](crate::marker_counts) returns them.
pub type Kinds<'a> = BTreeMap<&'a str, BTreeMap<&'a str, usize>>;

/// Categories a preset never hides: place names keep the plot readable.
const ALWAYS_SHOWN: &[&str] = &["labels"];

/// `show` entry meaning every kind.
pub const EVERYTHING: &str = "*";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Maps it is meant for (MetaForge ids); empty: every map.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maps: Vec<String>,
    /// Conditions it is meant for, by name ("Hurricane"); empty: any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
    /// Kinds shown: a category (`"quests"`), a subcategory
    /// (`"containers/raider_cache"`), or [`EVERYTHING`]. The rest is hidden.
    pub show: Vec<String>,
}

impl Preset {
    /// How well the preset suits a context, if at all: condition presets
    /// beat map presets, which beat general ones. A condition preset never
    /// suits an unknown condition.
    pub fn fit(&self, map: &str, condition: Option<&str>) -> Option<u8> {
        let map_ok = self.maps.is_empty() || self.maps.iter().any(|m| m == map);
        let condition_ok = self.conditions.is_empty()
            || condition.is_some_and(|c| self.conditions.iter().any(|x| x.eq_ignore_ascii_case(c)));
        (map_ok && condition_ok)
            .then(|| 2 * u8::from(!self.conditions.is_empty()) + u8::from(!self.maps.is_empty()))
    }

    /// The filter that shows this preset's kinds among `kinds` and hides
    /// the rest. Kinds the map lacks don't matter.
    pub fn filter(&self, kinds: &Kinds<'_>) -> MarkerFilter {
        let mut filter = MarkerFilter::default();
        if self.show.iter().any(|s| s == EVERYTHING) {
            return filter;
        }
        for (&category, subs) in kinds {
            if ALWAYS_SHOWN.contains(&category) || self.show.iter().any(|s| s == category) {
                continue;
            }
            let listed: Vec<&str> = subs
                .keys()
                .copied()
                .filter(|sub| !sub.is_empty() && self.show.contains(&sub_key(category, sub)))
                .collect();
            if listed.is_empty() {
                filter.hidden.insert(category.to_owned());
            } else {
                filter.hidden.extend(
                    subs.keys()
                        .filter(|sub| !sub.is_empty() && !listed.contains(sub))
                        .map(|sub| sub_key(category, sub)),
                );
            }
        }
        filter
    }

    /// The `show` list that reproduces `filter` on `kinds`.
    pub fn show_from(filter: &MarkerFilter, kinds: &Kinds<'_>) -> Vec<String> {
        let mut show = Vec::new();
        let mut everything = true;
        for (&category, subs) in kinds {
            if ALWAYS_SHOWN.contains(&category) {
                continue;
            }
            if !filter.shows_category(category) {
                everything = false;
                continue;
            }
            let named: Vec<&str> = subs.keys().copied().filter(|s| !s.is_empty()).collect();
            let shown: Vec<&str> = named
                .iter()
                .copied()
                .filter(|sub| filter.shows_subcategory(category, sub))
                .collect();
            if shown.len() == named.len() {
                show.push(category.to_owned());
            } else {
                everything = false;
                show.extend(shown.into_iter().map(|sub| sub_key(category, sub)));
            }
        }
        if everything {
            vec![EVERYTHING.to_owned()]
        } else {
            show
        }
    }

    /// Whether `filter` still is this preset on `kinds` (not edited since).
    pub fn matches(&self, filter: &MarkerFilter, kinds: &Kinds<'_>) -> bool {
        self.filter(kinds) == *filter
    }
}

/// The player's presets and choices, saved between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetBook {
    /// The player's own presets. One with a built-in preset's id replaces
    /// it (an edited default); deleting it brings the default back.
    #[serde(default)]
    pub presets: Vec<Preset>,
    /// The preset last chosen per context ([`context_key`]).
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
}

/// Key of a map plus condition in [`PresetBook::choices`].
pub fn context_key(map: &str, condition: Option<&str>) -> String {
    format!("{map}|{}", condition.unwrap_or("").to_lowercase())
}

impl PresetBook {
    /// Built-in presets with the player's edits applied, then the player's
    /// own, in that order.
    pub fn all(&self, builtin: &[Preset]) -> Vec<Preset> {
        let mut all: Vec<Preset> = builtin
            .iter()
            .map(|b| {
                self.presets
                    .iter()
                    .find(|p| p.id == b.id)
                    .unwrap_or(b)
                    .clone()
            })
            .collect();
        all.extend(
            self.presets
                .iter()
                .filter(|p| !builtin.iter().any(|b| b.id == p.id))
                .cloned(),
        );
        all
    }

    /// The preset for a context: the one last chosen there, else the
    /// best-fitting one (the first of equals).
    pub fn pick<'a>(
        &self,
        all: &'a [Preset],
        map: &str,
        condition: Option<&str>,
    ) -> Option<&'a Preset> {
        let chosen = self.choices.get(&context_key(map, condition));
        chosen
            .and_then(|id| all.iter().find(|p| &p.id == id))
            .or_else(|| {
                all.iter()
                    .filter_map(|p| p.fit(map, condition).map(|score| (score, p)))
                    // `max_by_key` keeps the last of equals; reverse for the first.
                    .rev()
                    .max_by_key(|&(score, _)| score)
                    .map(|(_, p)| p)
            })
    }

    /// Remembers `id` as the choice for a context.
    pub fn choose(&mut self, map: &str, condition: Option<&str>, id: &str) {
        self.choices
            .insert(context_key(map, condition), id.to_owned());
    }

    /// Adds `preset`, or replaces the player's preset with its id.
    pub fn save(&mut self, preset: Preset) {
        match self.presets.iter_mut().find(|p| p.id == preset.id) {
            Some(existing) => *existing = preset,
            None => self.presets.push(preset),
        }
    }

    /// Drops the player's preset `id` (an edited default reverts). Choices
    /// of it are forgotten unless a built-in preset still has that id.
    pub fn remove(&mut self, id: &str, builtin: &[Preset]) {
        self.presets.retain(|p| p.id != id);
        if !builtin.iter().any(|b| b.id == id) {
            self.choices.retain(|_, chosen| chosen != id);
        }
    }

    /// An id for a new preset called `name`, unused by `all`.
    pub fn new_id(name: &str, all: &[Preset]) -> String {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = format!("my-{}", if slug.is_empty() { "preset" } else { &slug });
        let taken = |id: &str| all.iter().any(|p| p.id == id);
        if !taken(&base) {
            return base;
        }
        // `all` has fewer entries than this range, so one is free.
        (2..=all.len() + 2)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(id: &str, maps: &[&str], conditions: &[&str], show: &[&str]) -> Preset {
        Preset {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            maps: maps.iter().map(|s| (*s).into()).collect(),
            conditions: conditions.iter().map(|s| (*s).into()).collect(),
            show: show.iter().map(|s| (*s).into()).collect(),
        }
    }

    fn kinds() -> Kinds<'static> {
        let mut kinds = Kinds::new();
        kinds.insert(
            "containers",
            BTreeMap::from([("raider_cache", 3), ("hurricane_cache", 2), ("locker", 9)]),
        );
        kinds.insert("arc", BTreeMap::from([("tick", 4)]));
        kinds.insert("quests", BTreeMap::from([("espresso", 1)]));
        kinds.insert("labels", BTreeMap::from([("", 30)]));
        kinds
    }

    #[test]
    fn condition_presets_beat_map_presets_beat_general_ones() {
        let all = [
            preset("everything", &[], &[], &["*"]),
            preset("dam", &["dam"], &[], &["arc"]),
            preset("hurricane", &[], &["Hurricane"], &["containers"]),
            preset("dam-hurricane", &["dam"], &["Hurricane"], &["containers"]),
        ];
        let book = PresetBook::default();
        let pick = |map, condition| book.pick(&all, map, condition).unwrap().id.clone();
        assert_eq!(pick("spaceport", None), "everything");
        assert_eq!(pick("dam", None), "dam");
        assert_eq!(pick("spaceport", Some("hurricane")), "hurricane");
        assert_eq!(pick("dam", Some("Hurricane")), "dam-hurricane");
        assert_eq!(pick("dam", Some("Cold Snap")), "dam");
    }

    #[test]
    fn a_choice_sticks_to_its_context() {
        let all = [
            preset("everything", &[], &[], &["*"]),
            preset("quests", &[], &[], &["quests"]),
        ];
        let mut book = PresetBook::default();
        book.choose("dam", Some("Hurricane"), "quests");
        assert_eq!(
            book.pick(&all, "dam", Some("hurricane")).unwrap().id,
            "quests"
        );
        assert_eq!(book.pick(&all, "dam", None).unwrap().id, "everything");
        // A deleted preset's choice falls back to the best fit.
        book.remove("quests", &[]);
        assert_eq!(
            book.pick(&all[..1], "dam", Some("Hurricane")).unwrap().id,
            "everything"
        );
        assert_eq!(book.choices.len(), 0);
    }

    #[test]
    fn a_preset_becomes_a_filter_and_back() {
        let kinds = kinds();
        let caches = preset(
            "caches",
            &[],
            &[],
            &[
                "containers/raider_cache",
                "containers/hurricane_cache",
                "quests",
            ],
        );
        let filter = caches.filter(&kinds);
        assert!(!filter.shows_category("arc"));
        assert!(filter.shows_category("labels"));
        assert!(filter.shows_category("quests"));
        assert!(filter.shows_subcategory("containers", "raider_cache"));
        assert!(!filter.shows_subcategory("containers", "locker"));
        assert!(caches.matches(&filter, &kinds));

        let mut show = Preset::show_from(&filter, &kinds);
        show.sort();
        let mut expected = caches.show.clone();
        expected.sort();
        assert_eq!(show, expected);

        let everything = preset("all", &[], &[], &["*"]);
        assert_eq!(everything.filter(&kinds), MarkerFilter::default());
        assert_eq!(Preset::show_from(&MarkerFilter::default(), &kinds), ["*"]);
        assert!(!everything.matches(&filter, &kinds));
    }

    #[test]
    fn edited_defaults_replace_and_revert() {
        let builtin = [preset("everything", &[], &[], &["*"])];
        let mut book = PresetBook::default();
        book.save(preset("everything", &[], &[], &["arc"]));
        book.save(preset("mine", &[], &[], &["quests"]));
        let all = book.all(&builtin);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].show, ["arc"]);
        book.choose("dam", None, "everything");
        book.remove("everything", &builtin);
        assert_eq!(book.all(&builtin)[0].show, ["*"]);
        // The built-in still exists, so the choice stays.
        assert_eq!(book.choices.len(), 1);
    }

    #[test]
    fn new_ids_are_unique_slugs() {
        let all = [preset("my-cache-run", &[], &[], &[])];
        assert_eq!(PresetBook::new_id("Loot run!", &all), "my-loot-run");
        assert_eq!(PresetBook::new_id("Cache run", &all), "my-cache-run-2");
        assert_eq!(PresetBook::new_id("  ", &all), "my-preset");
    }
}
