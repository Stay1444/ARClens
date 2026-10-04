//! [MetaForge](https://metaforge.app/arc-raiders) public API: the event
//! schedule and map markers.
//!
//! Terms (<https://metaforge.app/arc-raiders/api>): attribution with a link
//! for public projects, contact them before any paid use, cache responses —
//! endpoints may change without notice. ARClens is non-commercial, credits
//! MetaForge in its README, and caches every response.

use crate::Error;
use arclens_core::{MapId, MapPoint, Marker, ScheduledEvent};
use serde::Deserialize;

/// Credit line, as used in the README.
pub const ATTRIBUTION: &str = "MetaForge (metaforge.app/arc-raiders)";
pub const EVENTS_SCHEDULE_URL: &str = "https://metaforge.app/api/arc-raiders/events-schedule";

/// Server regions the schedule can be asked for: `(id, name)`. Ids and
/// names as in MetaForge's own front end (its JS bundle in a capture of
/// 2026-10-03: `["europe","north-america","brazil","east-asia","oceania"]`,
/// unknown ids fall back to `europe`). Callers still compare
/// [`Schedule::region`] with what they asked for.
pub const REGIONS: &[(&str, &str)] = &[
    ("europe", "Europe"),
    ("north-america", "North America"),
    ("brazil", "South America"),
    ("east-asia", "Asia"),
    ("oceania", "Oceania"),
];

/// A region id as saved by older versions, mapped to MetaForge's id.
pub fn normalize_region(id: &str) -> &str {
    match id {
        "south-america" => "brazil",
        "asia" => "east-asia",
        other => other,
    }
}

/// The region MetaForge itself picks for an IANA time zone (same rules as
/// its front end, 2026-10-03), as a first guess before the player chooses.
/// `None` where it makes no guess (e.g. the Middle East).
pub fn region_for_time_zone(tz: &str) -> Option<&'static str> {
    // South American zones outside `America/Argentina/…`.
    const SOUTH_AMERICA: &[&str] = &[
        "America/Sao_Paulo",
        "America/Bahia",
        "America/Belem",
        "America/Fortaleza",
        "America/Manaus",
        "America/Recife",
        "America/Bogota",
        "America/Caracas",
        "America/Lima",
        "America/Santiago",
        "America/Montevideo",
        "America/Asuncion",
        "America/La_Paz",
        "America/Guayaquil",
        "America/Cayenne",
        "America/Paramaribo",
        "America/Guyana",
    ];
    const MIDDLE_EAST: &[&str] = &[
        "Asia/Dubai",
        "Asia/Baghdad",
        "Asia/Kuwait",
        "Asia/Muscat",
        "Asia/Nicosia",
        "Asia/Qatar",
        "Asia/Riyadh",
        "Asia/Tbilisi",
        "Asia/Tehran",
        "Asia/Tel_Aviv",
        "Asia/Jerusalem",
        "Asia/Yerevan",
        "Asia/Baku",
        "Asia/Amman",
        "Asia/Beirut",
        "Asia/Damascus",
        "Asia/Bahrain",
    ];
    match tz.split('/').next()? {
        "Europe" | "Africa" => Some("europe"),
        "Atlantic" => Some(match tz {
            "Atlantic/Bermuda" => "north-america",
            "Atlantic/Stanley" | "Atlantic/South_Georgia" => "brazil",
            _ => "europe",
        }),
        "America" => Some(
            if tz.starts_with("America/Argentina/") || SOUTH_AMERICA.contains(&tz) {
                "brazil"
            } else {
                "north-america"
            },
        ),
        "Asia" => (!MIDDLE_EAST.contains(&tz)).then_some("east-asia"),
        "Australia" | "Pacific" => Some("oceania"),
        _ => None,
    }
}

/// Schedule URL for a region (`None`: MetaForge's own choice).
pub fn events_schedule_url(region: Option<&str>) -> String {
    match region {
        Some(region) => format!("{EVENTS_SCHEDULE_URL}?region={region}"),
        None => EVENTS_SCHEDULE_URL.to_owned(),
    }
}

/// A parsed `events-schedule` response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schedule {
    pub events: Vec<ScheduledEvent>,
    /// The region the times are for, as the response states it.
    pub region: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    data: Vec<RawEvent>,
    #[serde(default)]
    region: Option<String>,
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
    Ok(parse_schedule(bytes)?.events)
}

/// Parses an `events-schedule` response body, with its region.
pub fn parse_schedule(bytes: &[u8]) -> Result<Schedule, Error> {
    let response: EventsResponse = serde_json::from_slice(bytes)?;
    let events = response
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
        .collect();
    Ok(Schedule {
        events,
        region: response.region,
    })
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

/// The map whose name starts the in-game map panel title, e.g.
/// `"DAM BATTLEGROUNDS - 18:45"` → `"dam"`. Tolerates OCR slips and a
/// missing "The".
pub fn map_for_title(title: &str) -> Option<&'static str> {
    // The raid clock follows the name, with or without a dash
    // ("BURIED CITY 26:03"): drop clock-like words (digits, colons, dashes);
    // keep the rest, OCR slips like "GR0UNDS" included.
    let is_clock = |w: &str| {
        w.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '-' | '—' | '–' | '.'))
    };
    let name: String = title
        .split_whitespace()
        .filter(|w| !is_clock(w))
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.strip_prefix("THE ").unwrap_or(&name);
    MAPS.iter()
        .map(|&(id, full)| {
            let full = full.to_uppercase();
            let full = full.strip_prefix("THE ").unwrap_or(&full).to_owned();
            (id, strsim::normalized_levenshtein(name, &full))
        })
        .filter(|&(_, score)| score >= 0.8)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Whether a schedule entry's `map` ("Dam", "Spaceport", "Blue Gate", …)
/// names the map with MetaForge id `map_id`.
pub fn event_on_map(event_map: &str, map_id: &str) -> bool {
    let squash = |s: &str| -> String {
        let s = s.to_lowercase();
        let s = s.strip_prefix("the ").unwrap_or(&s);
        s.chars().filter(char::is_ascii_alphanumeric).collect()
    };
    let event = squash(event_map);
    !event.is_empty()
        && MAPS.iter().any(|&(id, name)| {
            id == map_id && (squash(id) == event || squash(name).starts_with(&event))
        })
}

/// Each map's conditions and their bit in a marker's `eventConditionMask`,
/// from the `eventConditions` lists in MetaForge's map page bundle
/// (recorded 2026-10-03). The numbering differs per map (Cold Snap is 9 on
/// Dam, 11 on the Spaceport). Verified on Dam's data: hurricane caches
/// carry bit 12 (Hurricane), snow piles bit 9 (Cold Snap), husks bit 4
/// (Husk Graveyard), probes bit 1, assessors bit 14 (Close Scrutiny).
pub const CONDITIONS: &[(&str, &[(&str, u8)])] = &[
    (
        "dam",
        &[
            ("No Event", 0),
            ("Prospecting Probes", 1),
            ("Harvester", 2),
            ("Uncovered Caches", 3),
            ("Husk Graveyard", 4),
            ("Electromagnetic Storm", 5),
            ("Lush Blooms", 6),
            ("Night Raid", 7),
            ("Matriarch", 8),
            ("Cold Snap", 9),
            ("Hurricane", 12),
            ("Close Scrutiny", 14),
        ],
    ),
    (
        "spaceport",
        &[
            ("No Event", 0),
            ("Prospecting Probes", 1),
            ("Harvester", 2),
            ("Uncovered Caches", 3),
            ("Husk Graveyard", 4),
            ("Launch Tower Loot", 5),
            ("Lush Blooms", 6),
            ("Night Raid", 7),
            ("Electromagnetic Storm", 8),
            ("Hidden Bunker", 9),
            ("Matriarch", 10),
            ("Cold Snap", 11),
            ("Hurricane", 12),
            ("Close Scrutiny", 14),
        ],
    ),
    (
        "buried-city",
        &[
            ("No Event", 0),
            ("Prospecting Probes", 1),
            ("Uncovered Caches", 3),
            ("Husk Graveyard", 4),
            ("Lush Blooms", 6),
            ("Night Raid", 7),
            ("Cold Snap", 8),
            ("Hurricane", 12),
            ("Close Scrutiny", 14),
        ],
    ),
    (
        "blue-gate",
        &[
            ("No Event", 0),
            ("Harvester", 2),
            ("Uncovered Caches", 3),
            ("Husk Graveyard", 4),
            ("Lush Blooms", 6),
            ("Night Raid", 7),
            ("Electromagnetic Storm", 8),
            ("Cold Snap", 9),
            ("Matriarch", 10),
            ("Hurricane", 12),
            ("Locked Gate", 13),
            ("Close Scrutiny", 14),
        ],
    ),
    ("stella-montis", &[("No Event", 0), ("Night Raid", 7)]),
    (
        "riven-tides",
        &[("No Event", 0), ("Night Raid", 7), ("Beachcombing", 15)],
    ),
];

/// A map's conditions with their bits (empty for an unknown map).
pub fn conditions(map: &str) -> &'static [(&'static str, u8)] {
    CONDITIONS
        .iter()
        .find(|(id, _)| *id == map)
        .map_or(&[], |(_, list)| list)
}

/// The condition of `map` named by `text` (the map panel's condition line
/// as read, e.g. `"? Hurricane"`, or a schedule name), with its bit.
/// Tolerates OCR slips and stray symbols.
pub fn condition_on_map(map: &str, text: &str) -> Option<(&'static str, u8)> {
    let squash = |s: &str| -> String {
        s.chars()
            .filter(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_lowercase()
    };
    let text = squash(text);
    if text.is_empty() {
        return None;
    }
    conditions(map)
        .iter()
        .map(|&(name, bit)| {
            (
                (name, bit),
                strsim::normalized_levenshtein(&text, &squash(name)),
            )
        })
        .filter(|&(_, score)| score >= 0.8)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(condition, _)| condition)
}

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
    #[serde(default)]
    event_condition_mask: Option<serde_json::Value>,
    #[serde(default)]
    zlayers: Option<serde_json::Value>,
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

/// A marker's `eventConditionMask`: a bit set over the map's
/// [`CONDITIONS`]. Absent, `0` and `1` (bit 0 alone, "No Event") mean the
/// marker is there in every condition: `1` is what most ordinary markers
/// carry (1247 of 7662 on Dam, raider caches and lockers among them), so it
/// reads as the editor's default rather than "only without a condition".
fn condition_mask(value: Option<&serde_json::Value>) -> Option<u32> {
    let mask = match value? {
        serde_json::Value::Number(n) => n.as_u64()?,
        serde_json::Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    u32::try_from(mask).ok().filter(|&m| m > 1)
}

/// A marker's `zlayers`: the floors it is on, as a bit set (Stella
/// Montis labels carry 1 upper, 2 lower, 3 both). Absent, `0` and
/// `2147483647` (every bit, what single-floor maps carry) mean every floor.
fn floor_mask(value: Option<&serde_json::Value>) -> Option<u32> {
    let mask = match value? {
        serde_json::Value::Number(n) => n.as_u64()?,
        serde_json::Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    u32::try_from(mask)
        .ok()
        .filter(|&m| m != 0 && m != i32::MAX.unsigned_abs())
}

/// Parses a `game-map-data` response for `map`.
///
/// MetaForge's `lat` grows *downwards* on the map (verified 2026-10-03 on
/// the in-game Dam map: Victory Ridge, lat 1461, is at the top; Formicai
/// Hills, lat 3915, at the bottom), so `x = lng`, `y = lat`.
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
                position: MapPoint::new(lng, lat),
                label: clean(m.instance_name),
                locked: m.behind_locked_door.unwrap_or(false),
                conditions: condition_mask(m.event_condition_mask.as_ref()),
                floors: floor_mask(m.zlayers.as_ref()),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_use_metaforge_ids_and_old_ids_migrate() {
        let ids: Vec<&str> = REGIONS.iter().map(|r| r.0).collect();
        assert_eq!(
            ids,
            ["europe", "north-america", "brazil", "east-asia", "oceania"]
        );
        assert_eq!(normalize_region("south-america"), "brazil");
        assert_eq!(normalize_region("asia"), "east-asia");
        assert_eq!(normalize_region("europe"), "europe");
    }

    #[test]
    fn guesses_the_region_from_the_time_zone() {
        assert_eq!(region_for_time_zone("Europe/Madrid"), Some("europe"));
        assert_eq!(
            region_for_time_zone("America/New_York"),
            Some("north-america")
        );
        assert_eq!(region_for_time_zone("America/Sao_Paulo"), Some("brazil"));
        assert_eq!(
            region_for_time_zone("America/Argentina/Buenos_Aires"),
            Some("brazil")
        );
        assert_eq!(region_for_time_zone("Asia/Tokyo"), Some("east-asia"));
        assert_eq!(region_for_time_zone("Asia/Dubai"), None);
        assert_eq!(region_for_time_zone("Australia/Sydney"), Some("oceania"));
        assert_eq!(region_for_time_zone("UTC"), None);
    }

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
        assert_eq!(queen.position, MapPoint::new(5211.4, 2495.7));
        assert_eq!(queen.map, MapId::new("dam"));
        let dome = markers.iter().find(|m| m.category == "labels").unwrap();
        assert_eq!(dome.title(), "Hydroponic Dome Complex");
        assert!(markers.iter().any(|m| m.locked));
        // String coordinates and numeric ids are accepted.
        assert!(markers.iter().any(|m| m.id == "42"));
        assert!(markers.iter().any(|m| m.category == "other"));
    }

    #[test]
    fn finds_the_map_from_the_panel_title() {
        assert_eq!(map_for_title("DAM BATTLEGROUNDS - 18:45"), Some("dam"));
        assert_eq!(map_for_title("DAM BATTLEGR0UNDS — 9:02"), Some("dam"));
        assert_eq!(map_for_title("THE SPACEPORT - 12:00"), Some("spaceport"));
        assert_eq!(map_for_title("BLUE GATE"), Some("blue-gate"));
        assert_eq!(map_for_title("STELLA MONTIS - 1:00"), Some("stella-montis"));
        assert_eq!(map_for_title("BURIED CITY 26:03"), Some("buried-city"));
        assert_eq!(map_for_title("BURIED CITY -25:57"), Some("buried-city"));
        assert_eq!(map_for_title("Stay1444"), None);
    }

    #[test]
    fn reads_condition_masks() {
        let markers = parse_map_markers(
            br#"[{"id":"a","lat":1,"lng":2,"category":"containers","eventConditionMask":4096},
                {"id":"b","lat":1,"lng":2,"category":"containers","eventConditionMask":1},
                {"id":"c","lat":1,"lng":2,"category":"containers","eventConditionMask":"512"},
                {"id":"d","lat":1,"lng":2,"category":"containers","eventConditionMask":null}]"#,
            "dam",
        )
        .unwrap();
        let masks: Vec<_> = markers.iter().map(|m| m.conditions).collect();
        assert_eq!(masks, [Some(4096), None, Some(512), None]);
        let (_, hurricane) = condition_on_map("dam", "Hurricane").unwrap();
        assert!(markers[0].occurs_in(Some(hurricane)));
        assert!(!markers[0].occurs_in(Some(0)));
    }

    #[test]
    fn reads_floor_masks() {
        let markers = parse_map_markers(
            br#"[{"id":"a","lat":1,"lng":2,"zlayers":1},
                {"id":"b","lat":1,"lng":2,"zlayers":"2"},
                {"id":"c","lat":1,"lng":2,"zlayers":3},
                {"id":"d","lat":1,"lng":2,"zlayers":2147483647},
                {"id":"e","lat":1,"lng":2}]"#,
            "stella-montis",
        )
        .unwrap();
        let masks: Vec<_> = markers.iter().map(|m| m.floors).collect();
        assert_eq!(masks, [Some(1), Some(2), Some(3), None, None]);
        let lower: Vec<_> = markers.iter().map(|m| m.on_floor(Some(2))).collect();
        assert_eq!(lower, [false, true, true, true, true]);
    }

    #[test]
    fn names_the_condition_per_map() {
        assert_eq!(
            condition_on_map("dam", "? Hurricane"),
            Some(("Hurricane", 12))
        );
        assert_eq!(condition_on_map("dam", "COLD SNAP"), Some(("Cold Snap", 9)));
        assert_eq!(
            condition_on_map("spaceport", "Cold Snap"),
            Some(("Cold Snap", 11))
        );
        assert_eq!(
            condition_on_map("dam", "Husk Graveyrd"),
            Some(("Husk Graveyard", 4))
        );
        assert_eq!(condition_on_map("stella-montis", "Hurricane"), None);
        assert_eq!(condition_on_map("dam", "??"), None);
        assert_eq!(condition_on_map("nowhere", "Hurricane"), None);
        // Every map we list has a condition table.
        assert!(MAPS.iter().all(|(id, _)| !conditions(id).is_empty()));
    }

    #[test]
    fn matches_schedule_map_names() {
        assert!(event_on_map("Dam", "dam"));
        assert!(event_on_map("Dam Battlegrounds", "dam"));
        assert!(event_on_map("The Spaceport", "spaceport"));
        assert!(event_on_map("Blue Gate", "blue-gate"));
        assert!(!event_on_map("Blue Gate", "dam"));
        assert!(!event_on_map("", "dam"));
    }

    #[test]
    fn reads_the_schedule_region() {
        let schedule = parse_schedule(br#"{"data":[],"cachedAt":1,"region":"europe"}"#).unwrap();
        assert_eq!(schedule.region.as_deref(), Some("europe"));
        assert_eq!(
            events_schedule_url(Some("asia")),
            "https://metaforge.app/api/arc-raiders/events-schedule?region=asia"
        );
        assert_eq!(events_schedule_url(None), EVENTS_SCHEDULE_URL);
    }

    #[test]
    fn accepts_a_bare_array() {
        let markers =
            parse_map_markers(br#"[{"id":"a","lat":1,"lng":2,"category":"arc"}]"#, "dam").unwrap();
        assert_eq!(markers[0].position, MapPoint::new(2.0, 1.0));
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
