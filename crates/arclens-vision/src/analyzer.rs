//! Per-frame analysis with caching: the piece a capture loop calls.

use crate::{
    FooterInfo, NameReader, PanelParams, Rect, find_panels, footer, name_lines, parse_value,
    value_cells,
};
use image::RgbImage;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// The tooltip under the cursor, as found in one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Recognised name text (not yet matched to the catalogue).
    pub name: String,
    /// The tooltip panel, in frame pixels.
    pub panel: Rect,
    /// Frame size, so consumers can normalise `panel`.
    pub frame_width: u32,
    pub frame_height: u32,
    /// What the tooltip footer says (value, raid or not), if it was found.
    pub footer: Option<FooterInfo>,
}

impl Hover {
    /// Same item and footer, tooltip in (almost) the same place: nothing
    /// worth re-sending. Tolerates a few pixels of detection jitter.
    pub fn same_as(&self, other: &Self) -> bool {
        const JITTER: u32 = 8;
        self.name == other.name
            && self.footer == other.footer
            && self.panel.x.abs_diff(other.panel.x) <= JITTER
            && self.panel.y.abs_diff(other.panel.y) <= JITTER
    }

    /// `panel` as `[x, y, width, height]` fractions of the frame.
    pub fn panel_normalized(&self) -> [f32; 4] {
        let (w, h) = (self.frame_width as f32, self.frame_height as f32);
        [
            self.panel.x as f32 / w,
            self.panel.y as f32 / h,
            self.panel.width as f32 / w,
            self.panel.height as f32 / h,
        ]
    }
}

/// Distinct tooltips remembered before the cache is reset (a stash holds a
/// few hundred items; this is a few KB).
const CACHE_LIMIT: usize = 2048;

/// What OCR made of one tooltip: its name and footer.
type Reading = (String, Option<FooterInfo>);

/// Runs detection on every frame and OCR only when the name changed.
#[derive(Debug)]
pub struct Analyzer {
    reader: NameReader,
    params: PanelParams,
    /// Readings by fingerprint of the name + value crops. Re-hovering an
    /// item seen this session skips OCR entirely.
    cache: std::collections::HashMap<u64, Option<Reading>>,
}

impl Analyzer {
    pub fn new(reader: NameReader) -> Self {
        Self {
            reader,
            params: PanelParams::default(),
            cache: std::collections::HashMap::new(),
        }
    }

    /// The hovered tooltip in `frame`, if any.
    ///
    /// When several panels are visible (trader screen: persistent purchase
    /// panel + hover tooltip) the **smallest** is taken: in every capture so
    /// far the hover tooltip is smaller than the purchase panel.
    pub fn analyze(&mut self, frame: &RgbImage) -> anyhow::Result<Option<Hover>> {
        let mut candidates: Vec<(Rect, Vec<Rect>)> = find_panels(frame, &self.params)
            .into_iter()
            .map(|panel| (panel, name_lines(frame, panel)))
            .filter(|(_, lines)| !lines.is_empty())
            .collect();
        candidates.sort_by_key(|(panel, _)| panel.area());
        let Some((panel, lines)) = candidates.into_iter().next() else {
            return Ok(None);
        };

        let cells = footer(frame, panel).map(|f| value_cells(frame, f));
        // Two copies of an item can differ in value (durability), so the
        // value cell is part of the cache key, not just the name.
        let mut regions = lines.clone();
        if let Some(cells) = &cells {
            regions.extend(cells.iter().copied());
        }
        let key = fingerprint(frame, &regions);
        let read = if let Some(read) = self.cache.get(&key) {
            read.clone()
        } else {
            let read = self
                .reader
                .read(frame, &lines)?
                .map(|name| -> anyhow::Result<_> {
                    Ok((name, self.read_footer(frame, cells.as_deref())?))
                })
                .transpose()?;
            if self.cache.len() >= CACHE_LIMIT {
                self.cache.clear();
            }
            self.cache.insert(key, read.clone());
            read
        };
        Ok(read.map(|(name, footer)| Hover {
            name,
            panel,
            frame_width: frame.width(),
            frame_height: frame.height(),
            footer,
        }))
    }

    /// Reads the value cell: `[weight]` → in raid, `[weight, value]` → menu.
    fn read_footer(
        &self,
        frame: &RgbImage,
        cells: Option<&[Rect]>,
    ) -> anyhow::Result<Option<FooterInfo>> {
        let Some(cells) = cells else {
            return Ok(None);
        };
        Ok(match cells {
            [] => None,
            [_weight] => Some(FooterInfo {
                sell_value: None,
                in_raid: true,
            }),
            [.., value] => Some(FooterInfo {
                sell_value: self
                    .reader
                    .read_text(frame, *value)?
                    .as_deref()
                    .and_then(parse_value),
                in_raid: false,
            }),
        })
    }
}

/// Cheap fingerprint of the name lines: geometry plus a coarse, quantised
/// sample of their pixels. Identical tooltips across frames hash equal even
/// with a little compression noise.
fn fingerprint(frame: &RgbImage, lines: &[Rect]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for line in lines {
        (line.width / 4, line.height / 4).hash(&mut hasher);
        for gy in 0..4 {
            for gx in 0..16 {
                let px =
                    frame.get_pixel(line.x + line.width * gx / 16, line.y + line.height * gy / 4);
                let luma: u16 = px.0.iter().map(|&c| u16::from(c)).sum();
                (luma / 96).hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hover(name: &str, x: u32, value: Option<u32>) -> Hover {
        Hover {
            name: name.into(),
            panel: Rect::new(x, 300, 508, 700),
            frame_width: 2560,
            frame_height: 1440,
            footer: Some(FooterInfo {
                sell_value: value,
                in_raid: false,
            }),
        }
    }

    #[test]
    fn small_jitter_is_the_same_hover() {
        assert!(hover("OSPREY II", 800, Some(18_431)).same_as(&hover(
            "OSPREY II",
            804,
            Some(18_431)
        )));
    }

    #[test]
    fn another_slot_or_value_is_a_new_hover() {
        let a = hover("MEDIUM AMMO", 800, Some(480));
        assert!(!a.same_as(&hover("MEDIUM AMMO", 1200, Some(480))));
        assert!(!a.same_as(&hover("MEDIUM AMMO", 800, Some(240))));
        assert!(!a.same_as(&hover("LIGHT AMMO", 800, Some(480))));
    }
}
