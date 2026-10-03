//! The player's own progress, entered in the companion app.
//!
//! The game exposes no API for it (only Embark's private one, which we don't
//! use), so the player tells us their workshop levels, finished quests and
//! project phases. With that, advice can tell "needed for an upgrade you
//! still have to build" from "needed for one you already have".

use crate::item::{Requirement, RequirementKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A workshop station and how far it can be upgraded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub max_level: u32,
}

/// A quest, for the progress editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub name: String,
    /// The trader who gives it.
    #[serde(default)]
    pub trader: String,
    /// Quests that must be done first.
    #[serde(default)]
    pub previous: Vec<String>,
    /// Whether it asks for items (and so matters to advice).
    #[serde(default)]
    pub needs_items: bool,
}

/// A project (expeditions among them) and its phases, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Current level per station id. Missing means level 0 (not built).
    #[serde(default)]
    pub stations: BTreeMap<String, u32>,
    /// Ids of finished quests.
    #[serde(default)]
    pub quests_done: BTreeSet<String>,
    /// Phases finished per project id. Missing means none.
    #[serde(default)]
    pub projects: BTreeMap<String, u32>,
    /// Ids of blueprints already learned.
    #[serde(default)]
    pub blueprints: BTreeSet<crate::ItemId>,
}

impl Progress {
    pub fn level(&self, station: &str) -> u32 {
        self.stations.get(station).copied().unwrap_or(0)
    }

    /// Phases of `project` finished.
    pub fn phases_done(&self, project: &str) -> u32 {
        self.projects.get(project).copied().unwrap_or(0)
    }

    pub fn blueprint_learned(&self, blueprint: &crate::ItemId) -> bool {
        self.blueprints.contains(blueprint)
    }

    pub fn quest_done(&self, quest: &str) -> bool {
        self.quests_done.contains(quest)
    }

    /// Marks `quest` done or not. Done also marks every quest before it
    /// (the game only offers a quest once those are done); not done also
    /// clears every quest after it.
    pub fn set_quest_done(&mut self, quest: &str, done: bool, quests: &[Quest]) {
        let mut stack = vec![quest.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if done {
                if let Some(q) = quests.iter().find(|q| q.id == id) {
                    stack.extend(q.previous.iter().cloned());
                }
                self.quests_done.insert(id);
            } else {
                stack.extend(
                    quests
                        .iter()
                        .filter(|q| q.previous.contains(&id))
                        .map(|q| q.id.clone()),
                );
                self.quests_done.remove(&id);
            }
        }
    }

    /// Whether `req` is still ahead of the player: workshop upgrades not
    /// built yet, quests not done, project phases not finished. Anything
    /// without an id to check counts.
    pub fn needs(&self, req: &Requirement) -> bool {
        match (req.kind, req.station.as_deref(), req.level) {
            (RequirementKind::WorkshopUpgrade, Some(station), Some(level)) => {
                level > self.level(station)
            }
            (RequirementKind::Quest, Some(quest), _) => !self.quest_done(quest),
            (RequirementKind::Project, Some(project), Some(phase)) => {
                phase > self.phases_done(project)
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upgrade(station: &str, level: u32) -> Requirement {
        Requirement {
            kind: RequirementKind::WorkshopUpgrade,
            name: format!("{station} {level}"),
            quantity: 1,
            station: Some(station.into()),
            level: Some(level),
        }
    }

    fn req(kind: RequirementKind, id: &str, level: Option<u32>) -> Requirement {
        Requirement {
            kind,
            name: id.into(),
            quantity: 1,
            station: Some(id.into()),
            level,
        }
    }

    fn quest(id: &str, previous: &[&str]) -> Quest {
        Quest {
            id: id.into(),
            name: id.into(),
            trader: String::new(),
            previous: previous.iter().map(|&p| p.into()).collect(),
            needs_items: false,
        }
    }

    #[test]
    fn done_quests_and_phases_are_no_longer_needed() {
        let mut progress = Progress::default();
        progress.quests_done.insert("a_bad_feeling".into());
        progress.projects.insert("expedition".into(), 2);
        assert!(!progress.needs(&req(RequirementKind::Quest, "a_bad_feeling", None)));
        assert!(progress.needs(&req(RequirementKind::Quest, "in_my_image", None)));
        assert!(!progress.needs(&req(RequirementKind::Project, "expedition", Some(2))));
        assert!(progress.needs(&req(RequirementKind::Project, "expedition", Some(3))));
    }

    #[test]
    fn quest_chains_are_marked_together() {
        let quests = [
            quest("a", &[]),
            quest("b", &["a"]),
            quest("c", &["b"]),
            quest("d", &["a"]),
        ];
        let mut progress = Progress::default();
        progress.set_quest_done("c", true, &quests);
        assert_eq!(
            progress.quests_done.iter().collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        progress.set_quest_done("d", true, &quests);
        progress.set_quest_done("b", false, &quests);
        assert_eq!(progress.quests_done.iter().collect::<Vec<_>>(), ["a", "d"]);
    }

    #[test]
    fn built_levels_are_no_longer_needed() {
        let mut progress = Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);
        assert!(!progress.needs(&upgrade("weapon_bench", 2)));
        assert!(progress.needs(&upgrade("weapon_bench", 3)));
        assert!(progress.needs(&upgrade("med_station", 1)));
    }
}
