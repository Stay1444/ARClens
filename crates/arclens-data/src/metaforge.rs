//! [MetaForge](https://metaforge.app/arc-raiders) public API: the event
//! schedule and map markers.
//!
//! Terms (<https://metaforge.app/arc-raiders/api>): attribution with a link
//! for public projects, contact them before any paid use, cache responses —
//! endpoints may change without notice. ARClens is non-commercial, credits
//! MetaForge wherever its data is shown, and caches every response.

use crate::Error;
use arclens_core::{MapId, MapPoint, Marker, ScheduledEvent};
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

/// MetaForge map ids (`mapID=`) and their in-game names.
pub const MAPS: &[(&str, &str)] = &[
    ("dam", "Dam Battlegrounds"),
    ("spaceport", "The Spaceport"),
    ("buried-city", "Buried City"),
    ("blue-gate", "The Blue Gate"),
    ("stella-montis", "Stella Montis"),
    ("riven-tides", "Riven Tides"),
];

/// Marker endpoint for one map (`id` from [`MAPS`]).
pub fn map_data_url(map: &str) -> String {
    format!("https://metaforge.app/api/game-map-data?tableID=arc_map_data&mapID={map}")
}

/// The response is documented as `{"allData": [...]}`; accept a bare array
/// or `{"data": [...]}` too, since the endpoint may change without notice.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MapDataResponse {
    All {
        #[serde(rename = "allData")]
        all_data: Vec<RawMarker>,
    },
    Data {
        data: Vec<RawMarker>,
    },
    Bare(Vec<RawMarker>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarker {
    id: serde_json::Value,
    #[serde(default)]
    lat: Option<Number>,
    #[serde(default)]
    lng: Option<Number>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    subcategory: Option<String>,
    #[serde(default)]
    instance_name: Option<String>,
    #[serde(default)]
    behind_locked_door: Option<bool>,
}

/// A coordinate, sent as a number or a numeric string.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Number {
    Float(f64),
    Text(String),
}

impl Number {
    #[allow(clippy::cast_possible_truncation, reason = "map pixels fit f32")]
    fn get(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v as f32),
            Self::Text(s) => s.trim().parse().ok(),
        }
        .filter(|v: &f32| v.is_finite())
    }
}

/// Parses a `game-map-data` response for `map`.
///
/// Leaflet `CRS.Simple` latitude grows upwards, so `y = -lat` (y down).
/// Records without coordinates are dropped; ones without a category land
/// in `"other"`.
pub fn parse_map_markers(bytes: &[u8], map: &str) -> Result<Vec<Marker>, Error> {
    let raw = match serde_json::from_slice(bytes)? {
        MapDataResponse::All { all_data: list }
        | MapDataResponse::Data { data: list }
        | MapDataResponse::Bare(list) => list,
    };
    Ok(raw
        .into_iter()
        .filter_map(|m| {
            let (lat, lng) = (m.lat?.get()?, m.lng?.get()?);
            let clean =
                |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
            Some(Marker {
                id: match m.id {
                    serde_json::Value::String(id) => id,
                    other => other.to_string(),
                },
                map: MapId::new(map),
                category: clean(m.category).unwrap_or_else(|| "other".to_owned()),
                subcategory: clean(m.subcategory),
                position: MapPoint::new(lng, -lat),
                label: clean(m.instance_name),
                locked: m.behind_locked_door.unwrap_or(false),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_map_markers() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/metaforge/map-data-dam.json"
        );
        let markers = parse_map_markers(&std::fs::read(path).unwrap(), "dam").unwrap();
        // One record has no coordinates.
        assert_eq!(markers.len(), 5);
        let queen = &markers[0];
        assert_eq!(queen.category, "arc");
        assert_eq!(queen.subcategory.as_deref(), Some("queen"));
        assert_eq!(queen.title(), "Queen");
        assert_eq!(queen.position, MapPoint::new(5211.4, -2495.7));
        assert_eq!(queen.map, MapId::new("dam"));
        let dome = markers.iter().find(|m| m.category == "labels").unwrap();
        assert_eq!(dome.title(), "Hydroponic Dome Complex");
        assert!(markers.iter().any(|m| m.locked));
        // String coordinates and numeric ids are accepted.
        assert!(markers.iter().any(|m| m.id == "42"));
        assert!(markers.iter().any(|m| m.category == "other"));
    }

    #[test]
    fn accepts_a_bare_array() {
        let markers =
            parse_map_markers(br#"[{"id":"a","lat":1,"lng":2,"category":"arc"}]"#, "dam").unwrap();
        assert_eq!(markers[0].position, MapPoint::new(2.0, -1.0));
    }

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
