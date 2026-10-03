//! How the in-game map moved between two frames (pan and zoom), so the
//! overlay's markers can follow the map at capture rate. Reading the place
//! names (OCR) gives the exact view but only a few times a second; this
//! fills the gaps, and each label read corrects what drifted.
//!
//! Method: phase correlation of a downsampled, windowed grey crop of the
//! map, with a one-dimensional search over the zoom factor (the map only
//! pans and zooms, never rotates). Each candidate scale warps the previous
//! crop about its centre; the scale whose correlation peak is highest wins,
//! refined by a parabola through the best three.

use image::RgbImage;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// The part of the frame tracked, `[x, y, width, height]` as fractions:
/// map only (right of the quest panel, left of the legend, below the tab
/// bar, above the key hints).
pub const TRACK_REGION: [f32; 4] = [0.28, 0.12, 0.48, 0.76];
/// Side of the square thumbnail correlated (power of two for the FFT).
const N: usize = 256;
/// Samples per thumbnail pixel and axis when downsampling.
const SAMPLES: u32 = 4;
/// Zoom search: step between candidate scales, and how far to walk.
const SCALE_STEP: f32 = 0.04;
const MAX_SCALE_STEPS: usize = 8;
/// Probe either side of the start scale for a pan (1 %).
const FINE_STEP: f32 = 0.01;
/// Correlation peaks kept per scale, and their minimum spacing.
const PEAKS: usize = 3;
const PEAK_SPACING: usize = 4;
/// A peak this share of the best competes for the map's motion.
const RIVAL_PEAK: f32 = 0.3;
/// Block correlation that counts as a match, and the variance below
/// which a block is too flat to judge.
const BLOCK_MATCH: f32 = 0.7;
const FLAT_VARIANCE: f32 = 4.0;
/// Gauss–Newton iterations, and the thumbnail border left out.
const REFINE_ITERATIONS: usize = 8;
const REFINE_MARGIN: usize = 6;
/// Neighbourhood (thumbnail pixels) masked pixels are filled from.
const FILL_RADIUS: usize = 3;
/// A peak this high is a sure match (noise stays under ~0.05).
const CONFIDENT: f32 = 0.12;
/// Largest zoom between two frames searched: e^0.4 ≈ 1.5×.
const MAX_LOG_SCALE: f32 = 0.4;
/// Halvings of the step when refining: 0.04 → 0.0025 (±0.13 %).
const REFINE_STEPS: usize = 4;

/// A pan and zoom in frame pixels: `p' = scale · p + (dx, dy)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub scale: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Motion {
    pub const NONE: Self = Self {
        scale: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    pub fn apply(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (self.scale * x + self.dx, self.scale * y + self.dy)
    }

    /// This motion after `earlier`: `p ↦ self(earlier(p))`.
    #[must_use]
    pub fn after(&self, earlier: &Self) -> Self {
        Self {
            scale: self.scale * earlier.scale,
            dx: self.scale * earlier.dx + self.dx,
            dy: self.scale * earlier.dy + self.dy,
        }
    }

    /// Whether it moves no point of a `size` frame by more than `pixels`.
    pub fn is_still(&self, size: (f32, f32), pixels: f32) -> bool {
        [(0.0, 0.0), (size.0, 0.0), (0.0, size.1), (size.0, size.1)]
            .iter()
            .all(|&(x, y)| {
                let (mx, my) = self.apply((x, y));
                (mx - x).abs() <= pixels && (my - y).abs() <= pixels
            })
    }
}

/// A motion and how sure we are of it: the phase-correlation peak, about
/// 0.1–0.6 for a clean match and under ~0.05 for noise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub motion: Motion,
    pub confidence: f32,
}

/// What we drew on screen ourselves, in frame pixels: the overlay is in
/// the capture too. Its markers only follow the map a step behind, so left
/// in they look like a still map and drag every estimate towards "nothing
/// moved" (field report 2026-10-03: a fast zoom-out on Blue Gate froze the
/// markers in place). The tracker ignores these pixels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Footprint {
    /// Badges: centre and radius.
    pub circles: Vec<(f32, f32, f32)>,
    /// Shaded areas: their outlines.
    pub polygons: Vec<Vec<(f32, f32)>>,
}

impl Footprint {
    pub fn is_empty(&self) -> bool {
        self.circles.is_empty() && self.polygons.is_empty()
    }
}

/// Downsampled grey crop of one frame.
#[derive(Debug, Clone)]
struct Thumb {
    /// `N × N`, zero mean.
    pixels: Vec<f32>,
    /// Pixels covered by our own drawing (empty: none).
    mask: Vec<bool>,
    /// Frame size it came from.
    frame: (u32, u32),
    /// Frame pixels per thumbnail pixel, per axis.
    step: (f32, f32),
    /// Centre of the tracked region, in frame pixels.
    center: (f32, f32),
}

/// Follows the map from frame to frame.
pub struct MotionTracker {
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    /// Hann window, `N × N`.
    window: Vec<f32>,
    prev: Option<Thumb>,
    /// Scale of the last estimate: a zoom animation continues, so the
    /// search starts there.
    last_scale: f32,
}

impl std::fmt::Debug for MotionTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MotionTracker")
            .field("has_prev", &self.prev.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for MotionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MotionTracker {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        let hann: Vec<f32> = (0..N)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())
            .collect();
        let window = (0..N * N).map(|i| hann[i / N] * hann[i % N]).collect();
        Self {
            fft: planner.plan_fft_forward(N),
            ifft: planner.plan_fft_inverse(N),
            window,
            prev: None,
            last_scale: 1.0,
        }
    }

    /// Forgets the previous frame (map closed, or the view jumped).
    pub fn reset(&mut self) {
        self.prev = None;
        self.last_scale = 1.0;
    }

    /// How the map moved since the previous frame given; `None` for the
    /// first frame or after a size change.
    pub fn track(&mut self, frame: &RgbImage) -> Option<Estimate> {
        self.track_ignoring(frame, &Footprint::default())
    }

    /// [`Self::track`], ignoring what we drew ourselves (`drawn`, in this
    /// frame's pixels).
    pub fn track_ignoring(&mut self, frame: &RgbImage, drawn: &Footprint) -> Option<Estimate> {
        let mut thumb = thumbnail(frame);
        thumb.mask = rasterize(drawn, &thumb);
        let prev = self.prev.replace(thumb);
        let prev = prev?;
        let thumb = self.prev.as_ref()?;
        if prev.frame != thumb.frame {
            return None;
        }
        let estimate = self.estimate(&prev, thumb);
        self.last_scale = estimate.motion.scale;
        Some(estimate)
    }

    fn estimate(&self, prev: &Thumb, cur: &Thumb) -> Estimate {
        // Our drawing in either frame is left out of both: filled in from
        // around it, so no hard edge stays put between them.
        let mask = union(&prev.mask, &cur.mask);
        let (prev, cur) = if mask.is_empty() {
            (prev.clone(), cur.clone())
        } else {
            (
                Thumb {
                    pixels: fill_masked(&prev.pixels, &mask),
                    ..prev.clone()
                },
                Thumb {
                    pixels: fill_masked(&cur.pixels, &mask),
                    ..cur.clone()
                },
            )
        };
        let (prev, cur) = (&prev, &cur);
        let cur_spectrum = self.spectrum(&cur.pixels);
        // Correlation peaks per log-scale tried.
        let mut tried: Vec<(f32, Vec<Shift>)> = Vec::new();
        let mut eval = |log_scale: f32| -> f32 {
            if let Some((_, peaks)) = tried.iter().find(|(l, _)| (l - log_scale).abs() < 1e-4) {
                return peaks[0].peak;
            }
            let warped = warp(&prev.pixels, log_scale.exp());
            let peaks = self.correlate(&self.spectrum(&warped), &cur_spectrum);
            let peak = peaks[0].peak;
            tried.push((log_scale, peaks));
            peak
        };

        // Start where the zoom was heading (an animation continues), else
        // at no zoom.
        let start = if (self.last_scale - 1.0).abs() > 0.005 {
            let continued = self.last_scale.ln();
            if eval(continued) > eval(0.0) {
                continued
            } else {
                0.0
            }
        } else {
            0.0
        };
        let mut best = start;
        let mut best_peak = eval(start);
        // A pan: the start beats its close neighbours. Three correlations.
        let pan = best_peak >= CONFIDENT
            && eval(start + FINE_STEP) < best_peak
            && eval(start - FINE_STEP) < best_peak;
        if !pan {
            let mut consider = |candidate: f32, best: &mut f32, best_peak: &mut f32| {
                let peak = eval(candidate);
                if peak > *best_peak {
                    *best = candidate;
                    *best_peak = peak;
                }
                peak
            };
            // Walk towards the better side while the peak grows.
            let up = consider(start + SCALE_STEP, &mut best, &mut best_peak);
            let down = consider(start - SCALE_STEP, &mut best, &mut best_peak);
            let dir = if up >= down { SCALE_STEP } else { -SCALE_STEP };
            let mut last = best_peak;
            for k in 2..=MAX_SCALE_STEPS {
                let peak = consider(start + dir * k as f32, &mut best, &mut best_peak);
                if peak < last {
                    break;
                }
                last = peak;
            }
            // No clear match near the start: a big zoom step. Scan them all.
            if best_peak < CONFIDENT {
                let steps = (MAX_LOG_SCALE / SCALE_STEP).round() as i32;
                for k in -steps..=steps {
                    consider(k as f32 * SCALE_STEP, &mut best, &mut best_peak);
                }
            }
            // Refine by halving steps around the best.
            let mut h = SCALE_STEP / 2.0;
            for _ in 0..REFINE_STEPS {
                let centre = best;
                consider(centre - h, &mut best, &mut best_peak);
                consider(centre + h, &mut best, &mut best_peak);
                h /= 2.0;
            }
        }
        let (log_scale, peaks) = tried
            .into_iter()
            .find(|(l, _)| (l - best).abs() < 1e-4)
            .unwrap_or((0.0, vec![Shift::NONE]));
        let scale = log_scale.exp();
        let shift = self.choose_peak(&peaks, &warp(&prev.pixels, scale), &cur.pixels);
        // Phase correlation lands within a pixel or two and a percent of
        // zoom; direct alignment makes it exact.
        let (scale, (sx, sy)) = refine(
            &prev.pixels,
            &cur.pixels,
            &mask,
            scale,
            (shift.dx, shift.dy),
            !pan,
        );
        // Thumbnail: p' = c + s(p − c) + d about the centre; in frame
        // pixels that is p' = s·p + (1 − s)·C + step·d.
        let (cx, cy) = cur.center;
        Estimate {
            motion: Motion {
                scale,
                dx: (1.0 - scale) * cx + cur.step.0 * sx,
                dy: (1.0 - scale) * cy + cur.step.1 * sy,
            },
            confidence: shift.peak,
        }
    }

    /// The correlation peak most of the map agrees with. A second strong
    /// peak means two things moved differently: the map, and something on
    /// top of it (the game's place card follows the pointer). The map
    /// covers more of the view, so the shift most blocks agree with wins.
    #[allow(clippy::unused_self, reason = "kept with the other steps")]
    fn choose_peak(&self, peaks: &[Shift], prev: &[f32], cur: &[f32]) -> Shift {
        let top = peaks[0];
        let rivals: Vec<Shift> = peaks
            .iter()
            .copied()
            .filter(|p| p.peak >= RIVAL_PEAK * top.peak)
            .collect();
        if rivals.len() < 2 {
            return top;
        }
        rivals
            .into_iter()
            .map(|p| (support(prev, cur, p), p))
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.peak.total_cmp(&b.1.peak)))
            .map_or(top, |(_, p)| p)
    }

    /// Windowed 2-D spectrum of a zero-mean thumbnail.
    fn spectrum(&self, pixels: &[f32]) -> Vec<Complex32> {
        let mut data: Vec<Complex32> = pixels
            .iter()
            .zip(&self.window)
            .map(|(p, w)| Complex32::new(p * w, 0.0))
            .collect();
        fft2(&mut data, self.fft.as_ref());
        data
    }

    /// The strongest shifts `d` with `cur(x) ≈ prev(x − d)`, best first
    /// (at least one), from the two spectra.
    fn correlate(&self, prev: &[Complex32], cur: &[Complex32]) -> Vec<Shift> {
        let mut cross: Vec<Complex32> = prev
            .iter()
            .zip(cur)
            .map(|(a, b)| {
                let c = a.conj() * b;
                let norm = c.norm();
                if norm > 1e-9 {
                    c / norm
                } else {
                    Complex32::new(0.0, 0.0)
                }
            })
            .collect();
        fft2(&mut cross, self.ifft.as_ref());
        let norm = 1.0 / (N * N) as f32;
        let at = |x: usize, y: usize| cross[(y % N) * N + (x % N)].re * norm;
        let wrap = |v: usize| {
            if v > N / 2 {
                v as f32 - N as f32
            } else {
                v as f32
            }
        };
        let sub = |l: f32, c: f32, r: f32| {
            let curvature = l - 2.0 * c + r;
            if curvature < 0.0 {
                (0.5 * (l - r) / curvature).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        };
        // Local maxima, strongest first, at least PEAK_SPACING apart.
        let mut order: Vec<usize> = (0..N * N).collect();
        let k = PEAKS * 64;
        order.select_nth_unstable_by(k, |&a, &b| cross[b].re.total_cmp(&cross[a].re));
        order.truncate(k);
        order.sort_unstable_by(|&a, &b| cross[b].re.total_cmp(&cross[a].re));
        let mut peaks: Vec<(usize, usize)> = Vec::with_capacity(PEAKS);
        for i in order {
            let (x, y) = (i % N, i / N);
            let near = |&(px, py): &(usize, usize)| {
                let d = |a: usize, b: usize| {
                    let d = a.abs_diff(b);
                    d.min(N - d)
                };
                d(x, px) <= PEAK_SPACING && d(y, py) <= PEAK_SPACING
            };
            if !peaks.iter().any(near) {
                peaks.push((x, y));
                if peaks.len() == PEAKS {
                    break;
                }
            }
        }
        peaks
            .into_iter()
            .map(|(x, y)| {
                let centre = at(x, y);
                Shift {
                    dx: wrap(x) + sub(at(x + N - 1, y), centre, at(x + 1, y)),
                    dy: wrap(y) + sub(at(x, y + N - 1), centre, at(x, y + 1)),
                    peak: centre,
                }
            })
            .collect()
    }
}

/// How many blocks of `cur` match `prev` shifted by `shift`.
fn support(prev: &[f32], cur: &[f32], shift: Shift) -> usize {
    const BLOCK: usize = N / 8;
    let mut count = 0;
    for by in 0..N / BLOCK {
        for bx in 0..N / BLOCK {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab, mut n) =
                (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for y in by * BLOCK..(by + 1) * BLOCK {
                let py = y as f32 - shift.dy;
                if py < 0.0 || py > (N - 1) as f32 {
                    continue;
                }
                for x in bx * BLOCK..(bx + 1) * BLOCK {
                    let px = x as f32 - shift.dx;
                    if px < 0.0 || px > (N - 1) as f32 {
                        continue;
                    }
                    let a = bilinear(prev, px, py);
                    let b = cur[y * N + x];
                    sa += a;
                    sb += b;
                    saa += a * a;
                    sbb += b * b;
                    sab += a * b;
                    n += 1.0;
                }
            }
            if n < (BLOCK * BLOCK / 2) as f32 {
                continue;
            }
            let va = saa / n - (sa / n).powi(2);
            let vb = sbb / n - (sb / n).powi(2);
            // Flat blocks (fog, the dark margin) say nothing.
            if va < FLAT_VARIANCE || vb < FLAT_VARIANCE {
                continue;
            }
            let ncc = (sab / n - sa / n * sb / n) / (va * vb).sqrt();
            if ncc > BLOCK_MATCH {
                count += 1;
            }
        }
    }
    count
}

/// Gauss–Newton refinement of a zoom about the thumbnail centre and a
/// shift, minimising the robustly weighted difference between `prev`
/// moved that way and `cur`, plus a brightness offset. Labels and icons
/// keep their size while the map zooms; the robust weights ignore them.
/// `zoom: false` keeps the scale.
#[allow(clippy::many_single_char_names, reason = "maths notation")]
fn refine(
    prev: &[f32],
    cur: &[f32],
    mask: &[bool],
    scale: f32,
    shift: (f32, f32),
    zoom: bool,
) -> (f32, (f32, f32)) {
    let (gx, gy) = gradients(prev);
    let c = (N as f32 - 1.0) / 2.0;
    // Parameters: log scale, dx, dy, brightness offset.
    let mut theta = [scale.ln(), shift.0, shift.1, 0.0];
    let first = usize::from(!zoom);
    let mut residuals: Vec<f32> = Vec::with_capacity(N * N / 4);
    for _ in 0..REFINE_ITERATIONS {
        let s = theta[0].exp();
        // Residuals first, for the robust weights' scale.
        residuals.clear();
        let mut samples: Vec<(f32, [f32; 4])> = Vec::with_capacity(N * N / 4);
        for y in (REFINE_MARGIN..N - REFINE_MARGIN).step_by(2) {
            for x in (REFINE_MARGIN..N - REFINE_MARGIN).step_by(2) {
                let qx = c + (x as f32 - c - theta[1]) / s;
                let qy = c + (y as f32 - c - theta[2]) / s;
                if !(1.0..(N - 2) as f32).contains(&qx) || !(1.0..(N - 2) as f32).contains(&qy) {
                    continue;
                }
                let at = |px: f32, py: f32| (py.round() as usize) * N + px.round() as usize;
                if !mask.is_empty() && (mask[y * N + x] || mask[at(qx, qy)]) {
                    continue;
                }
                let r = bilinear(prev, qx, qy) + theta[3] - cur[y * N + x];
                let (ix, iy) = (bilinear(&gx, qx, qy), bilinear(&gy, qx, qy));
                let jacobian = [-(ix * (qx - c) + iy * (qy - c)), -ix / s, -iy / s, 1.0];
                residuals.push(r.abs());
                samples.push((r, jacobian));
            }
        }
        if samples.len() < 1000 {
            break;
        }
        let mid = residuals.len() / 2;
        let (_, median, _) = residuals.select_nth_unstable_by(mid, f32::total_cmp);
        // Huber: full weight within ~2σ (σ ≈ 1.48·MAD).
        let k = (3.0 * *median).max(0.5);
        let mut h = [[0.0f64; 4]; 4];
        let mut g = [0.0f64; 4];
        for (r, j) in &samples {
            let w = if r.abs() <= k { 1.0 } else { k / r.abs() };
            for a in first..4 {
                g[a] += f64::from(w * j[a] * r);
                for b in first..4 {
                    h[a][b] += f64::from(w * j[a] * j[b]);
                }
            }
        }
        let Some(step) = solve(&h, &g, first) else {
            break;
        };
        for a in first..4 {
            theta[a] -= step[a] as f32;
        }
        if step[0].abs() < 1e-5 && step[1].abs() < 0.01 && step[2].abs() < 0.01 {
            break;
        }
    }
    let refined = (theta[0].exp(), (theta[1], theta[2]));
    // Trust phase correlation if alignment wandered off (a flat view).
    if (refined.0 / scale - 1.0).abs() > 0.05
        || (refined.1.0 - shift.0).abs() > 4.0
        || (refined.1.1 - shift.1).abs() > 4.0
    {
        (scale, shift)
    } else {
        refined
    }
}

/// Central-difference gradients (zero on the border).
fn gradients(pixels: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut gx = vec![0.0; N * N];
    let mut gy = vec![0.0; N * N];
    for y in 1..N - 1 {
        for x in 1..N - 1 {
            let i = y * N + x;
            gx[i] = 0.5 * (pixels[i + 1] - pixels[i - 1]);
            gy[i] = 0.5 * (pixels[i + N] - pixels[i - N]);
        }
    }
    (gx, gy)
}

/// Solves `h · x = g` for the parameters from `first` on (Gaussian
/// elimination with partial pivoting); `None` when singular.
#[allow(
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "maths notation"
)]
fn solve(h: &[[f64; 4]; 4], g: &[f64; 4], first: usize) -> Option<[f64; 4]> {
    let n = 4 - first;
    let mut m = [[0.0f64; 5]; 4];
    for a in 0..n {
        for b in 0..n {
            m[a][b] = h[a + first][b + first];
        }
        m[a][n] = g[a + first];
    }
    for col in 0..n {
        let pivot = (col..n).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-9 {
            return None;
        }
        m.swap(col, pivot);
        for row in 0..n {
            if row != col {
                let f = m[row][col] / m[col][col];
                for k in col..=n {
                    m[row][k] -= f * m[col][k];
                }
            }
        }
    }
    let mut x = [0.0f64; 4];
    for a in 0..n {
        x[a + first] = m[a][n] / m[a][a];
    }
    Some(x)
}

fn bilinear(pixels: &[f32], x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(N - 1), (y0 + 1).min(N - 1));
    let (wx, wy) = (x.fract(), y.fract());
    let top = pixels[y0 * N + x0] * (1.0 - wx) + pixels[y0 * N + x1] * wx;
    let bottom = pixels[y1 * N + x0] * (1.0 - wx) + pixels[y1 * N + x1] * wx;
    top * (1.0 - wy) + bottom * wy
}

#[derive(Debug, Clone, Copy)]
struct Shift {
    dx: f32,
    dy: f32,
    peak: f32,
}

impl Shift {
    const NONE: Self = Self {
        dx: 0.0,
        dy: 0.0,
        peak: 0.0,
    };
}

/// In-place 2-D FFT of an `N × N` row-major buffer.
fn fft2(data: &mut [Complex32], fft: &dyn Fft<f32>) {
    // Rows (the buffer is N rows of N back to back).
    fft.process(data);
    transpose(data);
    fft.process(data);
    transpose(data);
}

fn transpose(data: &mut [Complex32]) {
    for y in 0..N {
        for x in y + 1..N {
            data.swap(y * N + x, x * N + y);
        }
    }
}

/// Grey `N × N` thumbnail of [`TRACK_REGION`], zero mean.
fn thumbnail(frame: &RgbImage) -> Thumb {
    let (width, height) = frame.dimensions();
    let [fx, fy, fw, fh] = TRACK_REGION;
    let (x0, y0) = (fx * width as f32, fy * height as f32);
    let step = (fw * width as f32 / N as f32, fh * height as f32 / N as f32);
    let raw = frame.as_raw();
    let mut pixels = vec![0.0; N * N];
    for ty in 0..N {
        for tx in 0..N {
            let mut sum = 0u32;
            for sy in 0..SAMPLES {
                let y = (y0 + (ty as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * step.1) as u32;
                let row = y.min(height - 1) as usize * width as usize;
                for sx in 0..SAMPLES {
                    let x = (x0 + (tx as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * step.0) as u32;
                    let i = (row + x.min(width - 1) as usize) * 3;
                    sum += 77 * u32::from(raw[i])
                        + 150 * u32::from(raw[i + 1])
                        + 29 * u32::from(raw[i + 2]);
                }
            }
            pixels[ty * N + tx] = sum as f32 / (256 * SAMPLES * SAMPLES) as f32;
        }
    }
    let mean = pixels.iter().sum::<f32>() / pixels.len() as f32;
    for p in &mut pixels {
        *p -= mean;
    }
    Thumb {
        pixels,
        mask: Vec::new(),
        frame: (width, height),
        step,
        center: (x0 + 0.5 * fw * width as f32, y0 + 0.5 * fh * height as f32),
    }
}

/// Thumbnail pixels covered by `drawn` (grown by a pixel for the blur of
/// downsampling); empty when nothing is.
fn rasterize(drawn: &Footprint, thumb: &Thumb) -> Vec<bool> {
    if drawn.is_empty() {
        return Vec::new();
    }
    let (ox, oy) = (
        thumb.center.0 - thumb.step.0 * N as f32 / 2.0,
        thumb.center.1 - thumb.step.1 * N as f32 / 2.0,
    );
    // Frame point → thumbnail pixel (fractional), and back.
    let to_thumb = |x: f32, y: f32| ((x - ox) / thumb.step.0, (y - oy) / thumb.step.1);
    let to_frame = |tx: usize, ty: usize| {
        (
            ox + (tx as f32 + 0.5) * thumb.step.0,
            oy + (ty as f32 + 0.5) * thumb.step.1,
        )
    };
    let span = |lo: f32, hi: f32| {
        let lo = (lo.floor() - 1.0).max(0.0) as usize;
        let hi = ((hi.ceil() + 1.0).max(0.0) as usize).min(N - 1);
        lo..=hi
    };
    let mut mask = vec![false; N * N];
    let grow = thumb.step.0.max(thumb.step.1);
    for &(cx, cy, r) in &drawn.circles {
        let r = r + grow;
        let (x0, y0) = to_thumb(cx - r, cy - r);
        let (x1, y1) = to_thumb(cx + r, cy + r);
        for ty in span(y0, y1) {
            for tx in span(x0, x1) {
                let (fx, fy) = to_frame(tx, ty);
                if (fx - cx).hypot(fy - cy) <= r {
                    mask[ty * N + tx] = true;
                }
            }
        }
    }
    for polygon in &drawn.polygons {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in polygon {
            let (tx, ty) = to_thumb(x, y);
            (x0, y0, x1, y1) = (x0.min(tx), y0.min(ty), x1.max(tx), y1.max(ty));
        }
        if x0 > x1 {
            continue;
        }
        for ty in span(y0, y1) {
            for tx in span(x0, x1) {
                let (fx, fy) = to_frame(tx, ty);
                if inside(polygon, fx, fy) {
                    mask[ty * N + tx] = true;
                }
            }
        }
    }
    mask
}

/// Even-odd point-in-polygon test.
fn inside(polygon: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut odd = false;
    let mut j = polygon.len().wrapping_sub(1);
    for (i, &(xi, yi)) in polygon.iter().enumerate() {
        let (xj, yj) = polygon[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

fn union(a: &[bool], b: &[bool]) -> Vec<bool> {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Vec::new(),
        (false, true) => a.to_vec(),
        (true, false) => b.to_vec(),
        (false, false) => a.iter().zip(b).map(|(x, y)| *x || *y).collect(),
    }
}

/// Masked pixels replaced by the average of the unmasked ones around them
/// (normalised box filter, widened until something is found), so the
/// holes carry no edges of their own.
fn fill_masked(pixels: &[f32], mask: &[bool]) -> Vec<f32> {
    let mut out = pixels.to_vec();
    for radius in [FILL_RADIUS, 2 * FILL_RADIUS, 4 * FILL_RADIUS] {
        let (sum, count) = box_sums(&out, mask, radius);
        let mut left = false;
        for i in 0..N * N {
            if mask[i] {
                if count[i] > 0.0 {
                    out[i] = sum[i] / count[i];
                } else {
                    left = true;
                }
            }
        }
        if !left {
            return out;
        }
    }
    // Nothing unmasked anywhere near: the mean.
    for (p, &m) in out.iter_mut().zip(mask) {
        if m && !p.is_finite() {
            *p = 0.0;
        }
    }
    out
}

/// Sums of unmasked values and their counts over `(2r+1)²` boxes.
fn box_sums(pixels: &[f32], mask: &[bool], radius: usize) -> (Vec<f32>, Vec<f32>) {
    let value = |i: usize| {
        if mask[i] {
            (0.0, 0.0)
        } else {
            (pixels[i], 1.0)
        }
    };
    // Rows, then columns.
    let mut row_sum = vec![(0.0f32, 0.0f32); N * N];
    for y in 0..N {
        for x in 0..N {
            let (lo, hi) = (x.saturating_sub(radius), (x + radius).min(N - 1));
            let mut acc = (0.0, 0.0);
            for k in lo..=hi {
                let (v, c) = value(y * N + k);
                acc = (acc.0 + v, acc.1 + c);
            }
            row_sum[y * N + x] = acc;
        }
    }
    let mut sum = vec![0.0; N * N];
    let mut count = vec![0.0; N * N];
    for y in 0..N {
        for x in 0..N {
            let (lo, hi) = (y.saturating_sub(radius), (y + radius).min(N - 1));
            let mut acc = (0.0, 0.0);
            for k in lo..=hi {
                let (v, c) = row_sum[k * N + x];
                acc = (acc.0 + v, acc.1 + c);
            }
            sum[y * N + x] = acc.0;
            count[y * N + x] = acc.1;
        }
    }
    (sum, count)
}

/// `pixels` zoomed by `scale` about the centre (bilinear; zero outside).
fn warp(pixels: &[f32], scale: f32) -> Vec<f32> {
    if (scale - 1.0).abs() < 1e-6 {
        return pixels.to_vec();
    }
    let c = (N as f32 - 1.0) / 2.0;
    let inv = 1.0 / scale;
    let mut out = vec![0.0; N * N];
    for y in 0..N {
        let sy = c + (y as f32 - c) * inv;
        if sy < 0.0 || sy > (N - 1) as f32 {
            continue;
        }
        let (y0, wy) = (sy.floor() as usize, sy.fract());
        let y1 = (y0 + 1).min(N - 1);
        for x in 0..N {
            let sx = c + (x as f32 - c) * inv;
            if sx < 0.0 || sx > (N - 1) as f32 {
                continue;
            }
            let (x0, wx) = (sx.floor() as usize, sx.fract());
            let x1 = (x0 + 1).min(N - 1);
            let top = pixels[y0 * N + x0] * (1.0 - wx) + pixels[y0 * N + x1] * wx;
            let bottom = pixels[y1 * N + x0] * (1.0 - wx) + pixels[y1 * N + x1] * wx;
            out[y * N + x] = top * (1.0 - wy) + bottom * wy;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frame` moved by `motion` (bilinear, frame pixels).
    fn moved(frame: &RgbImage, motion: Motion) -> RgbImage {
        let (w, h) = frame.dimensions();
        RgbImage::from_fn(w, h, |x, y| {
            let sx = (x as f32 - motion.dx) / motion.scale;
            let sy = (y as f32 - motion.dy) / motion.scale;
            let sx = sx.clamp(0.0, (w - 1) as f32) as u32;
            let sy = sy.clamp(0.0, (h - 1) as f32) as u32;
            *frame.get_pixel(sx, sy)
        })
    }

    fn fixture() -> RgbImage {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/map/dam_zoom_mid.jpg"
        );
        image::open(path).unwrap().to_rgb8()
    }

    /// Error of `estimate` against `truth` at a point of the region.
    fn error(estimate: Motion, truth: Motion, size: (f32, f32)) -> f32 {
        [(0.3, 0.2), (0.7, 0.8), (0.5, 0.5)]
            .iter()
            .map(|&(fx, fy)| {
                let p = (fx * size.0, fy * size.1);
                let (a, b) = (estimate.apply(p), truth.apply(p));
                (a.0 - b.0).hypot(a.1 - b.1)
            })
            .fold(0.0, f32::max)
    }

    /// Our own badges, drawn at the same screen spots in two frames while
    /// the map zooms out under them (the overlay lagging a step), must not
    /// pin the estimate to "no motion" once tracking is told about them.
    #[test]
    fn ignores_the_overlay_drawn_over_the_map() {
        // Zoomed out the map is dim and low in contrast, and a zoom step
        // between two frames is small: the case that failed in the field.
        let mut frame = fixture();
        for p in frame.pixels_mut() {
            for c in &mut p.0 {
                *c = (f32::from(*c) * 0.35) as u8;
            }
        }
        let size = (frame.width() as f32, frame.height() as f32);
        let truth = Motion {
            scale: 0.95,
            dx: 0.05 * 1280.0,
            dy: 0.05 * 720.0,
        };
        let mut badges = Footprint::default();
        for gy in 0..22 {
            for gx in 0..26 {
                let (x, y) = (740.0 + gx as f32 * 46.0, 190.0 + gy as f32 * 48.0);
                badges.circles.push((x, y, 13.0));
            }
        }
        let paint = |mut image: RgbImage| {
            for &(x, y, r) in &badges.circles {
                for py in (y - r) as u32..=(y + r) as u32 {
                    for px in (x - r) as u32..=(x + r) as u32 {
                        let d = (px as f32 - x).hypot(py as f32 - y);
                        if d <= r - 2.0 {
                            image.put_pixel(px, py, image::Rgb([245, 170, 40]));
                        } else if d <= r {
                            image.put_pixel(px, py, image::Rgb([10, 10, 10]));
                        }
                    }
                }
            }
            image
        };
        let (a, b) = (paint(frame.clone()), paint(moved(&frame, truth)));

        let mut blind = MotionTracker::new();
        blind.track(&a);
        let fooled = blind.track(&b).unwrap().motion;
        assert!(
            error(fooled, truth, size) > 20.0,
            "badges should mislead plain tracking ({fooled:?})"
        );

        let mut aware = MotionTracker::new();
        aware.track_ignoring(&a, &badges);
        let estimate = aware.track_ignoring(&b, &badges).unwrap().motion;
        let err = error(estimate, truth, size);
        assert!(err < 4.0, "got {estimate:?} (error {err:.1} px)");
    }

    #[test]
    fn composes_motions() {
        let a = Motion {
            scale: 2.0,
            dx: 1.0,
            dy: -1.0,
        };
        let b = Motion {
            scale: 0.5,
            dx: 3.0,
            dy: 0.0,
        };
        let p = (10.0, 20.0);
        assert_eq!(b.after(&a).apply(p), b.apply(a.apply(p)));
        assert!(Motion::NONE.is_still((100.0, 100.0), 0.1));
        assert!(!a.is_still((100.0, 100.0), 0.5));
    }

    #[test]
    fn follows_pans_and_zooms_of_a_real_map_frame() {
        let frame = fixture();
        let size = (frame.width() as f32, frame.height() as f32);
        let cases = [
            Motion::NONE,
            Motion {
                scale: 1.0,
                dx: 37.0,
                dy: -12.0,
            },
            Motion {
                scale: 1.0,
                dx: -240.0,
                dy: 155.0,
            },
            // Zoom about the screen centre, and about a corner.
            Motion {
                scale: 1.1,
                dx: -0.1 * 1280.0,
                dy: -0.1 * 720.0,
            },
            Motion {
                scale: 0.9,
                dx: 0.1 * 900.0 + 20.0,
                dy: 0.1 * 500.0,
            },
            Motion {
                scale: 1.25,
                dx: -0.25 * 1500.0,
                dy: -0.25 * 800.0,
            },
        ];
        for truth in cases {
            let mut tracker = MotionTracker::new();
            assert!(tracker.track(&frame).is_none());
            let estimate = tracker.track(&moved(&frame, truth)).unwrap();
            let err = error(estimate.motion, truth, size);
            assert!(
                err < 3.0,
                "{truth:?}: got {:?} (error {err:.1} px, peak {:.3})",
                estimate.motion,
                estimate.confidence
            );
        }
    }
}
