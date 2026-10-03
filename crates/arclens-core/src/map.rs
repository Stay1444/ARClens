//! Maps, markers and the coordinate spaces between them.
//!
//! Two coordinate spaces exist:
//!
//! * **Map space** ([`MapPoint`]): the marker source's own units, with y
//!   pointing down. For MetaForge these are pixels of its Leaflet map image
//!   (`x = lng`, `y = -lat`). Markers of one map share one space.
//! * **Screen space**: pixels on the player's monitor. Only the overlay
//!   renderer deals with this, via a [`Transform`] fitted to the in-game
//!   map view.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable identifier of a map, e.g. `"dam-battlegrounds"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapId(pub String);

impl MapId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A point in map space (the marker source's units, y down).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapPoint {
    pub x: f32,
    pub y: f32,
}

impl MapPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D affine transform `p' = [a b; c d] · p + [tx ty]`.
///
/// Used both to import source coordinates into map space and to project map
/// space onto the screen once the in-game map view has been calibrated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Axis-aligned scale followed by translation — enough for a map view
    /// that can only be panned and zoomed.
    pub const fn scale_translate(sx: f32, sy: f32, tx: f32, ty: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx,
            ty,
        }
    }

    /// Builds the pan/zoom transform that maps `from[i]` onto `to[i]` for two
    /// reference points (e.g. two landmarks the user clicked during
    /// calibration). Returns `None` if the points are degenerate on an axis.
    pub fn from_two_points(from: [(f32, f32); 2], to: [(f32, f32); 2]) -> Option<Self> {
        let dx = from[1].0 - from[0].0;
        let dy = from[1].1 - from[0].1;
        if dx.abs() < f32::EPSILON || dy.abs() < f32::EPSILON {
            return None;
        }
        let sx = (to[1].0 - to[0].0) / dx;
        let sy = (to[1].1 - to[0].1) / dy;
        Some(Self::scale_translate(
            sx,
            sy,
            to[0].0 - sx * from[0].0,
            to[0].1 - sy * from[0].1,
        ))
    }

    pub fn apply(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        )
    }
}

/// A point of interest on a map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: String,
    pub map: MapId,
    /// Top-level group, as the source names it (`"arc"`, `"containers"`, …).
    pub category: String,
    /// Finer kind within the category (`"queen"`, `"weapon_case"`, …).
    #[serde(default)]
    pub subcategory: Option<String>,
    pub position: MapPoint,
    /// Proper name, for named places ("Hydroponic Dome Complex").
    #[serde(default)]
    pub label: Option<String>,
    /// Behind a locked door (needs a key).
    #[serde(default)]
    pub locked: bool,
}

impl Marker {
    /// What to call the marker: its name, else its kind.
    pub fn title(&self) -> String {
        match (&self.label, &self.subcategory) {
            (Some(label), _) if !label.trim().is_empty() => label.trim().to_owned(),
            (_, Some(sub)) => humanize(sub),
            _ => humanize(&self.category),
        }
    }

    /// Whether `query` (case-insensitive) appears in its name or kind.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.title().to_lowercase().contains(&query)
            || humanize(&self.category).to_lowercase().contains(&query)
            || self
                .subcategory
                .as_deref()
                .is_some_and(|s| humanize(s).to_lowercase().contains(&query))
    }
}

/// `"weapon_case"` / `"husk-graveyard"` → `"Weapon Case"` / `"Husk Graveyard"`.
pub fn humanize(id: &str) -> String {
    id.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Which marker kinds the player hides. Everything is shown by default, so
/// new kinds from the source appear without the player opting in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerFilter {
    /// Hidden categories (`"arc"`) and subcategories (`"arc/queen"`).
    #[serde(default)]
    pub hidden: BTreeSet<String>,
}

impl MarkerFilter {
    pub fn shows(&self, marker: &Marker) -> bool {
        self.shows_category(&marker.category)
            && marker
                .subcategory
                .as_deref()
                .is_none_or(|sub| self.shows_subcategory(&marker.category, sub))
    }

    pub fn shows_category(&self, category: &str) -> bool {
        !self.hidden.contains(category)
    }

    pub fn shows_subcategory(&self, category: &str, subcategory: &str) -> bool {
        !self.hidden.contains(&sub_key(category, subcategory))
    }

    pub fn toggle_category(&mut self, category: &str) {
        toggle(&mut self.hidden, category.to_owned());
    }

    pub fn toggle_subcategory(&mut self, category: &str, subcategory: &str) {
        toggle(&mut self.hidden, sub_key(category, subcategory));
    }

    pub fn show_all(&mut self) {
        self.hidden.clear();
    }

    /// Hides every category in `markers`.
    pub fn hide_all(&mut self, markers: &[Marker]) {
        self.hidden
            .extend(markers.iter().map(|m| m.category.clone()));
    }
}

fn sub_key(category: &str, subcategory: &str) -> String {
    format!("{category}/{subcategory}")
}

fn toggle(set: &mut BTreeSet<String>, key: String) {
    if !set.remove(&key) {
        set.insert(key);
    }
}

/// Marker counts per category, then per subcategory (`""` for none), sorted
/// by name.
pub fn marker_counts(markers: &[Marker]) -> BTreeMap<&str, BTreeMap<&str, usize>> {
    let mut counts: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for marker in markers {
        *counts
            .entry(marker.category.as_str())
            .or_default()
            .entry(marker.subcategory.as_deref().unwrap_or(""))
            .or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() <= 1e-3 * a.0.abs().max(1.0)
            && (a.1 - b.1).abs() <= 1e-3 * a.1.abs().max(1.0)
    }

    #[test]
    fn identity_is_a_no_op() {
        assert_eq!(Transform::IDENTITY.apply((0.3, 0.7)), (0.3, 0.7));
    }

    #[test]
    fn two_point_calibration_round_trips_reference_points() {
        let from = [(0.1, 0.2), (0.9, 0.8)];
        let to = [(400.0, 300.0), (1600.0, 900.0)];
        let t = Transform::from_two_points(from, to).expect("non-degenerate");
        assert!(close(t.apply(from[0]), to[0]));
        assert!(close(t.apply(from[1]), to[1]));
        assert!(close(t.apply((0.5, 0.5)), (1000.0, 600.0)));
    }

    #[test]
    fn two_point_calibration_rejects_degenerate_points() {
        assert!(
            Transform::from_two_points([(0.5, 0.1), (0.5, 0.9)], [(0.0, 0.0), (1.0, 1.0)])
                .is_none()
        );
    }

    fn marker(category: &str, sub: Option<&str>, label: Option<&str>) -> Marker {
        Marker {
            id: format!("{category}-{sub:?}-{label:?}"),
            map: MapId::new("dam"),
            category: category.into(),
            subcategory: sub.map(Into::into),
            position: MapPoint::new(0.0, 0.0),
            label: label.map(Into::into),
            locked: false,
        }
    }

    #[test]
    fn title_prefers_the_name_then_the_kind() {
        let dome = marker("locations", Some("poi"), Some("Hydroponic Dome Complex"));
        assert_eq!(dome.title(), "Hydroponic Dome Complex");
        assert_eq!(
            marker("containers", Some("weapon_case"), None).title(),
            "Weapon Case"
        );
        assert_eq!(
            marker("husk-graveyard", None, Some(" ")).title(),
            "Husk Graveyard"
        );
    }

    #[test]
    fn search_covers_name_category_and_subcategory() {
        let queen = marker("arc", Some("queen"), None);
        assert!(queen.matches(""));
        assert!(queen.matches("QUE"));
        assert!(queen.matches("arc"));
        assert!(!queen.matches("dome"));
    }

    #[test]
    fn filter_hides_categories_and_subcategories() {
        let queen = marker("arc", Some("queen"), None);
        let tick = marker("arc", Some("tick"), None);
        let case = marker("containers", None, None);
        let mut filter = MarkerFilter::default();
        assert!(filter.shows(&queen) && filter.shows(&case));

        filter.toggle_subcategory("arc", "tick");
        assert!(filter.shows(&queen) && !filter.shows(&tick));

        filter.toggle_category("arc");
        assert!(!filter.shows(&queen) && filter.shows(&case));

        filter.toggle_category("arc");
        filter.toggle_subcategory("arc", "tick");
        assert!(filter.shows(&tick));

        let all = [queen.clone(), case.clone()];
        filter.hide_all(&all);
        assert!(!filter.shows(&queen) && !filter.shows(&case));
        filter.show_all();
        assert!(filter.shows(&queen));
    }

    #[test]
    fn counts_group_by_category_then_subcategory() {
        let markers = [
            marker("arc", Some("queen"), None),
            marker("arc", Some("tick"), None),
            marker("arc", Some("tick"), None),
            marker("containers", None, None),
        ];
        let counts = marker_counts(&markers);
        assert_eq!(counts["arc"]["tick"], 2);
        assert_eq!(counts["arc"]["queen"], 1);
        assert_eq!(counts["containers"][""], 1);
    }
}
