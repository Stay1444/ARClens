//! Map conditions / events (Night Raid, Harvester, Matriarch, …) and when
//! they run.

use serde::{Deserialize, Serialize};

/// One scheduled occurrence of a map condition. Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub name: String,
    /// Display name of the map, e.g. "Dam", "Blue Gate".
    pub map: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl ScheduledEvent {
    pub fn is_active(&self, now_ms: i64) -> bool {
        (self.start_ms..self.end_ms).contains(&now_ms)
    }
}

/// A schedule split into what's running now and what comes next.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Agenda<'a> {
    /// Running now, ending soonest first.
    pub active: Vec<&'a ScheduledEvent>,
    /// Starting later, soonest first.
    pub upcoming: Vec<&'a ScheduledEvent>,
}

/// Splits `events` around `now_ms`; finished events are dropped.
pub fn agenda(events: &[ScheduledEvent], now_ms: i64) -> Agenda<'_> {
    let mut active: Vec<_> = events.iter().filter(|e| e.is_active(now_ms)).collect();
    let mut upcoming: Vec<_> = events.iter().filter(|e| e.start_ms > now_ms).collect();
    active.sort_by_key(|e| (e.end_ms, e.map.clone()));
    upcoming.sort_by_key(|e| (e.start_ms, e.map.clone()));
    Agenda { active, upcoming }
}

/// "33m 16s", "1h 33m", "2d 4h": compact countdown for `ms` milliseconds.
pub fn countdown(ms: i64) -> String {
    let secs = (ms.max(0) + 999) / 1000;
    let (days, hours, mins, secs) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60, secs % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, map: &str, start_min: i64, end_min: i64) -> ScheduledEvent {
        ScheduledEvent {
            name: name.into(),
            map: map.into(),
            icon: None,
            start_ms: start_min * 60_000,
            end_ms: end_min * 60_000,
        }
    }

    #[test]
    fn splits_active_and_upcoming_and_drops_finished() {
        let events = [
            event("Old", "Dam", 0, 60),
            event("Night Raid", "Riven Tides", 90, 150),
            event("Harvester", "Dam", 130, 190),
            event("Matriarch", "Spaceport", 60, 120),
        ];
        let agenda = agenda(&events, 100 * 60_000);
        let names = |v: &[&ScheduledEvent]| v.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&agenda.active), ["Matriarch", "Night Raid"]);
        assert_eq!(names(&agenda.upcoming), ["Harvester"]);
    }

    #[test]
    fn countdowns_are_compact() {
        assert_eq!(countdown(33 * 60_000 + 16_000), "33m 16s");
        assert_eq!(countdown(93 * 60_000), "1h 33m");
        assert_eq!(countdown(52 * 3_600_000), "2d 4h");
        assert_eq!(countdown(-5), "0s");
    }
}
