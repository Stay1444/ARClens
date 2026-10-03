//! Colour tokens. Verdict colours are chosen to be distinguishable from each
//! other at a glance; rarity colours follow the game's conventions.

use arclens_core::{Rarity, Verdict};
use iced::Color;

pub const SURFACE: Color = Color::from_rgba(0.06, 0.07, 0.09, 0.92);
pub const SURFACE_RAISED: Color = Color::from_rgb(0.11, 0.12, 0.15);
pub const BORDER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
pub const TEXT: Color = Color::from_rgb(0.93, 0.94, 0.96);
pub const TEXT_MUTED: Color = Color::from_rgb(0.62, 0.65, 0.71);
pub const COIN: Color = Color::from_rgb(0.98, 0.80, 0.33);

pub fn rarity(rarity: Option<Rarity>) -> Color {
    match rarity {
        None | Some(Rarity::Common) => Color::from_rgb8(0x9e, 0xa3, 0xab),
        Some(Rarity::Uncommon) => Color::from_rgb8(0x5c, 0xc2, 0x6a),
        Some(Rarity::Rare) => Color::from_rgb8(0x3f, 0x9b, 0xf5),
        Some(Rarity::Epic) => Color::from_rgb8(0xb2, 0x5c, 0xf0),
        Some(Rarity::Legendary) => Color::from_rgb8(0xff, 0xa7, 0x26),
    }
}

pub fn rarity_label(rarity: Option<Rarity>) -> &'static str {
    match rarity {
        None => "",
        Some(Rarity::Common) => "COMMON",
        Some(Rarity::Uncommon) => "UNCOMMON",
        Some(Rarity::Rare) => "RARE",
        Some(Rarity::Epic) => "EPIC",
        Some(Rarity::Legendary) => "LEGENDARY",
    }
}

pub fn verdict(verdict: Verdict) -> Color {
    match verdict {
        Verdict::Keep => Color::from_rgb8(0x2e, 0xc4, 0x8a),
        Verdict::Sell => Color::from_rgb8(0xf5, 0xb8, 0x2e),
        Verdict::Recycle => Color::from_rgb8(0x3c, 0xc8, 0xe6),
        Verdict::Unknown => Color::from_rgb8(0x80, 0x86, 0x90),
    }
}

pub fn verdict_label(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Keep => "KEEP",
        Verdict::Sell => "SELL",
        Verdict::Recycle => "RECYCLE",
        Verdict::Unknown => "NO DATA",
    }
}

/// `color` at `alpha` opacity.
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
