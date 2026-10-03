//! The map presets ARClens ships (`data/presets.json`). Kinds are
//! MetaForge's category and subcategory ids; conditions are the game's
//! names (see [`crate::metaforge::CONDITIONS`]).

use arclens_core::Preset;
use std::sync::OnceLock;

/// The built-in presets, general ones first, then per condition.
pub fn builtin() -> &'static [Preset] {
    static PRESETS: OnceLock<Vec<Preset>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        // A unit test keeps the file valid; empty would only lose defaults.
        serde_json::from_str(include_str!("../data/presets.json")).unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metaforge::CONDITIONS;
    use arclens_core::PresetBook;

    #[test]
    fn presets_parse_with_unique_ids() {
        let presets = builtin();
        assert_eq!(presets[0].id, "everything");
        let mut ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), presets.len());
    }

    #[test]
    fn preset_conditions_are_real_condition_names() {
        for preset in builtin() {
            for condition in &preset.conditions {
                assert!(
                    CONDITIONS
                        .iter()
                        .any(|(_, list)| list.iter().any(|(name, _)| name == condition)),
                    "{}: unknown condition {condition}",
                    preset.id
                );
            }
        }
    }

    #[test]
    fn hurricane_on_dam_picks_first_wave_caches() {
        let book = PresetBook::default();
        let all = book.all(builtin());
        let pick = |condition| book.pick(&all, "dam", condition).unwrap().id.as_str();
        assert_eq!(pick(Some("Hurricane")), "first-wave-caches");
        assert_eq!(pick(Some("Cold Snap")), "cold-snap");
        assert_eq!(pick(Some("Night Raid")), "everything");
        assert_eq!(pick(None), "everything");
    }
}
