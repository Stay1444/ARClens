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

/// What OCR made of one tooltip: its name and footer.
type Reading = (String, Option<FooterInfo>);

/// Runs detection on every frame and OCR only when the name changed.
#[derive(Debug)]
pub struct Analyzer {
    reader: NameReader,
    params: PanelParams,
    /// Fingerprint of the last name + value crops and what they read as.
    last: Option<(u64, Option<Reading>)>,
}

impl Analyzer {
    pub fn new(reader: NameReader) -> Self {
        Self {
            reader,
            params: PanelParams::default(),
            last: None,
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
        let read = match &self.last {
            Some((last_key, read)) if *last_key == key => read.clone(),
            _ => {
                let read = self
                    .reader
                    .read(frame, &lines)?
                    .map(|name| -> anyhow::Result<_> {
                        Ok((name, self.read_footer(frame, cells.as_deref())?))
                    })
                    .transpose()?;
                self.last = Some((key, read.clone()));
                read
            }
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
