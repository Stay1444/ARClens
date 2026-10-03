//! Place names on the in-game map ("Pattern House", "Generator Hall"):
//! near-white strokes with a dark outline, drawn over the greyscale map.
//!
//! Found by colour, like tooltips, so only the recognition model has to
//! run, on small crops. Bright terrain is told apart by the outline: a text
//! pixel has dark pixels close by on both sides.

use crate::Rect;
use image::RgbImage;

/// The map viewport as `[x, y, width, height]` fractions: right of the
/// screen edge, left of the legend panel, between the tab bar and the
/// footer hints. (The quest panel inside it is filtered out per label.)
const VIEWPORT: [f32; 4] = [0.02, 0.075, 0.76, 0.825];

/// Detection tuning, in fractions of the frame height (UI scales with it).
#[derive(Debug, Clone, Copy)]
pub struct LabelParams {
    /// Max distance from a text pixel to the outline on each side.
    pub outline_reach: f32,
    /// Gap between letters/words that still joins them into one label.
    pub join_gap: f32,
    /// Label text height range.
    pub min_height: f32,
    pub max_height: f32,
}

impl Default for LabelParams {
    fn default() -> Self {
        Self {
            outline_reach: 0.004,
            join_gap: 0.0075,
            min_height: 0.008,
            max_height: 0.028,
        }
    }
}

fn is_white([r, g, b]: [u8; 3]) -> bool {
    let (lo, hi) = (r.min(g).min(b), r.max(g).max(b));
    lo >= 215 && hi - lo <= 30
}

fn is_dark([r, g, b]: [u8; 3]) -> bool {
    r.max(g).max(b) <= 90
}

/// Boxes of the map labels in `frame`, top to bottom.
pub fn find_map_labels(frame: &RgbImage, params: &LabelParams) -> Vec<Rect> {
    let (w, h) = (frame.width(), frame.height());
    let hf = h as f32;
    let view = Rect::new(
        (VIEWPORT[0] * w as f32) as u32,
        (VIEWPORT[1] * hf) as u32,
        (VIEWPORT[2] * w as f32) as u32,
        (VIEWPORT[3] * hf) as u32,
    );
    let reach = ((params.outline_reach * hf).round() as u32).max(2);
    let px = |x: u32, y: u32| frame.get_pixel(x, y).0;

    // 1. Text pixels: white with a dark pixel within `reach` on both sides
    //    (horizontally or vertically).
    let (vw, vh) = (view.width as usize, view.height as usize);
    let mut mask = vec![false; vw * vh];
    for y in view.y + reach..view.bottom().min(h).saturating_sub(reach) {
        for x in view.x + reach..view.right().min(w).saturating_sub(reach) {
            if !is_white(px(x, y)) {
                continue;
            }
            let dark_h = (1..=reach).any(|d| is_dark(px(x - d, y)))
                && (1..=reach).any(|d| is_dark(px(x + d, y)));
            let dark_v = (1..=reach).any(|d| is_dark(px(x, y - d)))
                && (1..=reach).any(|d| is_dark(px(x, y + d)));
            if dark_h || dark_v {
                mask[(y - view.y) as usize * vw + (x - view.x) as usize] = true;
            }
        }
    }

    // 2. Join letters into labels: components of the mask after a
    //    horizontal dilation by `join_gap`.
    let gap = ((params.join_gap * hf).round() as usize).max(1);
    let boxes = components(&mask, vw, vh, gap);

    // 3. Keep text-shaped boxes.
    let (min_h, max_h) = (params.min_height * hf, params.max_height * hf);
    let mut labels: Vec<Rect> = boxes
        .into_iter()
        .map(|(x0, y0, x1, y1, count)| {
            (
                Rect::new(
                    view.x + x0 as u32,
                    view.y + y0 as u32,
                    (x1 - x0 + 1) as u32,
                    (y1 - y0 + 1) as u32,
                ),
                count,
            )
        })
        .filter(|(r, count)| {
            let height = r.height as f32;
            (min_h..=max_h).contains(&height)
                && r.width as f32 >= 2.5 * height
                // Strokes, not a filled blob.
                && (*count as f32) < 0.6 * r.area() as f32
                && (*count as f32) > 0.06 * r.area() as f32
        })
        .map(|(r, _)| r)
        .filter(|r| !on_ui_panel(frame, *r))
        .collect();
    labels.sort_by_key(|r| (r.y, r.x));
    merge_words(labels, 2.0 * params.join_gap * hf)
}

/// Joins boxes on the same line separated by a word gap ("Pipeline" +
/// "Tower"): mask runs sometimes miss the space between words.
fn merge_words(mut boxes: Vec<Rect>, max_gap: f32) -> Vec<Rect> {
    boxes.sort_by_key(|r| (r.x, r.y));
    let mut out: Vec<Rect> = Vec::new();
    'next: for b in boxes {
        for a in &mut out {
            let overlap = a.bottom().min(b.bottom()).saturating_sub(a.y.max(b.y));
            let same_line = overlap as f32 >= 0.5 * a.height.min(b.height) as f32;
            let gap = b.x.saturating_sub(a.right()) as f32;
            if same_line && b.x >= a.x && gap <= max_gap {
                let (x, y) = (a.x, a.y.min(b.y));
                let (right, bottom) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
                *a = Rect::new(x, y, right - x, bottom - y);
                continue 'next;
            }
        }
        out.push(b);
    }
    out.sort_by_key(|r| (r.y, r.x));
    out
}

/// Connected components of `mask` with horizontal gaps up to `gap` bridged.
/// Returns `(x0, y0, x1, y1, pixel_count)` per component.
fn components(
    mask: &[bool],
    w: usize,
    h: usize,
    gap: usize,
) -> Vec<(usize, usize, usize, usize, usize)> {
    // Runs per row: a run spans set pixels separated by at most `gap`.
    let mut runs: Vec<(usize, usize, usize, usize)> = Vec::new(); // (row, x0, x1, count)
    let mut row_start = Vec::with_capacity(h + 1);
    for y in 0..h {
        row_start.push(runs.len());
        let row = &mask[y * w..(y + 1) * w];
        let mut x = 0;
        while x < w {
            if !row[x] {
                x += 1;
                continue;
            }
            let (start, mut end, mut count) = (x, x, 0);
            let mut last_set = x;
            while x < w && x - last_set <= gap {
                if row[x] {
                    last_set = x;
                    end = x;
                    count += 1;
                }
                x += 1;
            }
            runs.push((y, start, end, count));
        }
    }
    row_start.push(runs.len());

    // Union-find over runs that overlap (within `gap`) on adjacent rows.
    let mut parent: Vec<usize> = (0..runs.len()).collect();
    for y in 1..h {
        for a in row_start[y - 1]..row_start[y] {
            for b in row_start[y]..row_start[y + 1] {
                let (_, ax0, ax1, _) = runs[a];
                let (_, bx0, bx1, _) = runs[b];
                if ax0 <= bx1 + gap && bx0 <= ax1 + gap {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    if ra != rb {
                        parent[ra] = rb;
                    }
                }
            }
        }
    }
    let mut out: std::collections::HashMap<usize, (usize, usize, usize, usize, usize)> =
        std::collections::HashMap::new();
    for (i, &(y, x0, x1, count)) in runs.iter().enumerate() {
        let root = find(&mut parent, i);
        let e = out.entry(root).or_insert((x0, y, x1, y, 0));
        e.0 = e.0.min(x0);
        e.1 = e.1.min(y);
        e.2 = e.2.max(x1);
        e.3 = e.3.max(y);
        e.4 += count;
    }
    out.into_values().collect()
}

/// Union-find root with path halving.
fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Whether the text sits on a flat dark UI panel (quest list, legend)
/// rather than on the textured map: the band just above and below it is
/// nearly uniform (text pixels aside).
fn on_ui_panel(frame: &RgbImage, r: Rect) -> bool {
    let band = (r.height / 2).max(2);
    let mut samples = Vec::new();
    for y in [r.y.saturating_sub(band + 2), r.bottom() + 2] {
        for dy in 0..band {
            let y = y + dy;
            if y >= frame.height() {
                continue;
            }
            for x in (r.x..r.right().min(frame.width())).step_by(3) {
                let [cr, cg, cb] = frame.get_pixel(x, y).0;
                let luma = (u32::from(cr) + u32::from(cg) + u32::from(cb)) as f32 / 3.0;
                // Skip neighbouring text lines; only the background counts.
                if luma < 100.0 {
                    samples.push(luma);
                }
            }
        }
    }
    if samples.is_empty() {
        return false;
    }
    let n = samples.len() as f32;
    let mean = samples.iter().sum::<f32>() / n;
    let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    mean < 60.0 && var.sqrt() < 6.0
}
