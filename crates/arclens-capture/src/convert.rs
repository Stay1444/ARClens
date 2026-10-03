//! Captured pixels to RGB.

use image::RgbImage;

/// Byte order of a 4-byte pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// B, G, R, then alpha or padding.
    Bgr,
    /// R, G, B, then alpha or padding.
    #[cfg_attr(
        not(target_os = "linux"),
        allow(dead_code, reason = "PipeWire may send it")
    )]
    Rgb,
}

/// Converts one 4-byte-per-pixel frame with rows `stride` bytes apart.
/// `None` for inconsistent sizes.
pub fn to_rgb(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    layout: Layout,
) -> Option<RgbImage> {
    if stride < width as usize * 4
        || bytes.len() < stride * (height as usize).saturating_sub(1) + width as usize * 4
    {
        return None;
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height as usize {
        let row = &bytes[y * stride..y * stride + width as usize * 4];
        for px in row.as_chunks::<4>().0 {
            match layout {
                Layout::Bgr => rgb.extend_from_slice(&[px[2], px[1], px[0]]),
                Layout::Rgb => rgb.extend_from_slice(&px[..3]),
            }
        }
    }
    RgbImage::from_raw(width, height, rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_bgrx_with_row_padding() {
        // 2×1 image, stride 12 (4 bytes padding).
        let bytes = [3, 2, 1, 0, 6, 5, 4, 0, 9, 9, 9, 9];
        let img = to_rgb(&bytes, 2, 1, 12, Layout::Bgr).unwrap();
        assert_eq!(img.into_raw(), vec![1, 2, 3, 4, 5, 6]);
        let img = to_rgb(&bytes, 2, 1, 12, Layout::Rgb).unwrap();
        assert_eq!(img.into_raw(), vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn rejects_short_buffers() {
        assert!(to_rgb(&[0; 4], 2, 1, 8, Layout::Bgr).is_none());
        assert!(to_rgb(&[0; 8], 2, 1, 4, Layout::Bgr).is_none());
    }
}
