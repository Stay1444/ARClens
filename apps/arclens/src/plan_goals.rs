//! What the player plans for, all at once and in no order: workshop
//! stations to build up, projects (expeditions too) to deliver, items to
//! craft. Saved in `settings.json`; turned into [`Goal`]s against the
//! player's progress each time the plan is made.

use arclens_core::{Goal, ItemId, Progress, Project, Station};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanGoals {
    /// Station ids to build up.
    #[serde(default)]
    pub stations: BTreeSet<String>,
    /// Project ids to deliver.
    #[serde(default)]
    pub projects: BTreeSet<String>,
    /// All the way (every level, every phase) rather than the next one.
    #[serde(default)]
    pub to_max: bool,
    /// Items to craft, and how many.
    #[serde(default)]
    pub crafts: BTreeMap<ItemId, u32>,
}

impl PlanGoals {
    pub fn is_empty(&self) -> bool {
        self.stations.is_empty() && self.projects.is_empty() && self.crafts.is_empty()
    }

    /// The goals, against `progress`: each chosen station's next level (or
    /// its max), each chosen project's next phase (or its last), each
    /// craft. Stations and projects already finished drop out.
    pub fn goals(
        &self,
        stations: &[Station],
        projects: &[Project],
        progress: &Progress,
    ) -> Vec<Goal> {
        let station_goals = stations
            .iter()
            .filter(|s| self.stations.contains(&s.id))
            .filter_map(|s| {
                let now = progress.level(&s.id);
                let level = if self.to_max { s.max_level } else { now + 1 };
                (now < s.max_level).then(|| Goal::Station {
                    station: s.id.clone(),
                    level: level.min(s.max_level),
                })
            });
        let project_goals = projects
            .iter()
            .filter(|p| self.projects.contains(&p.id))
            .filter_map(|p| {
                let phases = u32::try_from(p.phases.len()).unwrap_or(u32::MAX);
                let done = progress.phases_done(&p.id);
                let phase = if self.to_max { phases } else { done + 1 };
                (done < phases).then(|| Goal::Project {
                    project: p.id.clone(),
                    phase: phase.min(phases),
                })
            });
        let crafts = self.crafts.iter().map(|(item, &count)| Goal::Craft {
            item: item.clone(),
            count: count.max(1),
        });
        station_goals.chain(project_goals).chain(crafts).collect()
    }

    /// With `station` chosen or not.
    #[must_use]
    pub fn toggle_station(&self, station: &str) -> Self {
        let mut next = self.clone();
        if !next.stations.remove(station) {
            next.stations.insert(station.to_owned());
        }
        next
    }

    #[must_use]
    pub fn toggle_project(&self, project: &str) -> Self {
        let mut next = self.clone();
        if !next.projects.remove(project) {
            next.projects.insert(project.to_owned());
        }
        next
    }

    /// With every one of `stations` chosen, or (if all were) none.
    #[must_use]
    pub fn toggle_all_stations<'a>(&self, stations: impl IntoIterator<Item = &'a str>) -> Self {
        let ids: BTreeSet<String> = stations.into_iter().map(str::to_owned).collect();
        let mut next = self.clone();
        if ids.is_subset(&next.stations) {
            next.stations.retain(|s| !ids.contains(s));
        } else {
            next.stations.extend(ids);
        }
        next
    }

    /// With `item` crafted `count` times (0 removes it).
    #[must_use]
    pub fn with_craft(&self, item: &ItemId, count: u32) -> Self {
        let mut next = self.clone();
        if count == 0 {
            next.crafts.remove(item);
        } else {
            next.crafts.insert(item.clone(), count);
        }
        next
    }

    #[must_use]
    pub fn with_to_max(&self, to_max: bool) -> Self {
        Self {
            to_max,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(id: &str, max_level: u32) -> Station {
        Station {
            id: id.into(),
            name: id.into(),
            aliases: Vec::new(),
            max_level,
        }
    }

    #[test]
    fn goals_follow_progress_and_reach() {
        let stations = [station("weapon_bench", 3), station("refiner", 3)];
        let projects = [Project {
            id: "trophy".into(),
            name: "Trophy Display".into(),
            phases: vec!["1".into(), "2".into(), "3".into()],
        }];
        let mut progress = Progress::default();
        progress.stations.insert("weapon_bench".into(), 1);
        progress.stations.insert("refiner".into(), 3);
        progress.projects.insert("trophy".into(), 1);
        let goals = PlanGoals::default()
            .toggle_all_stations(["weapon_bench", "refiner"])
            .toggle_project("trophy")
            .with_craft(&ItemId::new("anvil_i"), 2);
        assert_eq!(
            goals.goals(&stations, &projects, &progress),
            [
                // The refiner is maxed: no goal.
                Goal::Station {
                    station: "weapon_bench".into(),
                    level: 2
                },
                Goal::Project {
                    project: "trophy".into(),
                    phase: 2
                },
                Goal::Craft {
                    item: ItemId::new("anvil_i"),
                    count: 2
                },
            ]
        );
        let all = goals
            .with_to_max(true)
            .goals(&stations, &projects, &progress);
        assert_eq!(
            all[..2],
            [
                Goal::Station {
                    station: "weapon_bench".into(),
                    level: 3
                },
                Goal::Project {
                    project: "trophy".into(),
                    phase: 3
                },
            ]
        );
    }

    #[test]
    fn toggles_add_and_remove() {
        let goals = PlanGoals::default().toggle_all_stations(["a", "b"]);
        assert_eq!(goals.stations.len(), 2);
        assert!(goals.toggle_all_stations(["a", "b"]).stations.is_empty());
        assert_eq!(goals.toggle_station("a").stations.len(), 1);
        let crafted = goals.with_craft(&ItemId::new("x"), 3);
        assert_eq!(crafted.crafts[&ItemId::new("x")], 3);
        assert!(crafted.with_craft(&ItemId::new("x"), 0).crafts.is_empty());
        assert!(PlanGoals::default().is_empty());
    }
}
