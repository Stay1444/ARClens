//! Map presets in the app: the built-in ones merged with the player's,
//! which one each map and condition uses, and the "save as" draft.

use arclens_core::presets::{Kinds, context_key};
use arclens_core::{MarkerFilter, Preset, PresetBook};
use std::path::PathBuf;

/// What `presets.json` holds.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Saved {
    #[serde(flatten)]
    book: PresetBook,
    /// The preset applied last.
    #[serde(default)]
    active: Option<String>,
    /// The map and condition it was applied for ([`context_key`]). A new
    /// context applies its own preset; the same one keeps the player's
    /// toggles, across restarts too.
    #[serde(default)]
    context: Option<String>,
}

#[derive(Debug)]
pub struct Presets {
    saved: Saved,
    path: PathBuf,
    /// Built-in presets with the player's edits, then the player's own.
    all: Vec<Preset>,
    /// Name typed for a new preset.
    pub draft: String,
    /// Scope of a new preset: only the current map, only the current
    /// condition.
    pub for_map: bool,
    pub for_condition: bool,
    /// The marker filter was toggled since the active preset was applied.
    pub edited: bool,
}

impl Presets {
    pub fn load(path: PathBuf) -> Self {
        let saved: Saved = crate::store::load(&path).unwrap_or_default();
        let all = saved.book.all(arclens_data::presets::builtin());
        Self {
            saved,
            path,
            all,
            draft: String::new(),
            for_map: false,
            for_condition: true,
            edited: false,
        }
    }

    fn changed(&mut self) {
        self.all = self.saved.book.all(arclens_data::presets::builtin());
        crate::store::save(&self.path, &self.saved);
    }

    pub fn active(&self) -> Option<&Preset> {
        let id = self.saved.active.as_deref()?;
        self.all.iter().find(|p| p.id == id)
    }

    /// Presets suited to a context, best fit first.
    pub fn suited(&self, map: &str, condition: Option<&str>) -> Vec<&Preset> {
        let mut suited: Vec<(u8, &Preset)> = self
            .all
            .iter()
            .filter_map(|p| p.fit(map, condition).map(|score| (score, p)))
            .collect();
        // Stable: the shipped order stays within a fit.
        suited.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
        suited.into_iter().map(|(_, p)| p).collect()
    }

    /// The preset to apply when the context (map, condition) is new; `None`
    /// while it is the one the current filter was made for.
    pub fn on_context(&mut self, map: &str, condition: Option<&str>) -> Option<Preset> {
        let key = context_key(map, condition);
        if self.saved.context.as_deref() == Some(key.as_str()) {
            return None;
        }
        let preset = self.saved.book.pick(&self.all, map, condition)?.clone();
        self.saved.context = Some(key);
        self.saved.active = Some(preset.id.clone());
        self.changed();
        Some(preset)
    }

    /// The player picked `id` for a context: remembered for it.
    pub fn choose(&mut self, id: &str, map: &str, condition: Option<&str>) -> Option<Preset> {
        let preset = self.all.iter().find(|p| p.id == id)?.clone();
        self.saved.book.choose(map, condition, id);
        self.saved.context = Some(context_key(map, condition));
        self.saved.active = Some(preset.id.clone());
        self.changed();
        Some(preset)
    }

    /// Saves the current filter as a new preset named after the draft,
    /// scoped as asked, and makes it the context's choice.
    pub fn save_draft(
        &mut self,
        filter: &MarkerFilter,
        kinds: &Kinds<'_>,
        map: &str,
        condition: Option<&str>,
    ) {
        let name = self.draft.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let id = PresetBook::new_id(&name, &self.all);
        self.saved.book.save(Preset {
            id: id.clone(),
            name,
            description: String::new(),
            maps: if self.for_map {
                vec![map.to_owned()]
            } else {
                Vec::new()
            },
            conditions: match condition {
                Some(c) if self.for_condition => vec![c.to_owned()],
                _ => Vec::new(),
            },
            show: Preset::show_from(filter, kinds),
        });
        self.draft.clear();
        self.saved.book.choose(map, condition, &id);
        self.saved.context = Some(context_key(map, condition));
        self.saved.active = Some(id);
        self.changed();
    }

    /// Stores the current filter in the active preset (a built-in one gets
    /// the player's copy).
    pub fn update_active(&mut self, filter: &MarkerFilter, kinds: &Kinds<'_>) {
        let Some(mut preset) = self.active().cloned() else {
            return;
        };
        preset.show = Preset::show_from(filter, kinds);
        self.saved.book.save(preset);
        self.changed();
    }

    /// Deletes the player's preset, or reverts their edit of a built-in.
    pub fn remove(&mut self, id: &str) {
        let builtin = arclens_data::presets::builtin();
        self.saved.book.remove(id, builtin);
        if self.active().is_none() {
            self.saved.active = None;
        }
        self.changed();
    }

    pub fn is_builtin(id: &str) -> bool {
        arclens_data::presets::builtin().iter().any(|p| p.id == id)
    }

    /// The player has their own version of `id` (their preset, or an
    /// edited built-in).
    pub fn is_customised(&self, id: &str) -> bool {
        self.saved.book.presets.iter().any(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presets() -> (Presets, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Presets::load(dir.path().join("presets.json")), dir)
    }

    #[test]
    fn a_new_context_applies_its_preset_once() {
        let (mut presets, dir) = presets();
        let first = presets.on_context("dam", Some("Hurricane")).unwrap();
        assert_eq!(first.id, "first-wave-caches");
        // Same context: the player's toggles stay.
        assert!(presets.on_context("dam", Some("Hurricane")).is_none());
        // Remembered across restarts.
        let mut reloaded = Presets::load(dir.path().join("presets.json"));
        assert!(reloaded.on_context("dam", Some("Hurricane")).is_none());
        assert_eq!(reloaded.active().unwrap().id, "first-wave-caches");
        assert_eq!(reloaded.on_context("dam", None).unwrap().id, "everything");
    }

    #[test]
    fn choices_and_new_presets_stick_to_the_context() {
        let (mut presets, _dir) = presets();
        presets.choose("quests", "dam", Some("Hurricane")).unwrap();
        presets.on_context("dam", None);
        assert_eq!(
            presets.on_context("dam", Some("Hurricane")).unwrap().id,
            "quests"
        );

        let kinds = Kinds::from([("arc", [("tick", 2)].into()), ("quests", [("x", 1)].into())]);
        let mut filter = MarkerFilter::default();
        filter.toggle_category("arc");
        presets.draft = "Quest run".into();
        presets.save_draft(&filter, &kinds, "dam", Some("Hurricane"));
        let saved = presets.active().unwrap().clone();
        assert_eq!(saved.id, "my-quest-run");
        assert_eq!(saved.conditions, ["Hurricane"]);
        assert!(saved.maps.is_empty());
        assert_eq!(saved.show, ["quests"]);
        assert!(
            presets
                .suited("spaceport", Some("Hurricane"))
                .contains(&&saved)
        );
        assert!(!presets.suited("dam", None).contains(&&saved));

        presets.remove("my-quest-run");
        assert!(presets.active().is_none());
    }

    #[test]
    fn editing_a_default_keeps_a_copy_until_reset() {
        let (mut presets, _dir) = presets();
        presets.on_context("dam", None);
        let kinds = Kinds::from([("arc", [("tick", 2)].into())]);
        let mut filter = MarkerFilter::default();
        filter.toggle_category("arc");
        presets.update_active(&filter, &kinds);
        assert!(presets.is_customised("everything"));
        assert!(presets.active().unwrap().show.is_empty());
        presets.remove("everything");
        assert_eq!(presets.active().unwrap().show, ["*"]);
        assert!(Presets::is_builtin("everything"));
    }
}
