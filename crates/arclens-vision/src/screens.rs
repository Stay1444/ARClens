//! Which menu screen the game shows, from fixed UI elements (no OCR).
//!
//! - The main menu ("PLAY" tab): its big yellow PLAY button bottom right,
//!   and the PLAY tab's outline top left. Regions measured on one 2560×1440
//!   screenshot (2026-10-03, field report); **unverified** elsewhere until
//!   fixture frames exist.
//! - The INVENTORY tab of the pause menu, out of raid (stash + loadout) and
//!   in raid (backpack): the outlined "INVENTORY" pill is the first tab of
//!   the top bar.
//! - A trader's TRADES tab: an outlined pill between two tab labels and
//!   the cream purchase panel on the right.
//!
//! [`classify`] combines these with the map, workshop and quest detectors.
//! Pill measurements: 2026-10-04, on the 2560×1440 fixtures in
//! `tests/fixtures/` (see `tests/screens.rs`); **unverified** at other
//! resolutions and aspect ratios.

use crate::map_header::{bright_share, region};
use crate::{QuestScreen, is_map_screen, is_workshop_overview, quest_screen};
use image::RgbImage;

/// Which screen a frame shows, as far as the cheap detectors can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    /// The main menu's PLAY tab.
    MainMenu,
    /// The map (in raid, or the pause menu's MAP tab).
    Map,
    /// The pause menu's INVENTORY tab: stash and loadout between raids,
    /// or the backpack in raid (same tab, different contents).
    Inventory,
    /// The Workshop overview (station tiles).
    Workshop,
    /// A trader's TRADES tab (item grid and purchase panel).
    Trader,
    /// A trader's QUESTS tab.
    TraderQuests,
    /// The pause menu's LOGBOOK tab.
    Logbook,
    /// Anything else (in raid without a menu, projects, crafting, …).
    Unknown,
}

/// Which screen `frame` shows. Cheap (no OCR); run it every frame.
pub fn classify(frame: &RgbImage) -> Screen {
    if is_main_menu(frame) {
        Screen::MainMenu
    } else if is_map_screen(frame) {
        Screen::Map
    } else if is_workshop_overview(frame) {
        Screen::Workshop
    } else if let Some(quests) = quest_screen(frame) {
        match quests {
            QuestScreen::TraderQuests => Screen::TraderQuests,
            QuestScreen::Logbook => Screen::Logbook,
        }
    } else if is_inventory(frame) {
        Screen::Inventory
    } else if is_trader(frame) {
        Screen::Trader
    } else {
        Screen::Unknown
    }
}

/// Inside the PLAY button, right of its dark chevron: `[x, y, w, h]`.
const PLAY_BUTTON: [f32; 4] = [0.82, 0.825, 0.14, 0.045];
/// Both ends of the outlined "PLAY" tab in the top bar.
const PLAY_TAB_LEFT: [f32; 4] = [0.0415, 0.012, 0.006, 0.034];
const PLAY_TAB_RIGHT: [f32; 4] = [0.0790, 0.012, 0.006, 0.034];
/// The button is mostly yellow; its black "PLAY" text covers the rest.
const MIN_YELLOW: f32 = 0.45;
/// Bright share of a tab end when outlined (as for the map's MAP tab).
const MIN_OUTLINE: f32 = 0.03;

/// Rows a pill's top border may be on: measured y 0.012 in the menus,
/// 0.022 in raid (the in-raid bar sits lower).
const PILL_TOP: [f32; 2] = [0.006, 0.03];
/// Rows a pill's bottom border may be on: measured 0.0465 in the menus,
/// 0.0576 in raid.
const PILL_BOTTOM: [f32; 2] = [0.036, 0.066];
/// Shortest straight border run of a pill. The in-raid MAP tab's is the
/// shortest measured: ≈ 0.026 (66 px at 2560, 2026-10-05).
const PILL_MIN_RUN: f32 = 0.02;
/// Share of a label window that must be ink for "a tab label is there"
/// (a grey label ≈ 14–19 %, the empty bar 0 %).
const MIN_LABEL_INK: f32 = 0.04;
/// Where to look for a neighbouring label: from this far beside the pill…
const LABEL_NEAR: f32 = 0.01;
/// …to this far (a tab label plus the gap measures ≈ 0.05).
const LABEL_FAR: f32 = 0.06;

/// The INVENTORY pill's straight border: measured width 0.069–0.071 at
/// centre 0.428 in the stash (INVENTORY · LOGBOOK · SYSTEM), 0.062 at
/// centre 0.353 in raid (INVENTORY · CRAFTING · MAP · LOGBOOK · SYSTEM,
/// a slightly smaller bar). LOGBOOK's pill measures 0.060 (with a label
/// before it), MAP's 0.039, TRADES'/QUESTS'/PROJECT's 0.047–0.050, and
/// WORKSHOP's 0.071, but at centre 0.135 with PLAY before it.
const INVENTORY_WIDTH: [f32; 2] = [0.058, 0.08];
const INVENTORY_CENTRE: [f32; 2] = [0.25, 0.5];

/// The MAP pill: straight border ≈ 0.026 wide at centre ≈ 0.509 in raid
/// (outline ends at x 0.486–0.534, see `map_header`), between CRAFTING and
/// LOGBOOK, and between two tabs in the pause menu too. A browser's
/// address bar there is a far wider pill with no tab labels beside it
/// (field report 2026-10-05).
const MAP_WIDTH: [f32; 2] = [0.02, 0.05];
const MAP_CENTRE: [f32; 2] = [0.49, 0.53];

/// The TRADES pill: measured width 0.050 at centre 0.236 (Tian Wen),
/// with GUN SHOP on its left and QUESTS on its right.
const TRADES_WIDTH: [f32; 2] = [0.045, 0.056];
const TRADES_MAX_CENTRE: f32 = 0.45;
/// Inside the trader's cream purchase panel, below the item name and
/// right of where hover tooltips reach (panel measured x 0.73–0.97,
/// y 0.1–0.92; tooltips reach y ≥ 0.5 when they overlap it).
const TRADE_PANEL: [f32; 4] = [0.80, 0.2, 0.15, 0.25];
/// Share of cream the panel region needs (1.0 measured on the trader
/// fixtures, ≤ 0.19 on all others except the quest pages' cream panel,
/// whose pill has no tab after it).
const MIN_CREAM: f32 = 0.8;

/// Whether `frame` shows the main menu. Cheap; run it every frame.
///
/// A trader's TRADES tab also has a yellow button there (BUY) and its back
/// button's chevron sits where the PLAY tab's left end is, bright enough
/// to pass for an outline in a downscaled frame (1280×720), so the
/// trader's cream purchase panel rules the main menu out.
pub fn is_main_menu(frame: &RgbImage) -> bool {
    yellow_share(frame, PLAY_BUTTON) >= MIN_YELLOW
        && cream_share(frame, TRADE_PANEL) < MIN_CREAM
        && bright_share(frame, PLAY_TAB_LEFT) >= MIN_OUTLINE
        && bright_share(frame, PLAY_TAB_RIGHT) >= MIN_OUTLINE
}

/// Whether `frame` shows the pause menu's INVENTORY tab (stash and
/// loadout, or the backpack in raid): an outlined pill of INVENTORY's
/// width with no tab label before it (it is the first tab) and one after
/// it. Cheap (no OCR); run it every frame.
pub fn is_inventory(frame: &RgbImage) -> bool {
    pills(frame).into_iter().any(|pill| {
        pill.fits(frame, INVENTORY_WIDTH, INVENTORY_CENTRE)
            && pill.labels_beside(frame) == (false, true)
    })
}

/// Whether the top bar has the MAP tab's outlined pill, with a tab label on
/// each side.
pub(crate) fn has_map_pill(frame: &RgbImage) -> bool {
    pills(frame).into_iter().any(|pill| {
        pill.fits(frame, MAP_WIDTH, MAP_CENTRE) && pill.labels_beside(frame) == (true, true)
    })
}

/// Whether `frame` shows a trader's TRADES tab: a pill between two tab
/// labels, and the cream purchase panel.
fn is_trader(frame: &RgbImage) -> bool {
    cream_share(frame, TRADE_PANEL) >= MIN_CREAM
        && pills(frame).into_iter().any(|pill| {
            pill.fits(frame, TRADES_WIDTH, [0.0, TRADES_MAX_CENTRE])
                && pill.labels_beside(frame) == (true, true)
        })
}

/// Share of saturated yellow (the game's accent) in the region.
fn yellow_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    share(frame, fraction, |[r, g, b]| {
        r >= 200 && g >= 150 && b <= 100 && r - b >= 120
    })
}

/// Share of the tooltips' and panels' cream in the region.
fn cream_share(frame: &RgbImage, fraction: [f32; 4]) -> f32 {
    share(frame, fraction, |[r, g, b]| {
        r >= 220 && g >= 210 && b >= 180 && r >= b && r - b <= 50
    })
}

fn share(frame: &RgbImage, fraction: [f32; 4], hit: impl Fn([u8; 3]) -> bool) -> f32 {
    let area = region(frame, fraction);
    let (mut hits, mut total) = (0u32, 0u32);
    for row in area.y..area.bottom().min(frame.height()) {
        for col in area.x..area.right().min(frame.width()) {
            total += 1;
            hits += u32::from(hit(frame.get_pixel(col, row).0));
        }
    }
    if total == 0 {
        0.0
    } else {
        hits as f32 / total as f32
    }
}

/// Ink of a tab outline or label (labels are grey, ≈ 150–200).
fn is_ink(frame: &RgbImage, col: u32, row: u32) -> bool {
    frame.get_pixel(col, row).0.iter().all(|&c| c >= 150)
}

fn px(fraction: f32, size: u32) -> u32 {
    ((fraction * size as f32) as u32).min(size)
}

/// An outlined tab in the top bar: the columns `[left, right)` of its
/// straight borders and the rows of its top and bottom border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pill {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

impl Pill {
    /// Whether the pill's width and centre (fractions of the frame width)
    /// lie in the given ranges.
    fn fits(self, frame: &RgbImage, width: [f32; 2], centre: [f32; 2]) -> bool {
        let w = frame.width() as f32;
        let within = |value: f32, [lo, hi]: [f32; 2]| (lo..=hi).contains(&value);
        within((self.right - self.left) as f32 / w, width)
            && within((self.left + self.right) as f32 / 2.0 / w, centre)
    }

    /// Whether a tab label sits just before and just after the pill.
    fn labels_beside(self, frame: &RgbImage) -> (bool, bool) {
        let w = frame.width();
        let (near, far) = (px(LABEL_NEAR, w), px(LABEL_FAR, w));
        let label = |from: u32, to: u32| self.label_ink(frame, from, to) >= MIN_LABEL_INK;
        (
            label(
                self.left.saturating_sub(far),
                self.left.saturating_sub(near),
            ),
            label(self.right + near, self.right + far),
        )
    }

    /// Share of label ink between columns `from..to`, in the middle half
    /// of the pill's rows (where the tab labels are written).
    fn label_ink(self, frame: &RgbImage, from: u32, to: u32) -> f32 {
        let to = to.min(frame.width());
        let quarter = (self.bottom - self.top) / 4;
        let rows = (self.top + quarter)..(self.bottom - quarter).min(frame.height());
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

/// Outlined tabs in the top bar: a long ink run near the top of the bar
/// with a matching one (overlapping ≥ 80 %) near its bottom. Same method
/// as the quest screens', with the border rows kept for the labels.
fn pills(frame: &RgbImage) -> Vec<Pill> {
    let (w, h) = frame.dimensions();
    let min = px(PILL_MIN_RUN, w).max(4);
    let collect = |[from, to]: [f32; 2]| -> Vec<(u32, (u32, u32))> {
        (px(from, h)..px(to, h))
            .flat_map(|row| {
                ink_runs(frame, row, min)
                    .into_iter()
                    .map(move |run| (row, run))
            })
            .collect()
    };
    let (tops, bottoms) = (collect(PILL_TOP), collect(PILL_BOTTOM));
    let mut pills: Vec<Pill> = Vec::new();
    for &(top, (left, right)) in &tops {
        let Some(bottom) = bottoms
            .iter()
            .filter(|&&(row, (l, r))| {
                let overlap = right.min(r).saturating_sub(left.max(l));
                row > top && overlap * 5 >= (right - left) * 4
            })
            .map(|&(row, _)| row)
            .max()
        else {
            continue;
        };
        // Rows of the same border: keep the widest extent.
        match pills.iter_mut().find(|p| left < p.right && p.left < right) {
            Some(p) => {
                *p = Pill {
                    left: p.left.min(left),
                    right: p.right.max(right),
                    top: p.top.min(top),
                    bottom: p.bottom.max(bottom),
                };
            }
            None => pills.push(Pill {
                left,
                right,
                top,
                bottom,
            }),
        }
    }
    pills
}

/// Names of each map as the map panel's title writes them (with the
/// clock dropped), then short forms, by MetaForge map id. Same ids as
/// `arclens_data::metaforge::MAPS`, which the app uses for the same job
/// (`map_for_title`); this copy keeps the vision crate free of the data
/// crate.
const MAP_TITLES: &[(&str, &[&str])] = &[
    ("dam", &["DAM BATTLEGROUNDS", "DAM"]),
    ("spaceport", &["SPACEPORT"]),
    ("buried-city", &["BURIED CITY"]),
    ("blue-gate", &["BLUE GATE"]),
    ("stella-montis", &["STELLA MONTIS"]),
    ("riven-tides", &["RIVEN TIDES"]),
];
/// Least similarity (1 − edits / length) to accept a name.
const MIN_TITLE_SIMILARITY: f32 = 0.8;

/// The map (MetaForge id: `"dam"`, `"blue-gate"`, …) whose name starts the
/// map panel's title as OCR read it, e.g. `"DAM BATTLEGROUNDS - 18:55"` →
/// `"dam"`. Drops the raid clock, a leading "THE", case and punctuation,
/// and tolerates OCR slips (`0`/`O`, `1`/`I`, a wrong letter or two).
/// `None` for anything that isn't close to a map name.
pub fn map_from_title(title: &str) -> Option<&'static str> {
    let words: Vec<String> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !word.chars().all(|c| c.is_ascii_digit()))
        .map(|word| {
            word.chars()
                .map(|c| match c.to_ascii_uppercase() {
                    '0' => 'O',
                    '1' => 'I',
                    other => other,
                })
                .collect()
        })
        .collect();
    let words = match words.first() {
        Some(first) if first == "THE" => &words[1..],
        _ => &words[..],
    };
    let name = &words.join(" ");
    if name.is_empty() {
        return None;
    }
    MAP_TITLES
        .iter()
        .flat_map(|&(id, names)| names.iter().map(move |known| (id, similarity(name, known))))
        .filter(|&(_, score)| score >= MIN_TITLE_SIMILARITY)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// `1 − levenshtein(a, b) / max(len)`, on chars.
fn similarity(a: &str, b: &str) -> f32 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, &cb) in b.iter().enumerate() {
            current[j + 1] = (previous[j] + usize::from(ca != cb))
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        previous = current;
    }
    1.0 - previous[b.len()] as f32 / longest as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(frame: &mut RgbImage, fraction: [f32; 4], colour: [u8; 3]) {
        let area = region(frame, fraction);
        for row in area.y..area.bottom() {
            for col in area.x..area.right() {
                frame.put_pixel(col, row, image::Rgb(colour));
            }
        }
    }

    #[test]
    fn needs_the_play_button_and_the_outlined_tab() {
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([30, 25, 20]));
        assert!(!is_main_menu(&frame));
        paint(&mut frame, PLAY_BUTTON, [245, 197, 24]);
        assert!(!is_main_menu(&frame), "button alone");
        // A 2 px outline at each end of the tab.
        for end in [PLAY_TAB_LEFT, PLAY_TAB_RIGHT] {
            paint(
                &mut frame,
                [end[0], end[1], 0.0008, end[3]],
                [240, 240, 240],
            );
        }
        assert!(is_main_menu(&frame));
        assert_eq!(classify(&frame), Screen::MainMenu);
    }

    #[test]
    fn other_screens_are_not_the_main_menu() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        for sub in ["frames", "map"] {
            for entry in std::fs::read_dir(format!("{dir}/{sub}")).unwrap() {
                let path = entry.unwrap().path();
                let frame = image::open(&path).unwrap().to_rgb8();
                assert!(!is_main_menu(&frame), "{}", path.display());
            }
        }
    }

    /// A dark top bar with a pill outline over `[left, right)` (fractions).
    fn bar_with_pill(left: f32, right: f32) -> RgbImage {
        let mut frame = RgbImage::from_pixel(2560, 1440, image::Rgb([15, 15, 20]));
        paint(
            &mut frame,
            [left, 0.012, right - left, 0.0014],
            [240, 240, 240],
        );
        paint(
            &mut frame,
            [left, 0.046, right - left, 0.0014],
            [240, 240, 240],
        );
        frame
    }

    fn label(frame: &mut RgbImage, x: f32) {
        paint(frame, [x, 0.022, 0.04, 0.014], [180, 180, 180]);
    }

    #[test]
    fn inventory_is_the_first_tab_and_outlined() {
        let mut frame = bar_with_pill(0.393, 0.463);
        assert!(!is_inventory(&frame), "no tab after it");
        label(&mut frame, 0.475);
        assert!(is_inventory(&frame));
        assert_eq!(classify(&frame), Screen::Inventory);
        label(&mut frame, 0.345);
        assert!(!is_inventory(&frame), "a tab before it");
        // A narrower pill (LOGBOOK's) is not INVENTORY.
        let mut frame = bar_with_pill(0.400, 0.452);
        label(&mut frame, 0.47);
        assert!(!is_inventory(&frame));
    }

    #[test]
    fn trades_needs_tabs_on_both_sides_and_the_panel() {
        let mut frame = bar_with_pill(0.30, 0.35);
        label(&mut frame, 0.25);
        label(&mut frame, 0.36);
        assert!(!is_trader(&frame), "no purchase panel");
        paint(&mut frame, TRADE_PANEL, [245, 236, 222]);
        assert!(is_trader(&frame));
        assert_eq!(classify(&frame), Screen::Trader);
    }

    #[test]
    fn blank_frames_are_unknown() {
        let frame = RgbImage::from_pixel(1280, 720, image::Rgb([20, 20, 20]));
        assert_eq!(classify(&frame), Screen::Unknown);
        assert_eq!(classify(&RgbImage::new(0, 0)), Screen::Unknown);
    }

    #[test]
    fn similarity_counts_edits() {
        assert!((similarity("DAM", "DAM") - 1.0).abs() < f32::EPSILON);
        assert!((similarity("BLUE GATE", "BLUE GATF") - 8.0 / 9.0).abs() < 1e-6);
        assert!((similarity("", "") - 1.0).abs() < f32::EPSILON);
        assert!(similarity("ABC", "").abs() < f32::EPSILON);
    }
}
