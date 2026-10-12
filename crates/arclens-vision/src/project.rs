//! A project's page (PROJECTS → a project, its PROJECT tab): the phase
//! circles top left, the selected one filled white; under the phase name,
//! one row per required item with a progress bar and "delivered/needed"
//! right-aligned under the bar's end. Rows of a locked phase (an earlier
//! one isn't finished) are dimmed: their bars are darker grey.
//!
//! Detection is cheap (no OCR): the outlined PROJECT tab in the top bar,
//! with the project's name before it and ABOUT after it, a selected phase
//! circle and at least one row bar. Reading the title and the counts uses
//! OCR on small boxes.
//!
//! All positions are fractions of the frame, measured 2026-10-12 on the
//! 2560×1440 fixtures `project_trophy_*` and
//! `project_ascending_the_mountain` (the last two of those scaled up from
//! 2000×1125 screenshots); **unverified** at other resolutions and aspect
//! ratios. Rows end before a column of per-item reward tiles when the
//! project has them (Trophy Display) and further right when it doesn't,
//! so each bar's end is found rather than assumed.

use crate::quests::{LABEL_FAR, LABEL_NEAR, MIN_LABEL_INK, bright_lines, label_ink, pills, px};
use crate::{NameReader, Rect};
use image::RgbImage;

/// The outlined PROJECT tab: width 0.046–0.048 of the frame measured
/// (QUESTS' pill is as wide but is the last tab: no label after it).
const TAB_WIDTH: [f32; 2] = [0.042, 0.054];
/// Its centre moves with the project name's length (0.216 for Trophy
/// Display, 0.267 for Ascending the Mountain).
const TAB_MAX_CENTRE: f32 = 0.45;
/// The project's name in the top bar: from right of the back button and
/// the separator (text starts at 0.0735) to the tab, in the bar's rows.
const TITLE: [f32; 3] = [0.068, 0.008, 0.035];
/// Gap kept between the title box and the tab.
const TITLE_GAP: f32 = 0.008;

/// Phase circles: the first centre, the distance between centres, and
/// their row (measured centres x 91 + 69k, y 175 at 2560×1440).
const CIRCLE_X: f32 = 91.0 / 2560.0;
const CIRCLE_PITCH: f32 = 69.0 / 2560.0;
const CIRCLE_Y: f32 = 175.0 / 1440.0;
/// The filled parts of a circle beside its digit (or lock), each side:
/// from this far from the centre…
const CIRCLE_SIDE_NEAR: f32 = 12.0 / 2560.0;
/// …to this far (the circle's radius is ≈ 25 px).
const CIRCLE_SIDE_FAR: f32 = 18.0 / 2560.0;
/// Half the height of those boxes.
const CIRCLE_SIDE_HALF_H: f32 = 3.0 / 1440.0;
/// A selected circle is filled white (≈ 237); others dark grey (≈ 54).
const MIN_SELECTED: f32 = 0.8;
/// More circles than any project has phases (they have 3–6).
const MAX_PHASES: u32 = 8;

/// The first row's bar top, the distance between rows, and the bar's
/// height (measured y 364–376, rows 149.3 px apart at 1440).
const BAR_TOP: f32 = 364.0 / 1440.0;
const ROW_PITCH: f32 = 149.33 / 1440.0;
const BAR_MID: f32 = 6.0 / 1440.0;
/// Where to compare a bar pixel with the row's background: this far above.
const ABOVE_BAR: f32 = 9.0 / 1440.0;
/// Bars start at 0.083; checked from a little inside.
const BAR_START: f32 = 0.094;
/// A bar ends at 0.293 (with reward tiles) or 0.352 (without).
const BAR_MIN_END: f32 = 0.25;
/// A bar pixel differs from the background above by this much (sum of
/// channels): measured ≥ 170 for the dimmest (locked) bar.
const BAR_CONTRAST: i32 = 90;
/// Rows a phase may have (they have 4).
const MAX_ROWS: u32 = 6;
/// Locked rows' bars are grey ≈ 71; open ones ≈ 153.
const MAX_LOCKED_GREY: u32 = 110;
/// Where the bar's grey is sampled, back from its end: `[from, to]`.
const BAR_TAIL: [f32; 2] = [0.012, 0.004];
/// The count under the bar's end: back from the end, and the rows below
/// the bar top (ink measured 35–49 px below it).
const COUNT_WIDTH: f32 = 0.035;
const COUNT_ROWS: [f32; 2] = [30.0 / 1440.0, 54.0 / 1440.0];

/// One requirement row as found on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRowBox {
    /// Where "delivered/needed" is written.
    pub count: Rect,
    /// Dimmed: the phase is locked.
    pub locked: bool,
}

/// Where things are on a project page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLayout {
    /// The project's name in the top bar.
    pub title: Rect,
    /// The selected phase, from 1.
    pub phase: u32,
    pub rows: Vec<ProjectRowBox>,
}

/// One requirement row as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRow {
    pub delivered: u32,
    pub needed: u32,
    pub locked: bool,
}

/// A project page as read: its title as written (`TROPHY DISPLAY`), the
/// selected phase and its rows, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPage {
    pub title: String,
    pub phase: u32,
    pub rows: Vec<ProjectRow>,
}

/// The PROJECT tab's left edge, if the top bar shows one.
fn project_tab(frame: &RgbImage) -> Option<u32> {
    let w = frame.width();
    let (near, far) = (px(LABEL_NEAR, w), px(LABEL_FAR, w));
    pills(frame).into_iter().find_map(|(left, right)| {
        let width = (right - left) as f32 / w as f32;
        let centre = (left + right) as f32 / 2.0 / w as f32;
        let fits = (TAB_WIDTH[0]..=TAB_WIDTH[1]).contains(&width) && centre <= TAB_MAX_CENTRE;
        let before = label_ink(frame, left.saturating_sub(far), left.saturating_sub(near));
        let after = label_ink(frame, right + near, right + far);
        (fits && before >= MIN_LABEL_INK && after >= MIN_LABEL_INK).then_some(left)
    })
}

/// Share of white pixels in `[x, y, w, h]` (fractions).
fn white_share(frame: &RgbImage, [x, y, w, h]: [f32; 4]) -> f32 {
    let (fw, fh) = frame.dimensions();
    let (x0, y0) = (px(x, fw), px(y, fh));
    let (x1, y1) = (px(x + w, fw).max(x0 + 1), px(y + h, fh).max(y0 + 1));
    let mut white = 0u32;
    let mut total = 0u32;
    for row in y0..y1.min(fh) {
        for col in x0..x1.min(fw) {
            total += 1;
            white += u32::from(frame.get_pixel(col, row).0.iter().all(|&c| c >= 190));
        }
    }
    if total == 0 {
        0.0
    } else {
        white as f32 / total as f32
    }
}

/// The selected phase circle (from 1): the only one filled white.
fn selected_phase(frame: &RgbImage) -> Option<u32> {
    let width = CIRCLE_SIDE_FAR - CIRCLE_SIDE_NEAR;
    let (y, h) = (CIRCLE_Y - CIRCLE_SIDE_HALF_H, 2.0 * CIRCLE_SIDE_HALF_H);
    let mut selected = (0..MAX_PHASES).filter(|&k| {
        let cx = CIRCLE_X + CIRCLE_PITCH * k as f32;
        let left = white_share(frame, [cx - CIRCLE_SIDE_FAR, y, width, h]);
        let right = white_share(frame, [cx + CIRCLE_SIDE_NEAR, y, width, h]);
        left >= MIN_SELECTED && right >= MIN_SELECTED
    });
    let first = selected.next()?;
    selected.next().is_none().then_some(first + 1)
}

fn sum(frame: &RgbImage, col: u32, row: u32) -> i32 {
    frame
        .get_pixel(col, row)
        .0
        .iter()
        .map(|&c| i32::from(c))
        .sum()
}

/// Row `k`'s bar: its end column, if the row is there.
fn bar_end(frame: &RgbImage, k: u32) -> Option<(u32, u32)> {
    let (w, h) = frame.dimensions();
    let top = BAR_TOP + ROW_PITCH * k as f32;
    let row = px(top + BAR_MID, h);
    let above = px(top + BAR_MID - ABOVE_BAR, h);
    if row >= h {
        return None;
    }
    let is_bar = |col: u32| (sum(frame, col, row) - sum(frame, col, above)).abs() > BAR_CONTRAST;
    let start = px(BAR_START, w);
    if !is_bar(start) {
        return None;
    }
    // Up to the first gap of 3 columns.
    let mut end = start;
    let mut col = start;
    while col + 1 < w && (col + 1..(col + 4).min(w)).any(is_bar) {
        col += 1;
        if is_bar(col) {
            end = col;
        }
    }
    (end >= px(BAR_MIN_END, w)).then_some((end, px(top, h)))
}

/// Mean grey of the bar just before its end.
fn bar_tail_grey(frame: &RgbImage, end: u32, top: u32) -> u32 {
    let (w, h) = frame.dimensions();
    let row = top + px(BAR_MID, h);
    let (from, to) = (
        end.saturating_sub(px(BAR_TAIL[0], w)),
        end.saturating_sub(px(BAR_TAIL[1], w)),
    );
    let cols = (from..to.max(from + 1)).collect::<Vec<_>>();
    let total: u32 = cols
        .iter()
        .map(|&col| {
            frame
                .get_pixel(col, row)
                .0
                .iter()
                .map(|&c| u32::from(c))
                .sum::<u32>()
                / 3
        })
        .sum();
    total / cols.len() as u32
}

/// Where the title, selected phase and rows are, if `frame` shows a
/// project page. Cheap (no OCR).
pub fn project_layout(frame: &RgbImage) -> Option<ProjectLayout> {
    let (w, h) = frame.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let tab = project_tab(frame)?;
    let phase = selected_phase(frame)?;
    let rows: Vec<ProjectRowBox> = (0..MAX_ROWS)
        .map_while(|k| bar_end(frame, k))
        .map(|(end, top)| {
            let count_w = px(COUNT_WIDTH, w);
            let count = Rect::new(
                end.saturating_sub(count_w),
                top + px(COUNT_ROWS[0], h),
                count_w + 4,
                px(COUNT_ROWS[1] - COUNT_ROWS[0], h),
            );
            ProjectRowBox {
                count,
                locked: bar_tail_grey(frame, end, top) < MAX_LOCKED_GREY,
            }
        })
        .collect();
    if rows.is_empty() {
        return None;
    }
    let left = px(TITLE[0], w);
    let right = tab.saturating_sub(px(TITLE_GAP, w));
    let area = Rect::new(
        left,
        px(TITLE[1], h),
        right.saturating_sub(left),
        px(TITLE[2], h),
    );
    let title = bright_lines(frame, area).into_iter().next()?;
    Some(ProjectLayout { title, phase, rows })
}

/// Whether `frame` shows a project page. Cheap (no OCR).
pub fn is_project_page(frame: &RgbImage) -> bool {
    project_layout(frame).is_some()
}

/// A glyph of a count, by its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountGlyph {
    /// A digit other than 1 (≈ 0.65 of the text height wide).
    Digit,
    /// The game font's 1: a narrow stem (≈ 0.33 wide). OCR often drops
    /// it or reads it as 7.
    One,
    /// The slash (≈ 0.5 wide, leaning right).
    Slash,
}

/// Narrower than this share of the text height: a 1.
const MAX_ONE_WIDTH: f32 = 0.43;
/// A slash's ink is this much (share of its width) further right in its
/// top quarter than in its bottom quarter.
const MIN_SLASH_LEAN: f32 = 0.35;
/// Ink differs from the background by this much grey.
const COUNT_INK: i32 = 70;

/// The glyphs of the count in `rect`, left to right: columns of ink
/// (dark on the cream selected row, light elsewhere) split by blank ones.
pub fn count_glyphs(frame: &RgbImage, rect: Rect) -> Vec<CountGlyph> {
    let grey = |col: u32, row: u32| -> i32 {
        frame
            .get_pixel(col, row)
            .0
            .iter()
            .map(|&c| i32::from(c))
            .sum::<i32>()
            / 3
    };
    let right = rect.right().min(frame.width());
    let bottom = rect.bottom().min(frame.height());
    if rect.x >= right || rect.y >= bottom {
        return Vec::new();
    }
    let background = grey(rect.x, rect.y);
    let ink = |col: u32, row: u32| (grey(col, row) - background).abs() > COUNT_INK;
    let rows: Vec<u32> = (rect.y..bottom)
        .filter(|&row| (rect.x..right).any(|col| ink(col, row)))
        .collect();
    let (Some(&top), Some(&last)) = (rows.first(), rows.last()) else {
        return Vec::new();
    };
    let height = (last + 1 - top) as f32;
    let mut spans: Vec<(u32, u32)> = Vec::new();
    for col in rect.x..right {
        if !(top..=last).any(|row| ink(col, row)) {
            continue;
        }
        match spans.last_mut() {
            Some(span) if span.1 == col => span.1 = col + 1,
            _ => spans.push((col, col + 1)),
        }
    }
    // Mean ink column of `rows` within a span, relative to its left edge.
    let mean_col = |(from, to): (u32, u32), rows: std::ops::RangeInclusive<u32>| {
        let cols: Vec<u32> = rows
            .flat_map(|row| (from..to).filter(move |&col| ink(col, row)))
            .collect();
        (!cols.is_empty())
            .then(|| cols.iter().map(|&c| (c - from) as f32).sum::<f32>() / cols.len() as f32)
    };
    let quarter = ((last - top) / 4).max(1);
    spans
        .into_iter()
        .filter(|(from, to)| to - from >= 2)
        .map(|span| {
            let width = (span.1 - span.0) as f32;
            let upper = mean_col(span, top..=top + quarter);
            let lower = mean_col(span, last - quarter..=last);
            let lean = upper.zip(lower).map_or(0.0, |(u, l)| (u - l) / width);
            if width < MAX_ONE_WIDTH * height {
                CountGlyph::One
            } else if lean >= MIN_SLASH_LEAN {
                CountGlyph::Slash
            } else {
                CountGlyph::Digit
            }
        })
        .collect()
}

/// "delivered/needed" from what OCR read (`text`) and the glyph shapes:
/// 1s and the slash come from the shapes, the other digits from OCR, in
/// order. OCR's 1s are dropped (the font's 1 is always narrow), and so
/// are 7s while it read more digits than there are (a 1 read as 7).
/// `None` when the two disagree.
pub fn parse_delivery(text: &str, glyphs: &[CountGlyph]) -> Option<(u32, u32)> {
    let wide = glyphs.iter().filter(|&&g| g == CountGlyph::Digit).count();
    let mut digits: Vec<char> = text
        .chars()
        .map(|c| match c {
            'O' | 'o' | 'D' | 'Q' => '0',
            'S' | 's' => '5',
            c => c,
        })
        .filter(|&c| c.is_ascii_digit() && c != '1')
        .collect();
    while digits.len() > wide {
        let seven = digits.iter().position(|&c| c == '7')?;
        digits.remove(seven);
    }
    if digits.len() != wide || glyphs.iter().filter(|&&g| g == CountGlyph::Slash).count() != 1 {
        return None;
    }
    let mut digits = digits.into_iter();
    let written: String = glyphs
        .iter()
        .map(|g| match g {
            CountGlyph::One => Some('1'),
            CountGlyph::Slash => Some('/'),
            CountGlyph::Digit => digits.next(),
        })
        .collect::<Option<String>>()?;
    let (delivered, needed) = written.split_once('/')?;
    let (delivered, needed): (u32, u32) = (delivered.parse().ok()?, needed.parse().ok()?);
    (needed > 0 && delivered <= needed).then_some((delivered, needed))
}

/// Reads the project page `frame` shows, if it shows one and every row's
/// count reads.
pub fn read_project_page(
    reader: &NameReader,
    frame: &RgbImage,
) -> anyhow::Result<Option<ProjectPage>> {
    let Some(layout) = project_layout(frame) else {
        return Ok(None);
    };
    let Some(title) = reader.read_free_text(frame, layout.title)? else {
        return Ok(None);
    };
    let mut rows = Vec::new();
    for row in &layout.rows {
        let text = reader.read_text(frame, row.count)?.unwrap_or_default();
        let Some((delivered, needed)) = parse_delivery(&text, &count_glyphs(frame, row.count))
        else {
            return Ok(None);
        };
        rows.push(ProjectRow {
            delivered,
            needed,
            locked: row.locked,
        });
    }
    Ok(Some(ProjectPage {
        title: title.trim().to_owned(),
        phase: layout.phase,
        rows,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use CountGlyph::{Digit as D, One as I, Slash as S};

    #[test]
    fn counts_take_ones_from_shapes() {
        // What OCR read on the 2026-10-12 fixtures, and the shapes.
        assert_eq!(parse_delivery("0/3 ", &[D, S, D]), Some((0, 3)));
        assert_eq!(parse_delivery("0/75 ", &[D, S, I, D]), Some((0, 15)));
        assert_eq!(parse_delivery("0/]5 ", &[D, S, I, D]), Some((0, 15)));
        assert_eq!(parse_delivery("0/0 ", &[D, S, I, D]), Some((0, 10)));
        assert_eq!(parse_delivery("0120", &[D, S, D, D]), Some((0, 20)));
        assert_eq!(parse_delivery("O/5", &[D, S, D]), Some((0, 5)));
        assert_eq!(parse_delivery("7/7", &[D, S, D]), Some((7, 7)));
        assert_eq!(parse_delivery("12/15", &[I, D, S, I, D]), Some((12, 15)));
        // Disagreements and impossible counts read as nothing.
        assert_eq!(parse_delivery("0/1", &[D, S, I, D]), None);
        assert_eq!(parse_delivery("0/5", &[D, D, I, D]), None);
        assert_eq!(parse_delivery("9/5", &[D, S, D]), None);
        assert_eq!(parse_delivery("0/0", &[D, S, D]), None);
    }
}
