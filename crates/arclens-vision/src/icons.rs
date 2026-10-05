//! Recognising an item from its stash-slot icon by comparing it with the
//! dataset's item images.
//!
//! Slot icons are the same 3D models as RaidTheory's `images/items/*.png`,
//! but rendered from a slightly different angle, smaller, on a navy
//! background with a rarity-coloured corner. So the comparison is loose:
//! both sides are cut to the object's silhouette, squared and shrunk to
//! [`SIZE`]², then scored on silhouette overlap and colour.
//! Measured on 42 labelled slots (2026-10-05, Python prototype): the right
//! item first 76 % of the time, its image group (tiers and blueprints
//! share one) 81 %, top three 90 %. See
//! `docs/vision/findings-2026-10-05-stash-icons.md`.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_possible_wrap,
    reason = "image kernels: x, y, w, h over images of at most 256 px a side"
)]

use crate::Rect;
use image::imageops::{FilterType, resize};
use image::{GrayImage, Luma, RgbImage, RgbaImage};

/// Side of the square the features are sampled on.
pub const SIZE: u32 = 32;

/// An icon's silhouette and colours, `SIZE`² each.
#[derive(Debug, Clone)]
pub struct IconFeatures {
    colour: Vec<[f32; 3]>,
    mask: Vec<f32>,
}

impl IconFeatures {
    fn mirrored(&self) -> Self {
        let n = SIZE as usize;
        let flip = |i: usize| (i / n) * n + (n - 1 - i % n);
        Self {
            colour: (0..n * n).map(|i| self.colour[flip(i)]).collect(),
            mask: (0..n * n).map(|i| self.mask[flip(i)]).collect(),
        }
    }

    /// Silhouette overlap (`IoU`) minus twice the mean colour difference
    /// where both are present. Higher is more alike; ~0.3–0.7 for a match.
    fn score(&self, other: &Self) -> f32 {
        let (mut inter, mut union, mut weighted, mut weight) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..self.mask.len() {
            let (a, b) = (self.mask[i], other.mask[i]);
            let w = a.min(b);
            inter += w;
            union += a.max(b);
            let diff: f32 = (0..3)
                .map(|c| (self.colour[i][c] - other.colour[i][c]).abs())
                .sum();
            weighted += diff * w;
            weight += w;
        }
        if union == 0.0 {
            return f32::MIN;
        }
        inter / union - 2.0 * weighted / (weight + 1e-6)
    }
}

/// Dataset icons by id, to rank against a slot.
#[derive(Debug, Clone)]
pub struct IconIndex<K> {
    entries: Vec<(K, IconFeatures)>,
}

impl<K> Default for IconIndex<K> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<K: Clone> IconIndex<K> {
    /// Adds a dataset image (RGBA, transparent background). Images with no
    /// opaque pixels are skipped.
    pub fn add(&mut self, key: K, icon: &RgbaImage) {
        if let Some(features) = icon_features(icon) {
            self.entries.push((key, features));
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The `k` best matches for `slot`, best first, with their scores. The
    /// slot is also tried mirrored: some renders face the other way.
    pub fn rank(&self, slot: &IconFeatures, k: usize) -> Vec<(K, f32)> {
        let mirrored = slot.mirrored();
        let mut scored: Vec<(usize, f32)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, (_, f))| (i, f.score(slot).max(f.score(&mirrored))))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored
            .into_iter()
            .take(k)
            .map(|(i, s)| (self.entries[i].0.clone(), s))
            .collect()
    }
}

/// A dataset image's features: its alpha channel is the silhouette.
pub fn icon_features(icon: &RgbaImage) -> Option<IconFeatures> {
    let (w, h) = icon.dimensions();
    let mut mask = GrayImage::new(w, h);
    let mut colour = RgbImage::new(w, h);
    for (x, y, p) in icon.enumerate_pixels() {
        let [r, g, b, a] = p.0;
        mask.put_pixel(x, y, Luma([if a > 20 { a } else { 0 }]));
        colour.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    square_features(&colour, &mask)
}

/// Working size of a slot's icon area before cutting out the object.
const WORK_W: u32 = 96;
const WORK_H: u32 = 78;
/// Median window for the background estimate (≈ a quarter of the icon
/// area, so thin objects like rifles don't become background).
const MEDIAN_RADIUS: i32 = 13;
const MEDIAN_STEP: usize = 3;

/// A stash slot's features: the object cut from its background.
pub fn slot_features(frame: &RgbImage, slot: Rect) -> Option<IconFeatures> {
    // The icon area: inside the outline, above the bottom bar.
    let x = slot.x + slot.width * 4 / 100;
    let y = slot.y + slot.height * 4 / 100;
    let w = slot.width * 92 / 100;
    let h = slot.height * 76 / 100;
    if x + w > frame.width() || y + h > frame.height() || w == 0 || h == 0 {
        return None;
    }
    let crop = image::imageops::crop_imm(frame, x, y, w, h).to_image();
    let area = resize(&crop, WORK_W, WORK_H, FilterType::Triangle);
    let mask = object_mask(&area)?;
    square_features(&area, &mask)
}

/// Pixels that belong to the object: unlike the smooth background (far
/// from its local median, or on a sharp edge), not rarity-coloured,
/// cleaned up and reduced to the largest blob.
fn object_mask(area: &RgbImage) -> Option<GrayImage> {
    let (w, h) = area.dimensions();
    let px = |x: u32, y: u32| area.get_pixel(x, y).0.map(|c| f32::from(c) / 255.0);
    let background = median_background(area);
    let gray: Vec<f32> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| px(x, y).iter().sum::<f32>() / 3.0)
        .collect();
    let blurred = blur3(&gray, w as usize, h as usize);
    let mut mask = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let [r, g, b] = px(x, y);
            let sat = r.max(g).max(b) - r.min(g).min(b);
            // The rarity corner: saturated blue/cyan, or magenta.
            let rarity = sat > 0.30 && (b > g + 0.15 || (r > g + 0.25 && b > g));
            let diff: f32 = (0..3).map(|c| (px(x, y)[c] - background[i][c]).abs()).sum();
            let edge = gradient(&blurred, w as usize, h as usize, x as usize, y as usize);
            mask[i] = (diff > 0.10 || edge > 0.035) && !rarity;
        }
    }
    let (w, h) = (w as usize, h as usize);
    for _ in 0..3 {
        mask = dilate(&mask, w, h);
    }
    for _ in 0..3 {
        mask = erode(&mask, w, h);
    }
    fill_holes(&mut mask, w, h);
    mask = dilate(&erode(&mask, w, h), w, h);
    let mask = largest_component(&mask, w, h)?;
    Some(GrayImage::from_fn(w as u32, h as u32, |x, y| {
        Luma([if mask[y as usize * w + x as usize] {
            255
        } else {
            0
        }])
    }))
}

/// Per-pixel median of a window around it, sampled every
/// [`MEDIAN_STEP`] pixels (exact enough for a smooth background).
fn median_background(area: &RgbImage) -> Vec<[f32; 3]> {
    let (w, h) = area.dimensions();
    let mut out = Vec::with_capacity((w * h) as usize);
    let mut samples: [Vec<u8>; 3] = Default::default();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for s in &mut samples {
                s.clear();
            }
            for dy in (-MEDIAN_RADIUS..=MEDIAN_RADIUS).step_by(MEDIAN_STEP) {
                for dx in (-MEDIAN_RADIUS..=MEDIAN_RADIUS).step_by(MEDIAN_STEP) {
                    let (sx, sy) = (x + dx, y + dy);
                    if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                        continue;
                    }
                    let p = area.get_pixel(sx as u32, sy as u32).0;
                    for c in 0..3 {
                        samples[c].push(p[c]);
                    }
                }
            }
            let mut m = [0.0; 3];
            for c in 0..3 {
                let mid = samples[c].len() / 2;
                let (_, v, _) = samples[c].select_nth_unstable(mid);
                m[c] = f32::from(*v) / 255.0;
            }
            out.push(m);
        }
    }
    out
}

fn blur3(v: &[f32], w: usize, h: usize) -> Vec<f32> {
    let at = |x: isize, y: isize| {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, h as isize - 1) as usize;
        v[y * w + x]
    };
    let k = [1.0, 2.0, 1.0];
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x as isize, y as isize)))
        .map(|(x, y)| {
            let mut s = 0.0;
            for (j, ky) in k.iter().enumerate() {
                for (i, kx) in k.iter().enumerate() {
                    s += kx * ky * at(x + i as isize - 1, y + j as isize - 1);
                }
            }
            s / 16.0
        })
        .collect()
}

/// Sobel gradient magnitude, in intensity per pixel.
fn gradient(v: &[f32], w: usize, h: usize, x: usize, y: usize) -> f32 {
    let at = |dx: isize, dy: isize| {
        let xx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
        let yy = (y as isize + dy).clamp(0, h as isize - 1) as usize;
        v[yy * w + xx]
    };
    let gx = (at(1, -1) + 2.0 * at(1, 0) + at(1, 1)) - (at(-1, -1) + 2.0 * at(-1, 0) + at(-1, 1));
    let gy = (at(-1, 1) + 2.0 * at(0, 1) + at(1, 1)) - (at(-1, -1) + 2.0 * at(0, -1) + at(1, -1));
    (gx * gx + gy * gy).sqrt() / 8.0
}

/// 4-neighbour dilation; outside the image counts as empty.
fn dilate(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            m[i] || (x > 0 && m[i - 1])
                || (x + 1 < w && m[i + 1])
                || (y > 0 && m[i - w])
                || (y + 1 < h && m[i + w])
        })
        .collect()
}

/// 4-neighbour erosion; outside the image counts as empty.
fn erode(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            m[i] && x > 0
                && m[i - 1]
                && x + 1 < w
                && m[i + 1]
                && y > 0
                && m[i - w]
                && y + 1 < h
                && m[i + w]
        })
        .collect()
}

/// Sets empty pixels not connected to the image border.
fn fill_holes(m: &mut [bool], w: usize, h: usize) {
    let mut outside = vec![false; w * h];
    let mut stack: Vec<usize> = (0..w * h)
        .filter(|&i| (i % w == 0 || i % w == w - 1 || i / w == 0 || i / w == h - 1) && !m[i])
        .collect();
    for &i in &stack {
        outside[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        let next = [
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
        ];
        for n in next.into_iter().flatten() {
            if !m[n] && !outside[n] {
                outside[n] = true;
                stack.push(n);
            }
        }
    }
    for (i, v) in m.iter_mut().enumerate() {
        *v = !outside[i];
    }
}

/// The biggest 4-connected blob, or `None` if there is none.
fn largest_component(m: &[bool], w: usize, h: usize) -> Option<Vec<bool>> {
    let mut label = vec![0u32; w * h];
    let mut sizes = vec![0usize];
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !m[start] || label[start] != 0 {
            continue;
        }
        let id = sizes.len() as u32;
        sizes.push(0);
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            sizes[id as usize] += 1;
            let (x, y) = (i % w, i / w);
            let next = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for n in next.into_iter().flatten() {
                if m[n] && label[n] == 0 {
                    label[n] = id;
                    stack.push(n);
                }
            }
        }
    }
    let (best, _) = sizes.iter().enumerate().skip(1).max_by_key(|&(_, s)| *s)?;
    Some(label.iter().map(|&l| l == best as u32).collect())
}

/// Crops colour and mask to the mask's bounding box, pads it to a centred
/// square and samples both at [`SIZE`]².
fn square_features(colour: &RgbImage, mask: &GrayImage) -> Option<IconFeatures> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in mask.enumerate_pixels() {
        if p.0[0] > 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return None;
    }
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let side = bw.max(bh);
    let (ox, oy) = ((side - bw) / 2, (side - bh) / 2);
    let mut sq_colour = RgbImage::new(side, side);
    let mut sq_mask = GrayImage::new(side, side);
    for y in 0..bh {
        for x in 0..bw {
            sq_colour.put_pixel(ox + x, oy + y, *colour.get_pixel(x0 + x, y0 + y));
            sq_mask.put_pixel(ox + x, oy + y, *mask.get_pixel(x0 + x, y0 + y));
        }
    }
    let c = resize(&sq_colour, SIZE, SIZE, FilterType::Lanczos3);
    let m = resize(&sq_mask, SIZE, SIZE, FilterType::Lanczos3);
    Some(IconFeatures {
        colour: c
            .pixels()
            .map(|p| p.0.map(|v| f32::from(v) / 255.0))
            .collect(),
        mask: m.pixels().map(|p| f32::from(p.0[0]) / 255.0).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid shape on a transparent background.
    fn icon(shape: impl Fn(u32, u32) -> bool, colour: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(64, 64, |x, y| {
            if shape(x, y) {
                image::Rgba([colour[0], colour[1], colour[2], 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        })
    }

    #[test]
    fn ranks_the_same_shape_and_colour_first() {
        let square = |x: u32, y: u32| (16..48).contains(&x) && (16..48).contains(&y);
        let bar = |x: u32, y: u32| (4..60).contains(&x) && (28..36).contains(&y);
        let mut index = IconIndex::default();
        index.add("red square", &icon(square, [200, 40, 40]));
        index.add("green square", &icon(square, [40, 200, 40]));
        index.add("red bar", &icon(bar, [200, 40, 40]));
        let probe = icon_features(&icon(square, [190, 50, 45])).unwrap();
        let ranked = index.rank(&probe, 3);
        assert_eq!(ranked[0].0, "red square");
        assert!(ranked[0].1 > ranked[1].1);
    }

    #[test]
    fn mirrored_icons_still_match() {
        let wedge = |x: u32, y: u32| x < 48 && (8..56).contains(&y) && x <= y;
        let mut index = IconIndex::default();
        index.add("wedge", &icon(wedge, [150, 150, 150]));
        index.add("square", &icon(|x, y| x < 40 && y < 40, [150, 150, 150]));
        let mirrored = icon(|x, y| wedge(63 - x, y), [150, 150, 150]);
        let ranked = index.rank(&icon_features(&mirrored).unwrap(), 2);
        assert_eq!(ranked[0].0, "wedge");
    }

    #[test]
    fn blank_icons_have_no_features() {
        assert!(icon_features(&RgbaImage::new(8, 8)).is_none());
    }

    #[test]
    fn holes_are_filled_and_specks_dropped() {
        let (w, h) = (7, 7);
        let mut m = vec![false; w * h];
        for y in 1..6 {
            for x in 1..6 {
                m[y * w + x] = !(x == 3 && y == 3);
            }
        }
        fill_holes(&mut m, w, h);
        assert!(m[3 * w + 3]);
        m[0] = true;
        let big = largest_component(&m, w, h).unwrap();
        assert!(!big[0] && big[3 * w + 3]);
    }
}
