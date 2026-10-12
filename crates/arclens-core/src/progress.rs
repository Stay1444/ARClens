//! The player's own progress, entered in the companion app.
//!
//! The game exposes no API for it (only Embark's private one, which we don't
//! use), so the player tells us their workshop levels, finished quests and
//! project phases. With that, advice can tell "needed for an upgrade you
//! still have to build" from "needed for one you already have".

use crate::item::{ItemId, ItemQuantity, Requirement, RequirementKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A workshop station and how far it can be upgraded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    /// Its name in the other languages, for reading it off the screen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub max_level: u32,
}

/// A quest, for the progress editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub name: String,
    /// Its name in the other languages, for reading it off the screen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
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

impl Station {
    /// Its name, then its aliases.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

impl Quest {
    /// Its name, then its aliases.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

/// A project (expeditions among them) and its phases, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub phases: Vec<String>,
    /// What each phase asks for, in the game's order (its project page
    /// lists them top to bottom). Empty for a phase that asks for none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phase_items: Vec<Vec<ItemQuantity>>,
}

/// One requirement row of a project page as the game shows it: "4/15".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryRow {
    pub delivered: u32,
    pub needed: u32,
    /// Shown locked: an earlier phase isn't finished.
    pub locked: bool,
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
    /// Items already delivered to each project's current phase (the one
    /// after those finished), as its page in the game shows them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub delivered: BTreeMap<String, BTreeMap<ItemId, u32>>,
}

impl Progress {
    pub fn level(&self, station: &str) -> u32 {
        self.stations.get(station).copied().unwrap_or(0)
    }

    /// Phases of `project` finished.
    pub fn phases_done(&self, project: &str) -> u32 {
        self.projects.get(project).copied().unwrap_or(0)
    }

    /// How many of `item` are already delivered to `project`'s current
    /// phase.
    pub fn delivered(&self, project: &str, item: &ItemId) -> u32 {
        self.delivered
            .get(project)
            .and_then(|items| items.get(item))
            .copied()
            .unwrap_or(0)
    }

    /// Sets the phases of `project` finished. Deliveries counted towards
    /// another phase no longer apply.
    pub fn set_phases_done(&mut self, project: &str, phases: u32) {
        if self.phases_done(project) != phases {
            self.delivered.remove(project);
        }
        self.projects.insert(project.to_owned(), phases);
    }

    /// What `project`'s page in the game shows for `phase` (1-based): its
    /// rows' "delivered/needed" counts, in order. A phase being delivered
    /// is the current one, a locked phase is past it, a full phase is
    /// done. Returns whether anything changed; rows that don't match the
    /// phase's requirements (a misread, other data) change nothing.
    pub fn note_project_page(
        &mut self,
        project: &Project,
        phase: u32,
        rows: &[DeliveryRow],
    ) -> bool {
        let Some(items) = phase
            .checked_sub(1)
            .and_then(|i| project.phase_items.get(i as usize))
        else {
            return false;
        };
        let matches = !items.is_empty()
            && items.len() == rows.len()
            && items
                .iter()
                .zip(rows)
                .all(|(item, row)| item.quantity == row.needed && row.delivered <= row.needed);
        if !matches {
            return false;
        }
        let open: Vec<&DeliveryRow> = rows.iter().filter(|r| r.delivered < r.needed).collect();
        let done = self.phases_done(&project.id);
        let before = (done, self.delivered.get(&project.id).cloned());
        if open.is_empty() {
            self.set_phases_done(&project.id, done.max(phase));
        } else if open.iter().all(|r| r.locked) {
            // Locked: the phase before it isn't finished either.
            self.set_phases_done(&project.id, done.min(phase.saturating_sub(2)));
        } else if open.iter().all(|r| !r.locked) {
            self.set_phases_done(&project.id, phase - 1);
            let delivered: BTreeMap<ItemId, u32> = items
                .iter()
                .zip(rows)
                .filter(|(_, row)| row.delivered > 0)
                .map(|(item, row)| (item.item.clone(), row.delivered))
                .collect();
            if delivered.is_empty() {
                self.delivered.remove(&project.id);
            } else {
                self.delivered.insert(project.id.clone(), delivered);
            }
        }
        (
            self.phases_done(&project.id),
            self.delivered.get(&project.id).cloned(),
        ) != before
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

    /// `active` is in progress in the game (listed in its logbook or on
    /// a trader's quest page): every quest before it is done, and it and
    /// the ones after it are not. Returns whether anything changed.
    pub fn note_active_quest(&mut self, active: &str, quests: &[Quest]) -> bool {
        let before = self.quests_done.clone();
        if let Some(quest) = quests.iter().find(|q| q.id == active) {
            for previous in &quest.previous {
                self.set_quest_done(previous, true, quests);
            }
        }
        self.set_quest_done(active, false, quests);
        self.quests_done != before
    }

    /// Whether `req` is still ahead of the player: workshop upgrades not
    /// built yet, quests not done, project phases not finished. Anything
    /// without an id to check counts.
    /// Whether `req` is the very next step: a station's next level, a
    /// quest whose earlier quests are done, a project's next phase. What
    /// the player works on now, unlike [`Self::needs`] (everything ahead).
    pub fn is_next_step(&self, req: &Requirement, quests: &[Quest]) -> bool {
        match (req.kind, req.station.as_deref(), req.level) {
            (RequirementKind::WorkshopUpgrade, Some(station), Some(level)) => {
                level == self.level(station) + 1
            }
            (RequirementKind::Quest, Some(quest), _) => {
                !self.quest_done(quest)
                    && quests
                        .iter()
                        .find(|q| q.id == quest)
                        .is_none_or(|q| q.previous.iter().all(|p| self.quest_done(p)))
            }
            (RequirementKind::Project, Some(project), Some(phase)) => {
                phase == self.phases_done(project) + 1
            }
            _ => false,
        }
    }

    /// How many of `item` `req` still asks for: none if it is behind the
    /// player ([`Self::needs`]), less what is already delivered to a
    /// project's current phase.
    pub fn still_needed(&self, item: &ItemId, req: &Requirement) -> u32 {
        if !self.needs(req) {
            return 0;
        }
        match (req.kind, req.station.as_deref(), req.level) {
            (RequirementKind::Project, Some(project), Some(phase))
                if phase == self.phases_done(project) + 1 =>
            {
                req.quantity.saturating_sub(self.delivered(project, item))
            }
            _ => req.quantity,
        }
    }

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

    #[test]
    fn next_steps_are_one_ahead() {
        let mut progress = Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);
        progress.quests_done.insert("first".into());
        progress.projects.insert("trophy".into(), 1);
        let quests = [
            Quest {
                id: "second".into(),
                name: "Second".into(),
                aliases: Vec::new(),
                trader: String::new(),
                previous: vec!["first".into()],
                needs_items: true,
            },
            Quest {
                id: "third".into(),
                name: "Third".into(),
                aliases: Vec::new(),
                trader: String::new(),
                previous: vec!["second".into()],
                needs_items: true,
            },
        ];
        let req = |kind, id: &str, level| Requirement {
            kind,
            name: String::new(),
            quantity: 1,
            station: Some(id.into()),
            level,
        };
        let next = |r: &Requirement| progress.is_next_step(r, &quests);
        assert!(next(&upgrade("weapon_bench", 3)));
        assert!(!next(&upgrade("weapon_bench", 4)));
        assert!(!next(&upgrade("weapon_bench", 2)));
        assert!(next(&req(RequirementKind::Quest, "second", None)));
        assert!(!next(&req(RequirementKind::Quest, "third", None)));
        assert!(!next(&req(RequirementKind::Quest, "first", None)));
        assert!(next(&req(RequirementKind::Project, "trophy", Some(2))));
        assert!(!next(&req(RequirementKind::Project, "trophy", Some(3))));
    }

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
            aliases: Vec::new(),
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
    fn an_active_quest_completes_its_chain_before_it() {
        let quests = [
            quest("a", &[]),
            quest("b", &["a"]),
            quest("c", &["b"]),
            quest("d", &["c"]),
        ];
        let mut progress = Progress::default();
        // Ticked too far by hand: "c" is still in progress in game.
        progress.set_quest_done("d", true, &quests);
        assert!(progress.note_active_quest("c", &quests));
        assert_eq!(progress.quests_done.iter().collect::<Vec<_>>(), ["a", "b"]);
        assert!(!progress.note_active_quest("c", &quests));
    }

    #[test]
    fn built_levels_are_no_longer_needed() {
        let mut progress = Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);
        assert!(!progress.needs(&upgrade("weapon_bench", 2)));
        assert!(progress.needs(&upgrade("weapon_bench", 3)));
        assert!(progress.needs(&upgrade("med_station", 1)));
    }

    /// Trophy Display's first two phases, as the game shows them
    /// (2026-10-12 screenshots).
    fn trophy() -> Project {
        let phase = |items: &[(&str, u32)]| {
            items
                .iter()
                .map(|&(item, quantity)| ItemQuantity {
                    item: ItemId::new(item),
                    quantity,
                })
                .collect()
        };
        Project {
            id: "trophy".into(),
            name: "Trophy Display".into(),
            phases: vec!["Roaming Threats".into(), "Soaring Menaces".into()],
            phase_items: vec![
                phase(&[
                    ("rusted_bolts", 3),
                    ("pop_trigger", 15),
                    ("tick_pod", 15),
                    ("surveyor_vault", 5),
                ]),
                phase(&[
                    ("spotter_relay", 10),
                    ("expired_respirator", 3),
                    ("wasp_driver", 20),
                    ("hornet_driver", 15),
                ]),
            ],
        }
    }

    fn rows(counts: &[(u32, u32)], locked: bool) -> Vec<DeliveryRow> {
        counts
            .iter()
            .map(|&(delivered, needed)| DeliveryRow {
                delivered,
                needed,
                locked,
            })
            .collect()
    }

    fn project_req(phase: u32, quantity: u32) -> Requirement {
        Requirement {
            quantity,
            ..req(RequirementKind::Project, "trophy", Some(phase))
        }
    }

    #[test]
    fn project_pages_set_phase_and_deliveries() {
        let trophy = trophy();
        let mut progress = Progress::default();
        progress.projects.insert("trophy".into(), 2);
        // Phase 1 being delivered: it is the current one.
        let page = rows(&[(2, 3), (0, 15), (4, 15), (0, 5)], false);
        assert!(progress.note_project_page(&trophy, 1, &page));
        assert_eq!(progress.phases_done("trophy"), 0);
        let bolts = ItemId::new("rusted_bolts");
        assert_eq!(progress.delivered("trophy", &bolts), 2);
        assert_eq!(progress.still_needed(&bolts, &project_req(1, 3)), 1);
        assert_eq!(
            progress.still_needed(&ItemId::new("pop_trigger"), &project_req(1, 15)),
            15
        );
        assert!(!progress.note_project_page(&trophy, 1, &page));

        // Phase 2 locked: nothing new (phase 1 is still open).
        let locked = rows(&[(0, 10), (0, 3), (0, 20), (0, 15)], true);
        assert!(!progress.note_project_page(&trophy, 2, &locked));

        // Phase 1 full: done, and its deliveries no longer count.
        let full = rows(&[(3, 3), (15, 15), (15, 15), (5, 5)], false);
        assert!(progress.note_project_page(&trophy, 1, &full));
        assert_eq!(progress.phases_done("trophy"), 1);
        assert_eq!(progress.delivered("trophy", &bolts), 0);

        // Ticked too far by hand, then phase 2 seen locked.
        progress.set_phases_done("trophy", 2);
        assert!(progress.note_project_page(&trophy, 2, &locked));
        assert_eq!(progress.phases_done("trophy"), 0);
    }

    #[test]
    fn project_pages_that_do_not_match_change_nothing() {
        let trophy = trophy();
        let mut progress = Progress::default();
        // A misread count, a row missing, a phase the data doesn't have.
        let misread = rows(&[(0, 3), (0, 16), (0, 15), (0, 5)], false);
        assert!(!progress.note_project_page(&trophy, 1, &misread));
        assert!(!progress.note_project_page(&trophy, 1, &misread[..3]));
        assert!(!progress.note_project_page(&trophy, 3, &misread));
        assert!(!progress.note_project_page(&trophy, 0, &misread));
        assert_eq!(progress, Progress::default());
    }
}
