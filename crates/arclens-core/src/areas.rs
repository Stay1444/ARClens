//! Dense groups of same-kind markers drawn as one area.
//!
//! Some sources mark every *candidate* spot of a randomised spawn (884
//! raider caches on Dam form tight blobs), and buildings hold dozens of
//! lockers. One translucent shape with a count reads far better than forty
//! overlapping icons. Groups are found per kind with DBSCAN.

use crate::{MapPoint, Marker};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Neighbourhood radius, in source map units. Tuned on MetaForge's Dam
/// data (2026-10-03): with [`AREA_MIN`] = 5 it puts 95 % of the 884 raider
/// caches into 47 areas (median radius ~58), matching the blobs seen on the
/// map, without merging neighbouring ones.
pub const AREA_RADIUS: f32 = 45.0;
/// Markers needed within [`AREA_RADIUS`] to seed an area.
pub const AREA_MIN: usize = 5;

/// A dense group of markers of one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerArea {
    pub category: String,
    #[serde(default)]
    pub subcategory: Option<String>,
    /// Mean position of the members.
    pub center: MapPoint,
    /// Convex outline of the members, counter-clockwise.
    pub hull: Vec<MapPoint>,
    pub count: usize,
}

/// Markers split into areas and the ones drawn on their own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarkerLayout {
    pub areas: Vec<MarkerArea>,
    /// Indices (into the markers given) of markers outside any area.
    pub singles: Vec<usize>,
}

/// Groups `indices` of `markers` into areas, per category and subcategory.
pub fn layout(markers: &[Marker], indices: impl IntoIterator<Item = usize>) -> MarkerLayout {
    let mut kinds: BTreeMap<(&str, Option<&str>), Vec<usize>> = BTreeMap::new();
    for i in indices {
        let m = &markers[i];
        kinds
            .entry((m.category.as_str(), m.subcategory.as_deref()))
            .or_default()
            .push(i);
    }
    let mut out = MarkerLayout::default();
    for ((category, subcategory), members) in kinds {
        let points: Vec<MapPoint> = members.iter().map(|&i| markers[i].position).collect();
        let labels = dbscan(&points, AREA_RADIUS, AREA_MIN);
        let mut groups: BTreeMap<usize, Vec<MapPoint>> = BTreeMap::new();
        for (k, label) in labels.iter().enumerate() {
            match label {
                Some(group) => groups.entry(*group).or_default().push(points[k]),
                None => out.singles.push(members[k]),
            }
        }
        for points in groups.into_values() {
            #[allow(clippy::cast_precision_loss, reason = "marker counts are small")]
            let n = points.len() as f32;
            out.areas.push(MarkerArea {
                category: category.to_owned(),
                subcategory: subcategory.map(str::to_owned),
                center: MapPoint::new(
                    points.iter().map(|p| p.x).sum::<f32>() / n,
                    points.iter().map(|p| p.y).sum::<f32>() / n,
                ),
                hull: convex_hull(&points),
                count: points.len(),
            });
        }
    }
    out.singles.sort_unstable();
    out
}

/// Cluster label per point (`None` = noise). O(n²): fine for the few
/// thousand markers of a kind, computed once per data or filter change.
fn dbscan(points: &[MapPoint], eps: f32, min_points: usize) -> Vec<Option<usize>> {
    let near = |a: MapPoint, b: MapPoint| (a.x - b.x).hypot(a.y - b.y) <= eps;
    let neighbours: Vec<Vec<usize>> = points
        .iter()
        .map(|&p| (0..points.len()).filter(|&j| near(p, points[j])).collect())
        .collect();
    let mut labels = vec![None; points.len()];
    let mut next = 0;
    for seed in 0..points.len() {
        if labels[seed].is_some() || neighbours[seed].len() < min_points {
            continue;
        }
        labels[seed] = Some(next);
        let mut stack = vec![seed];
        while let Some(k) = stack.pop() {
            if neighbours[k].len() < min_points {
                continue; // border point: joins, doesn't expand
            }
            for &j in &neighbours[k] {
                if labels[j].is_none() {
                    labels[j] = Some(next);
                    stack.push(j);
                }
            }
        }
        next += 1;
    }
    labels
}

/// Convex hull (Andrew's monotone chain), counter-clockwise.
fn convex_hull(points: &[MapPoint]) -> Vec<MapPoint> {
    let mut pts: Vec<MapPoint> = points.to_vec();
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: MapPoint, a: MapPoint, b: MapPoint| {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    };
    let mut hull: Vec<MapPoint> = Vec::with_capacity(pts.len() * 2);
    for pass in [false, true] {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &MapPoint>> = if pass {
            Box::new(pts.iter().rev())
        } else {
            Box::new(pts.iter())
        };
        for &p in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop(); // the next pass starts with this point
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MapId;

    fn marker(sub: &str, x: f32, y: f32) -> Marker {
        Marker {
            id: format!("{sub}-{x}-{y}"),
            map: MapId::new("dam"),
            category: "containers".into(),
            subcategory: Some(sub.into()),
            position: MapPoint::new(x, y),
            label: None,
            locked: false,
            conditions: None,
        }
    }

    /// A blob of `n` points around `(cx, cy)` within ~10 units.
    fn blob(sub: &str, cx: f32, cy: f32, n: usize) -> Vec<Marker> {
        (0..n)
            .map(|i| {
                #[allow(clippy::cast_precision_loss, reason = "test data")]
                let a = i as f32 * 0.9;
                marker(sub, cx + 10.0 * a.cos(), cy + 10.0 * a.sin())
            })
            .collect()
    }

    #[test]
    fn dense_blobs_become_areas_and_the_rest_stay_single() {
        let mut markers = blob("raider_cache", 0.0, 0.0, 12);
        markers.extend(blob("raider_cache", 500.0, 500.0, 8));
        markers.push(marker("raider_cache", 1000.0, 0.0));
        markers.push(marker("raider_cache", 0.0, 1000.0));
        // Same place, other kind: not merged into the cache area.
        markers.push(marker("weapon_case", 1.0, 1.0));

        let layout = layout(&markers, 0..markers.len());
        let mut counts: Vec<usize> = layout.areas.iter().map(|a| a.count).collect();
        counts.sort_unstable();
        assert_eq!(counts, vec![8, 12]);
        assert_eq!(layout.singles.len(), 3);
        let big = layout.areas.iter().find(|a| a.count == 12).unwrap();
        assert!(big.center.x.abs() < 2.0 && big.center.y.abs() < 2.0);
        assert!(big.hull.len() >= 3);
    }

    #[test]
    fn sparse_markers_are_never_grouped() {
        let markers: Vec<Marker> = (0..20)
            .map(|i| {
                #[allow(clippy::cast_precision_loss, reason = "test data")]
                let x = i as f32 * 100.0;
                marker("locker", x, 0.0)
            })
            .collect();
        let layout = layout(&markers, 0..markers.len());
        assert!(layout.areas.is_empty());
        assert_eq!(layout.singles.len(), 20);
    }

    #[test]
    fn hull_is_the_outline() {
        let square = [
            MapPoint::new(0.0, 0.0),
            MapPoint::new(10.0, 0.0),
            MapPoint::new(10.0, 10.0),
            MapPoint::new(0.0, 10.0),
            MapPoint::new(5.0, 5.0), // inside
        ];
        let hull = convex_hull(&square);
        assert_eq!(hull.len(), 4);
        assert!(!hull.contains(&MapPoint::new(5.0, 5.0)));
    }
}
