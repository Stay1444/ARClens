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
    /// Which side of the tooltip the hovered item is on.
    pub item_side: crate::Side,
    /// What the tooltip footer says (value, raid or not), if it was found.
    pub footer: Option<FooterInfo>,
    /// Other cream panels on screen (the trader's purchase panel): a card
    /// placed beside the tooltip shouldn't cover them.
    pub others: Vec<Rect>,
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
        self.normalize(self.panel)
    }

    /// `rect` (frame pixels) as `[x, y, width, height]` fractions.
    pub fn normalize(&self, rect: Rect) -> [f32; 4] {
        let (w, h) = (self.frame_width as f32, self.frame_height as f32);
        [
            rect.x as f32 / w,
            rect.y as f32 / h,
            rect.width as f32 / w,
            rect.height as f32 / h,
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
    /// Map label text by crop content (not position): panning moves labels
    /// without changing them, so only newly visible ones are read.
    labels: std::collections::HashMap<u64, Option<String>>,
    label_params: crate::LabelParams,
}

/// A place name read off the in-game map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapLabel {
    pub text: String,
    /// Where it is, in frame pixels.
    pub rect: Rect,
}

impl Analyzer {
    pub fn new(reader: NameReader) -> Self {
        Self {
            reader,
            params: PanelParams::default(),
            cache: std::collections::HashMap::new(),
            labels: std::collections::HashMap::new(),
            label_params: crate::LabelParams::default(),
        }
    }

    /// The map labels in `frame`, read (OCR) only when not seen before.
    pub fn read_map_labels(&mut self, frame: &RgbImage) -> anyhow::Result<Vec<MapLabel>> {
        let mut out = Vec::new();
        for rect in crate::find_map_labels(frame, &self.label_params) {
            let key = content_fingerprint(frame, rect);
            let text = if let Some(text) = self.labels.get(&key) {
                text.clone()
            } else {
                let text = self.reader.read_free_text(frame, rect)?;
                if self.labels.len() >= CACHE_LIMIT {
                    self.labels.clear();
                }
                self.labels.insert(key, text.clone());
                text
            };
            if let Some(text) = text {
                out.push(MapLabel {
                    text: text.trim().to_owned(),
                    rect,
                });
            }
        }
        Ok(out)
    }

    /// The map panel header, if `frame` shows the map screen (OCR; call it
    /// sparingly, e.g. every few seconds while [`crate::is_map_screen`]).
    pub fn read_map_header(&self, frame: &RgbImage) -> anyhow::Result<Option<crate::MapHeader>> {
        crate::read_map_header(&self.reader, frame)
    }

    /// The workshop station page's title and level, if `frame` shows one
    /// (OCR when there is header text; call it every few seconds).
    pub fn read_station_header(
        &self,
        frame: &RgbImage,
    ) -> anyhow::Result<Option<crate::StationLevel>> {
        crate::read_station_header(&self.reader, frame)
    }

    /// The quests in progress, if `frame` shows the logbook or a trader's
    /// quest page (OCR; call it every few seconds).
    pub fn read_active_quests(&self, frame: &RgbImage) -> anyhow::Result<Vec<String>> {
        crate::read_active_quests(&self.reader, frame)
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
        let mut candidates = candidates.into_iter();
        let Some((panel, lines)) = candidates.next() else {
            return Ok(None);
        };
        let others: Vec<Rect> = candidates.map(|(panel, _)| panel).collect();

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
            item_side: crate::item_side(frame, panel),
            footer,
            others,
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

/// Fingerprint of a crop's content alone (size and a coarse pixel sample),
/// independent of where it is on screen.
fn content_fingerprint(frame: &RgbImage, rect: Rect) -> u64 {
    let mut hasher = DefaultHasher::new();
    (rect.width / 3, rect.height / 3).hash(&mut hasher);
    for gy in 0..4 {
        for gx in 0..24 {
            let px = frame.get_pixel(rect.x + rect.width * gx / 24, rect.y + rect.height * gy / 4);
            // Coarse: text is white on dark, so this is mostly the glyph shape.
            (px.0[0] / 128).hash(&mut hasher);
        }
    }
    hasher.finish()
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
            item_side: crate::Side::Left,
            footer: Some(FooterInfo {
                sell_value: value,
                in_raid: false,
            }),
            others: Vec::new(),
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
