//! [MetaForge](https://metaforge.app/arc-raiders) public API: event schedule
//! (and, next, map markers).
//!
//! Terms (<https://metaforge.app/arc-raiders/api>): attribution with a link
//! for public projects, contact them before any paid use, cache responses —
//! endpoints may change without notice. ARClens is non-commercial, credits
//! MetaForge wherever its data is shown, and caches every response.

use crate::Error;
use arclens_core::ScheduledEvent;
use serde::Deserialize;

pub const ATTRIBUTION: &str = "MetaForge (metaforge.app/arc-raiders)";
pub const EVENTS_SCHEDULE_URL: &str = "https://metaforge.app/api/arc-raiders/events-schedule";

#[derive(Debug, Deserialize)]
struct EventsResponse {
    data: Vec<RawEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEvent {
    name: String,
    map: String,
    #[serde(default)]
    icon: Option<String>,
    start_time: i64,
    end_time: i64,
}

/// Parses an `events-schedule` response body.
pub fn parse_events(bytes: &[u8]) -> Result<Vec<ScheduledEvent>, Error> {
    let response: EventsResponse = serde_json::from_slice(bytes)?;
    Ok(response
        .data
        .into_iter()
        .filter(|e| e.end_time > e.start_time)
        .map(|e| ScheduledEvent {
            name: e.name,
            map: e.map,
            icon: e.icon,
            start_ms: e.start_time,
            end_ms: e.end_time,
        })
        .collect())
}

/// Fetches the current schedule.
pub async fn fetch_events(client: &reqwest::Client) -> Result<Vec<ScheduledEvent>, Error> {
    let bytes = client
        .get(EVENTS_SCHEDULE_URL)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    parse_events(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_recorded_shape() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/metaforge/events-schedule.json"
        );
        let events = parse_events(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].name, "Cold Snap");
        assert_eq!(events[0].map, "Dam");
        assert_eq!(events[0].end_ms - events[0].start_ms, 2 * 3_600_000);
        assert!(events[2].icon.is_none());
    }
}
