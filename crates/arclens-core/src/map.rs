//! Maps, markers and the coordinate spaces between them.
//!
//! Three coordinate spaces exist:
//!
//! * **Source space** — whatever the upstream marker dataset uses (often
//!   pixels of that site's map image, or game world units).
//! * **Map space** ([`MapPoint`]) — normalised `[0, 1]²` over ARClens' own map
//!   image, origin top-left. Everything inside ARClens uses this.
//! * **Screen space** — pixels on the player's monitor. Only the overlay
//!   renderer deals with this, via a [`Transform`] produced by calibration.

use serde::{Deserialize, Serialize};

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

/// A point in normalised map space (`0.0..=1.0` on both axes, origin top-left).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapPoint {
    pub x: f32,
    pub y: f32,
}

impl MapPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Whether the point lies on the map.
    pub fn is_on_map(self) -> bool {
        (0.0..=1.0).contains(&self.x) && (0.0..=1.0).contains(&self.y)
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

/// What a marker represents. Unknown upstream kinds are kept as `Other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    Extraction,
    RaiderHatch,
    Loot,
    Container,
    QuestObjective,
    Arc,
    Spawn,
    Other(String),
}

/// A point of interest on a map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: String,
    pub map: MapId,
    pub kind: MarkerKind,
    pub position: MapPoint,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
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

    #[test]
    fn on_map_bounds_are_inclusive() {
        assert!(MapPoint::new(0.0, 1.0).is_on_map());
        assert!(!MapPoint::new(-0.01, 0.5).is_on_map());
    }
}
