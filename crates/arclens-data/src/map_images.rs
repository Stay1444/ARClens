//! The maps' images (RaidTheory's tiles) and how they line up with the
//! marker coordinates: `image_px = scale · map + (tx, ty)` at the full
//! resolution, per map (`data/map-image-transforms.json`; method and
//! residuals in `docs/research/map-images.md`).

use arclens_core::MapPoint;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const TRANSFORMS_JSON: &str = include_str!("../data/map-image-transforms.json");

/// Where RaidTheory's dataset files are served from.
pub const RAW_BASE: &str = "https://raw.githubusercontent.com/RaidTheory/arcraiders-data/main/";

/// A map drawn from square tiles (`{z}/{x}/{y}`, x the column), with its
/// alignment to marker coordinates.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MapImage {
    /// Path template of the full-quality tiles (zoom 2 and up).
    pub tiles: String,
    /// Path template of the small tiles (zoom 0 and 1).
    #[serde(default)]
    pub low_tiles: Option<String>,
    pub tile_size: u32,
    pub max_zoom: u32,
    /// Image size at `max_zoom`, pixels.
    pub size: [u32; 2],
    pub scale: f32,
    pub tx: f32,
    pub ty: f32,
}

impl MapImage {
    /// Image size at `zoom`, pixels.
    pub fn size_at(&self, zoom: u32) -> (u32, u32) {
        let div = 1 << self.max_zoom.saturating_sub(zoom);
        (self.size[0] / div, self.size[1] / div)
    }

    /// Every tile of `zoom` as `(column, row, url)`.
    pub fn tile_urls(&self, zoom: u32) -> Vec<(u32, u32, String)> {
        let template = match &self.low_tiles {
            Some(low) if zoom <= 1 => low,
            _ => &self.tiles,
        };
        let (w, h) = self.size_at(zoom);
        let (cols, rows) = (w.div_ceil(self.tile_size), h.div_ceil(self.tile_size));
        (0..cols)
            .flat_map(|x| (0..rows).map(move |y| (x, y)))
            .map(|(x, y)| {
                let path = template
                    .replace("{z}", &zoom.to_string())
                    .replace("{x}", &x.to_string())
                    .replace("{y}", &y.to_string());
                (x, y, format!("{RAW_BASE}{path}"))
            })
            .collect()
    }

    /// The image's corners in marker coordinates: top-left, bottom-right.
    pub fn bounds(&self) -> (MapPoint, MapPoint) {
        #[allow(clippy::cast_precision_loss, reason = "image sizes")]
        let (w, h) = (self.size[0] as f32, self.size[1] as f32);
        (
            MapPoint::new(-self.tx / self.scale, -self.ty / self.scale),
            MapPoint::new((w - self.tx) / self.scale, (h - self.ty) / self.scale),
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    Tiled(MapImage),
    /// Maps made of separate images per floor (Stella Montis): not drawn
    /// yet.
    Other(serde::de::IgnoredAny),
}

fn all() -> &'static BTreeMap<String, Entry> {
    static ALL: OnceLock<BTreeMap<String, Entry>> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(TRANSFORMS_JSON).unwrap_or_default())
}

/// The tiled image of `map` (MetaForge id), if it has one.
pub fn map_image(map: &str) -> Option<&'static MapImage> {
    match all().get(map)? {
        Entry::Tiled(image) => Some(image),
        Entry::Other(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dam_tiles_and_bounds() {
        let dam = map_image("dam").unwrap();
        assert_eq!(dam.size_at(1), (2000, 2000));
        let tiles = dam.tile_urls(1);
        assert_eq!(tiles.len(), 16);
        assert!(tiles[0].2.ends_with("dam-battleground/v2/low/1/0/0.webp"));
        // The bounds map back onto the image corners.
        let (min, max) = dam.bounds();
        assert!((dam.scale * min.x + dam.tx).abs() < 1e-2);
        assert!((dam.scale * max.y + dam.ty - 8000.0).abs() < 1e-2);
    }

    #[test]
    fn tiled_maps_are_listed_and_layered_ones_skipped() {
        for map in [
            "dam",
            "buried-city",
            "blue-gate",
            "spaceport",
            "riven-tides",
        ] {
            assert!(map_image(map).is_some(), "{map}");
        }
        assert!(map_image("stella-montis").is_none());
        assert!(map_image("nowhere").is_none());
    }
}
