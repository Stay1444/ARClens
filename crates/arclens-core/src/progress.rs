//! The player's own progress, entered in the companion app.
//!
//! The game exposes no API for it (only Embark's private one, which we don't
//! use), so the player tells us their workshop levels once. With that, advice
//! can tell "needed for an upgrade you still have to build" from "needed for
//! one you already have".

use crate::item::{Requirement, RequirementKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A workshop station and how far it can be upgraded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub max_level: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Current level per station id. Missing means level 0 (not built).
    #[serde(default)]
    pub stations: BTreeMap<String, u32>,
}

impl Progress {
    pub fn level(&self, station: &str) -> u32 {
        self.stations.get(station).copied().unwrap_or(0)
    }

    /// Whether `req` is still ahead of the player. Workshop upgrades already
    /// built are done; quests and projects aren't tracked yet, so they count.
    pub fn needs(&self, req: &Requirement) -> bool {
        match (req.kind, &req.station, req.level) {
            (RequirementKind::WorkshopUpgrade, Some(station), Some(level)) => {
                level > self.level(station)
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

    #[test]
    fn built_levels_are_no_longer_needed() {
        let mut progress = Progress::default();
        progress.stations.insert("weapon_bench".into(), 2);
        assert!(!progress.needs(&upgrade("weapon_bench", 2)));
        assert!(progress.needs(&upgrade("weapon_bench", 3)));
        assert!(progress.needs(&upgrade("med_station", 1)));
    }
}
