//! Quest screens: the logbook and the traders' quest pages list the
//! quests in progress. Reading their titles fills in the player's active
//! quests without typing.
//!
//! - A trader's QUESTS tab (`APOLLO — GRENADES & GADGETS  TRADES  [QUESTS]`)
//!   shows one selected quest in the centre column: the map name(s) in
//!   small grey caps, then the title in bold white caps.
//! - The pause menu's LOGBOOK tab lists the active quests on the left as
//!   expandable cards (chevron, trader icon, bold white title; then region
//!   names in grey and the objectives). The list scrolls: only the visible
//!   cards are read.
//!
//! Detection is cheap (no OCR): the outlined tab ("pill") in the top bar.
//! Its x depends on the trader's name length, so the pill is searched for
//! along the whole bar rather than at a fixed place.
//!
//! All positions are fractions of the frame, measured 2026-10-04 on the
//! 2560×1440 fixtures `quest_apollo_*`, `quest_lance_*` and `logbook`;
//! **unverified** at other resolutions and aspect ratios.

use crate::map_header::region;
use crate::{NameReader, Rect};
use image::RgbImage;

/// Which quest list the frame shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestScreen {
    /// A trader's QUESTS tab: one selected quest.
    TraderQuests,
    /// The pause menu's LOGBOOK tab: a card per active quest.
    Logbook,
}

/// Rows the pill's top border may be on (measured at y 0.012).
const PILL_TOP: [f32; 2] = [0.006, 0.022];
/// Rows the pill's bottom border may be on (measured at y 0.0465).
const PILL_BOTTOM: [f32; 2] = [0.036, 0.055];
/// Shortest straight border run of a pill (the MAP tab's is ≈ 0.034).
const PILL_MIN_RUN: f32 = 0.03;
/// Rows of the tab labels, between the pill's borders.
const TAB_TEXT: [f32; 2] = [0.02, 0.04];
/// Share of the label window that must be ink for "a tab label is there".
/// Measured: a grey label ≈ 15 %, the empty bar after the last tab < 1 %.
const MIN_LABEL_INK: f32 = 0.04;
/// Where to look for a neighbouring label: from this far beside the pill…
const LABEL_NEAR: f32 = 0.01;
/// …to this far (a tab label plus the gap measures ≈ 0.05).
const LABEL_FAR: f32 = 0.06;

/// The outlined LOGBOOK tab, centred in the bar between INVENTORY and
/// SYSTEM: measured centre 0.509, width 0.062 (MAP's pill, at the same
/// centre on the map screen, is 0.039 wide; INVENTORY's 0.070 at 0.43).
const LOGBOOK_CENTRE: [f32; 2] = [0.49, 0.53];
const LOGBOOK_WIDTH: [f32; 2] = [0.055, 0.068];
/// The outlined QUESTS tab on a trader page: measured width 0.050, centre
/// 0.347 (Apollo) and 0.268 (Lance). TRADES' pill is as wide, so the
/// label beside it decides: QUESTS is the last tab.
const QUESTS_WIDTH: [f32; 2] = [0.045, 0.056];
const QUESTS_MAX_CENTRE: f32 = 0.45;

/// The selected quest's title on a trader page: the first bright line in
/// this strip, `[x, y, w, h]`. Measured title rows 0.399–0.414 from x
/// 0.448; the grey map names above are not bright, the description below
/// starts at 0.44.
const TRADER_TITLE: [f32; 4] = [0.443, 0.385, 0.277, 0.049];
/// The logbook's quest list, where card titles are written: from right of
/// the trader icon (titles start at 0.141) to the card edge (0.356), and
/// from below the QUESTS heading to the bottom of the panel.
const LOGBOOK_TITLES: [f32; 4] = [0.139, 0.16, 0.213, 0.71];
/// Left of the titles: chevron + icon on a title row (neither has bright
/// pixels here), but the start of the text on an objective row (objective
/// text starts at 0.109).
const LOGBOOK_LEFT_OF_TITLE: [f32; 2] = [0.1055, 0.1387];
/// At most this share of the left window may be bright on a title row:
/// measured ≈ 1.7 % (icon highlights) on titles, ≥ 21 % on objectives.
const MAX_LEFT_INK: f32 = 0.06;
/// Text line height bounds (titles measure 0.0125–0.016).
const LINE_HEIGHT: [f32; 2] = [0.009, 0.025];

/// Bright (white or cream) text, as in the rest of the crate.
fn is_bright(frame: &RgbImage, col: u32, row: u32) -> bool {
    frame.get_pixel(col, row).0.iter().all(|&c| c >= 190)
}

/// Ink of a tab outline or label (labels are grey, ≈ 150–200).
fn is_ink(frame: &RgbImage, col: u32, row: u32) -> bool {
    frame.get_pixel(col, row).0.iter().all(|&c| c >= 150)
}

fn px(fraction: f32, size: u32) -> u32 {
    ((fraction * size as f32) as u32).min(size)
}

/// Horizontal runs of ink at least `min` long in `row`, as `[start, end)`.
fn ink_runs(frame: &RgbImage, row: u32, min: u32) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    let mut start = None;
    for col in 0..=frame.width() {
        let ink = col < frame.width() && is_ink(frame, col, row);
        match (ink, start) {
            (true, None) => start = Some(col),
            (false, Some(from)) => {
                if col - from >= min {
                    runs.push((from, col));
                }
                start = None;
            }
            _ => {}
        }
    }
    runs
}

/// Outlined tabs in the top bar, as `(left, right)` columns of their
/// straight top border: a long ink run near the top with a matching one
/// near the bottom of the bar.
fn pills(frame: &RgbImage) -> Vec<(u32, u32)> {
    let (w, h) = frame.dimensions();
    let min = px(PILL_MIN_RUN, w).max(4);
    let collect = |[from, to]: [f32; 2]| -> Vec<(u32, u32)> {
        (px(from, h)..px(to, h))
            .flat_map(|row| ink_runs(frame, row, min))
            .collect()
    };
    let (tops, bottoms) = (collect(PILL_TOP), collect(PILL_BOTTOM));
    let mut pills: Vec<(u32, u32)> = Vec::new();
    for &(left, right) in &tops {
        let matched = bottoms.iter().any(|&(l, r)| {
            let overlap = right.min(r).saturating_sub(left.max(l));
            overlap * 5 >= (right - left) * 4
        });
        if !matched {
            continue;
        }
        // Rows of the same border: keep the widest extent.
        match pills.iter_mut().find(|p| left < p.1 && p.0 < right) {
            Some(p) => *p = (p.0.min(left), p.1.max(right)),
            None => pills.push((left, right)),
        }
    }
    pills
}

/// Share of label ink in the tab-label rows between columns `from..to`.
fn label_ink(frame: &RgbImage, from: u32, to: u32) -> f32 {
    let h = frame.height();
    let to = to.min(frame.width());
    let rows = px(TAB_TEXT[0], h)..px(TAB_TEXT[1], h);
    let total = (to.saturating_sub(from) as usize) * rows.len();
    if total == 0 {
        return 0.0;
    }
    let ink = rows
        .flat_map(|row| (from..to).map(move |col| (col, row)))
        .filter(|&(col, row)| is_ink(frame, col, row))
        .count();
    ink as f32 / total as f32
}

/// Which quest screen `frame` shows, if any. Cheap (no OCR); run it every
/// frame.
pub fn quest_screen(frame: &RgbImage) -> Option<QuestScreen> {
    let w = frame.width();
    if w == 0 {
        return None;
    }
    let within = |value: f32, [lo, hi]: [f32; 2]| (lo..=hi).contains(&value);
    for (left, right) in pills(frame) {
        let width = (right - left) as f32 / w as f32;
        let centre = (left + right) as f32 / 2.0 / w as f32;
        if within(centre, LOGBOOK_CENTRE) && within(width, LOGBOOK_WIDTH) {
            return Some(QuestScreen::Logbook);
        }
        if within(width, QUESTS_WIDTH) && centre <= QUESTS_MAX_CENTRE {
            let (near, far) = (px(LABEL_NEAR, w), px(LABEL_FAR, w));
            let before = label_ink(frame, left.saturating_sub(far), left.saturating_sub(near));
            let after = label_ink(frame, right + near, right + far);
            if before >= MIN_LABEL_INK && after < MIN_LABEL_INK && trader_title(frame).is_some() {
                return Some(QuestScreen::TraderQuests);
            }
        }
    }
    None
}

/// Bright text lines in `area`: groups of rows with bright pixels (gaps of
/// up to 2 px inside a line), as tight boxes.
fn bright_lines(frame: &RgbImage, area: Rect) -> Vec<Rect> {
    let right = area.right().min(frame.width());
    let bottom = area.bottom().min(frame.height());
    let extent = |row: u32| {
        let first = (area.x..right).find(|&col| is_bright(frame, col, row))?;
        let last = (first..right)
            .rev()
            .find(|&col| is_bright(frame, col, row))
            .unwrap_or(first);
        Some((first, last))
    };
    let mut lines: Vec<Rect> = Vec::new();
    let mut last_row: Option<u32> = None;
    for row in area.y..bottom {
        let Some((from, to)) = extent(row) else {
            continue;
        };
        match lines.last_mut() {
            Some(line) if last_row.is_some_and(|r| row - r <= 3) => {
                let (l, r) = (line.x.min(from), line.right().max(to + 1));
                *line = Rect::new(l, line.y, r - l, row + 1 - line.y);
            }
            _ => lines.push(Rect::new(from, row, to + 1 - from, 1)),
        }
        last_row = Some(row);
    }
    let h = frame.height();
    let (min_h, max_h) = (px(LINE_HEIGHT[0], h).max(2), px(LINE_HEIGHT[1], h));
    lines.retain(|line| (min_h..=max_h).contains(&line.height) && line.width > line.height);
    lines
}

fn trader_title(frame: &RgbImage) -> Option<Rect> {
    bright_lines(frame, region(frame, TRADER_TITLE))
        .into_iter()
        .next()
}

fn logbook_titles(frame: &RgbImage) -> Vec<Rect> {
    let w = frame.width();
    let (from, to) = (
        px(LOGBOOK_LEFT_OF_TITLE[0], w),
        px(LOGBOOK_LEFT_OF_TITLE[1], w),
    );
    bright_lines(frame, region(frame, LOGBOOK_TITLES))
        .into_iter()
        .filter(|line| {
            let total = (to - from) * line.height;
            let ink = (line.y..line.bottom())
                .flat_map(|row| (from..to).map(move |col| (col, row)))
                .filter(|&(col, row)| is_bright(frame, col, row))
                .count();
            (ink as f32) <= MAX_LEFT_INK * total as f32
        })
        .collect()
}

/// Boxes of the quest titles to read on `screen`: the selected quest's
/// title on a trader page, each visible card's title in the logbook (top
/// to bottom). Objectives, region/map names and the logbook's tracked
/// resources are left out.
pub fn quest_title_boxes(frame: &RgbImage, screen: QuestScreen) -> Vec<Rect> {
    match screen {
        QuestScreen::TraderQuests => trader_title(frame).into_iter().collect(),
        QuestScreen::Logbook => logbook_titles(frame),
    }
}

/// Reads the quest titles the frame shows (empty if it shows no quest
/// screen), as written, e.g. `BATTENING DOWN`.
pub fn read_active_quests(reader: &NameReader, frame: &RgbImage) -> anyhow::Result<Vec<String>> {
    let Some(screen) = quest_screen(frame) else {
        return Ok(Vec::new());
    };
    let mut titles = Vec::new();
    for rect in quest_title_boxes(frame, screen) {
        if let Some(text) = reader.read_free_text(frame, rect)? {
            let text = text.trim();
            if !text.is_empty() {
                titles.push(text.to_owned());
            }
        }
    }
    Ok(titles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(frame: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, colour: [u8; 3]) {
        for row in y..y + h {
            for col in x..x + w {
                frame.put_pixel(col, row, image::Rgb(colour));
            }
        }
    }

    /// A dark top bar with a pill outline at `left..right` (2560×1440).
    fn bar_with_pill(left: u32, right: u32) -> RgbImage {
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([15, 15, 20]));
        paint(&mut frame, left, 17, right - left, 2, [240, 240, 240]);
        paint(&mut frame, left, 66, right - left, 2, [240, 240, 240]);
        frame
    }

    #[test]
    fn finds_pills_and_ignores_lone_lines() {
        let frame = bar_with_pill(1223, 1381);
        assert_eq!(pills(&frame), vec![(1223, 1381)]);
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([15, 15, 20]));
        paint(&mut frame, 500, 17, 200, 2, [240, 240, 240]);
        assert_eq!(pills(&frame), vec![]);
    }

    #[test]
    fn centred_pill_of_logbook_width_is_the_logbook() {
        assert_eq!(
            quest_screen(&bar_with_pill(1223, 1381)),
            Some(QuestScreen::Logbook)
        );
        // MAP's pill, narrower at the same place.
        assert_eq!(quest_screen(&bar_with_pill(1254, 1354)), None);
    }

    #[test]
    fn quests_pill_needs_a_tab_before_none_after_and_a_title() {
        let mut frame = bar_with_pill(824, 952);
        assert_eq!(quest_screen(&frame), None, "no TRADES label");
        paint(&mut frame, 680, 30, 100, 20, [180, 180, 180]);
        assert_eq!(quest_screen(&frame), None, "no title");
        paint(&mut frame, 1146, 574, 280, 22, [250, 240, 225]);
        assert_eq!(quest_screen(&frame), Some(QuestScreen::TraderQuests));
        paint(&mut frame, 990, 30, 100, 20, [180, 180, 180]);
        assert_eq!(quest_screen(&frame), None, "a tab after it");
    }
}
