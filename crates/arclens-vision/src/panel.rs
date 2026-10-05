//! Finding the cream tooltip panels.
//!
//! The tooltip is an opaque cream rectangle on a dark UI: by far the largest
//! warm, bright, low-saturation region on screen. We threshold that colour on
//! a subsampled grid, take connected components, and keep the ones that are
//! big and solid enough to be a panel.

use crate::Rect;
use image::RgbImage;

/// Tunables for [`find_panels`]. Defaults were fitted on 1440p captures
/// (see `tests/fixtures/`).
#[derive(Debug, Clone, Copy)]
pub struct PanelParams {
    /// Sample every `step`-th pixel in both axes of a 1440-px-tall frame;
    /// scaled with the frame height (at least 1), so the grid keeps the
    /// same share of the UI at every resolution and the gap between a
    /// tooltip and the hovered slot's outline stays at least one cell.
    pub step: u32,
    /// Minimum panel size as a fraction of the frame's width / height.
    pub min_width_frac: f32,
    pub min_height_frac: f32,
    /// A shorter panel still counts if a tooltip footer (weight, value)
    /// sits right below it: ammo's tooltip body is only ~0.11 H, while
    /// cream boxes that aren't tooltips (a project page's selected row,
    /// ~0.09 H) have no footer.
    pub min_footed_height_frac: f32,
    /// Minimum share of the bounding box that must be cream. Text, icons and
    /// stat rows make up the rest.
    pub min_fill: f32,
}

impl Default for PanelParams {
    fn default() -> Self {
        Self {
            step: 4,
            // Tooltips are ~0.20 W wide and ≥ 0.22 H tall at 1440p (ammo's
            // ~0.11 H, see `min_footed_height_frac`); buttons and selected
            // item tiles are far smaller.
            min_width_frac: 0.12,
            min_height_frac: 0.12,
            min_footed_height_frac: 0.08,
            min_fill: 0.55,
        }
    }
}

/// Whether a pixel has the tooltip body's cream colour. Measured
/// (248,232,216) on the body; the header tab above it ("ACTIONS",
/// "REQUEST …") is a darker (200,184,168) and must *not* match.
pub(crate) fn is_cream([r, g, b]: [u8; 3]) -> bool {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    r >= 225 && g >= 210 && b >= 188 && r >= b && max - min <= 50
}

/// All cream panels in `frame`, largest first.
pub fn find_panels(frame: &RgbImage, params: &PanelParams) -> Vec<Rect> {
    let step = ((params.step as f32 * frame.height() as f32 / 1440.0).round() as u32).max(1);
    let (gw, gh) = (frame.width() / step, frame.height() / step);
    if gw == 0 || gh == 0 {
        return Vec::new();
    }
    let grid_index = |x: u32, y: u32| (y * gw + x) as usize;

    let mut mask = vec![false; (gw * gh) as usize];
    for gy in 0..gh {
        for gx in 0..gw {
            mask[grid_index(gx, gy)] = is_cream(frame.get_pixel(gx * step, gy * step).0);
        }
    }

    let min_w = (params.min_width_frac * gw as f32) as u32;
    let min_h = (params.min_height_frac * gh as f32) as u32;
    let min_footed_h = (params.min_footed_height_frac * gh as f32) as u32;
    // Component label per grid cell (0 = unlabelled).
    let mut labels = vec![0u32; mask.len()];
    let mut next_label = 0u32;
    let mut panels = Vec::new();
    let mut stack = Vec::new();

    for start in 0..mask.len() {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        next_label += 1;
        let label = next_label;
        // Flood fill one 4-connected component, tracking its bounding box.
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        labels[start] = label;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i as u32 % gw, i as u32 / gw);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            let neighbours = [
                (x > 0).then(|| i - 1),
                (x + 1 < gw).then(|| i + 1),
                (y > 0).then(|| i - gw as usize),
                (y + 1 < gh).then(|| i + gw as usize),
            ];
            for n in neighbours.into_iter().flatten() {
                if mask[n] && labels[n] == 0 {
                    labels[n] = label;
                    stack.push(n);
                }
            }
        }

        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        if w < min_w || h < min_footed_h {
            continue;
        }
        let component = Rect::new(x0, y0, w, h);
        // Only this component's cells count: other cream bits inside the
        // bounding box (icons in a neighbouring item grid) must not.
        let member = Member {
            labels: &labels,
            gw,
            label,
        };
        // Parts tall enough to be panels: specks of cream joined to a panel
        // (the project pages' header) also start a part, but don't make
        // it share its right edge with anything.
        let bodies: Vec<Rect> = split_by_left_edge(&member, component)
            .into_iter()
            .filter_map(|part| trim_to_body(&member, part))
            .filter(|body| body.height >= min_footed_h)
            .collect();
        let was_split = bodies.len() > 1;
        for mut body in bodies {
            // After a split, a part wider than any one panel is a tooltip
            // merged with the panel beside it: its right edge is the other
            // panel's (cream meets cream), unknowable from colour alone, so
            // use the tooltip's fixed width instead. A narrower part is one
            // panel (the purchase panel above a tooltip overlapping it).
            let tooltip_w = (TOOLTIP_WIDTH_PER_HEIGHT * gh as f32).round() as u32;
            let widest = (MAX_PANEL_WIDTH_PER_HEIGHT * gh as f32).round() as u32;
            if was_split && body.width > widest {
                body.width = tooltip_w;
            }
            let fill = cream_fraction(&member, body);
            if body.width < min_w || fill < params.min_fill {
                continue;
            }
            let rect = Rect::new(
                body.x * step,
                body.y * step,
                body.width * step,
                body.height * step,
            );
            if body.height >= min_h || crate::footer::footer(frame, rect).is_some() {
                panels.push(rect);
            }
        }
    }

    panels.sort_by_key(|r| std::cmp::Reverse(r.area()));
    panels
}

/// Tooltip width ÷ frame height. Every tooltip measured 504–516 px wide at
/// 1440p (≈ 0.353 H, independent of content).
const TOOLTIP_WIDTH_PER_HEIGHT: f32 = 0.353;
/// Widest single panel ÷ frame height: the trader's purchase panel, 0.427 H
/// (x 0.73–0.97 W at 16:9). A tooltip merged with it is wider still: it
/// sticks out left of it by at least a split's jump.
const MAX_PANEL_WIDTH_PER_HEIGHT: f32 = 0.45;

/// Membership test for one labelled component on the grid.
struct Member<'a> {
    labels: &'a [u32],
    gw: u32,
    label: u32,
}

impl Member<'_> {
    fn at(&self, x: u32, y: u32) -> bool {
        self.labels[(y * self.gw + x) as usize] == self.label
    }
}

/// Share of the component's cells in `r` (grid coordinates).
fn cream_fraction(m: &Member<'_>, r: Rect) -> f32 {
    let mut n = 0u64;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            n += u64::from(m.at(x, y));
        }
    }
    n as f32 / r.area().max(1) as f32
}

/// Splits a component where its left edge jumps. A hover tooltip that
/// touches another panel (trader screen) merges with it into one cream
/// component; it shows up as a run of rows whose left edge is far from the
/// rest. Returns row ranges, each with its own left edge.
fn split_by_left_edge(m: &Member<'_>, c: Rect) -> Vec<Rect> {
    /// A left-edge jump of more than this share of the grid width starts a
    /// new part (16 cells of a 2560 px frame at step 4).
    const JUMP: f32 = 0.025;
    let jump = ((JUMP * m.gw as f32).round() as u32).max(1);
    let mut parts: Vec<(u32, u32, u32)> = Vec::new(); // (y0, y1, left)
    for y in c.y..c.bottom() {
        let Some(left) = (c.x..c.right()).find(|&x| m.at(x, y)) else {
            continue;
        };
        match parts.last_mut() {
            Some((_, y1, l)) if left.abs_diff(*l) <= jump && y <= *y1 + 1 => {
                *y1 = y + 1;
                *l = (*l).min(left);
            }
            _ => parts.push((y, y + 1, left)),
        }
    }
    parts
        .into_iter()
        .map(|(y0, y1, left)| Rect::new(left, y0, c.right() - left, y1 - y0))
        .collect()
}

/// Drops leading/trailing rows that are mostly not cream and tightens the
/// right edge to the cream actually present.
fn trim_to_body(m: &Member<'_>, r: Rect) -> Option<Rect> {
    let solid = |y: u32| cream_fraction(m, Rect::new(r.x, y, r.width, 1)) >= 0.6;
    let top = (r.y..r.bottom()).find(|&y| solid(y))?;
    let bottom = (top..r.bottom()).rev().find(|&y| solid(y))? + 1;
    let right = (top..bottom)
        .filter_map(|y| (r.x..r.right()).rev().find(|&x| m.at(x, y)))
        .max()?
        + 1;
    Some(Rect::new(r.x, top, right - r.x, bottom - top))
}
