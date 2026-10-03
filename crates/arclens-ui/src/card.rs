//! The item card: everything a player needs to decide what to do with an
//! item, ordered by how fast it must be read.
//!
//! 1. Verdict (KEEP / SELL / RECYCLE): largest, colour-coded, with a
//!    one-line reason.
//! 2. Identity: icon on a rarity-tinted tile, name, rarity and type.
//! 3. Numbers: sell vs. recycle value, the better one highlighted.
//! 4. Details: recycling outputs and what needs the item.

use crate::format::thousands;
use crate::palette::{self, with_alpha};
use arclens_core::{Advice, Item, RequirementKind, Verdict};
use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, font};
use std::path::Path;

/// How much of the card to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardSize {
    /// Overlay: fixed width, details truncated.
    Compact,
    /// Companion app: fills its container, nothing truncated.
    Full,
}

/// Inputs for [`item_card`].
#[derive(Debug, Clone)]
pub struct ItemCard<'a> {
    pub item: &'a Item,
    pub advice: Advice,
    /// Pre-decoded icon (see [`decode_icon`]). Never decode in `view`.
    pub icon: Option<&'a image::Handle>,
    /// Display names aligned with `item.recycles_into`.
    pub recycle_names: Vec<String>,
    pub size: CardSize,
}

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};
const COMPACT_WIDTH: f32 = 360.0;
const COMPACT_LIST_LIMIT: usize = 3;

pub fn item_card<'a, Message: 'a>(card: &ItemCard<'a>) -> Element<'a, Message> {
    let rarity_color = palette::rarity(card.item.rarity);
    let icon_size = match card.size {
        CardSize::Compact => 56.0,
        CardSize::Full => 96.0,
    };

    let mut body = column![
        header(card, rarity_color, icon_size),
        verdict_bar(card),
        values(&card.advice),
    ]
    .spacing(10);

    if let Some(section) = recycles_section(card) {
        body = body.push(section);
    }
    if let Some(section) = needed_for_section(card) {
        body = body.push(section);
    }
    if card.size == CardSize::Full
        && let Some(description) = &card.item.description
    {
        body = body.push(text(description).size(13).color(palette::TEXT_MUTED));
    }

    let width = match card.size {
        CardSize::Compact => Length::Fixed(COMPACT_WIDTH),
        CardSize::Full => Length::Fill,
    };

    // The border carries the rarity colour so the tier registers even in
    // peripheral vision.
    container(body.padding(14))
        .width(width)
        .style(move |_| container::Style {
            background: Some(palette::SURFACE.into()),
            border: Border {
                color: with_alpha(rarity_color, 0.55),
                width: 1.5,
                radius: 8.0.into(),
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        })
        .into()
}

fn header<'a, Message: 'a>(
    card: &ItemCard<'a>,
    rarity_color: Color,
    icon_size: f32,
) -> Element<'a, Message> {
    let icon: Element<'a, Message> = match card.icon {
        Some(handle) => image(handle.clone())
            .width(icon_size - 8.0)
            .height(icon_size - 8.0)
            .into(),
        None => Space::new()
            .width(icon_size - 8.0)
            .height(icon_size - 8.0)
            .into(),
    };
    let tile = container(icon)
        .width(icon_size)
        .height(icon_size)
        .center(icon_size)
        .style(move |_| container::Style {
            background: Some(with_alpha(rarity_color, 0.18).into()),
            border: Border {
                color: with_alpha(rarity_color, 0.6),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

    let mut subtitle = palette::rarity_label(card.item.rarity).to_owned();
    if let Some(category) = &card.item.category {
        if !subtitle.is_empty() {
            subtitle.push_str(" · ");
        }
        subtitle.push_str(&category.to_uppercase());
    }

    let name_size = match card.size {
        CardSize::Compact => 19,
        CardSize::Full => 26,
    };
    row![
        tile,
        column![
            text(&card.item.name).size(name_size).font(BOLD),
            text(subtitle).size(11).color(rarity_color),
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn verdict_bar<'a, Message: 'a>(card: &ItemCard<'a>) -> Element<'a, Message> {
    let verdict = card.advice.verdict;
    let color = palette::verdict(verdict);
    container(
        row![
            text(palette::verdict_label(verdict))
                .size(22)
                .font(BOLD)
                .color(color),
            text(reason(card)).size(13).color(palette::TEXT),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(with_alpha(color, 0.14).into()),
        border: Border {
            color: with_alpha(color, 0.7),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// One short line explaining the verdict.
fn reason(card: &ItemCard<'_>) -> String {
    let advice = &card.advice;
    match advice.verdict {
        Verdict::Keep => {
            let n = card.item.required_for.len();
            let first = card
                .item
                .required_for
                .first()
                .map_or("", |r| r.name.as_str());
            if n == 1 {
                format!("Needed for {first}")
            } else {
                format!("Needed for {first} +{} more", n - 1)
            }
        }
        Verdict::Recycle => match (advice.recycle_value, advice.sell_value) {
            (Some(r), Some(s)) => format!("+{} more than selling", thousands(r - s)),
            _ => "Worth more as parts".to_owned(),
        },
        Verdict::Sell => match (advice.sell_value, advice.recycle_value) {
            (Some(s), Some(r)) if s > r => format!("+{} more than recycling", thousands(s - r)),
            (Some(_), Some(_)) => "Same value either way".to_owned(),
            _ => "Doesn't recycle into anything useful".to_owned(),
        },
        Verdict::Unknown => "No value data for this item".to_owned(),
    }
}

fn values<'a, Message: 'a>(advice: &Advice) -> Element<'a, Message> {
    let best_is_recycle = advice.verdict == Verdict::Recycle;
    row![
        stat(
            "SELL",
            advice.sell_value,
            !best_is_recycle && advice.verdict != Verdict::Keep
        ),
        stat("RECYCLE", advice.recycle_value, best_is_recycle),
    ]
    .spacing(8)
    .into()
}

fn stat<'a, Message: 'a>(
    label: &'a str,
    value: Option<u32>,
    highlight: bool,
) -> Element<'a, Message> {
    let value_text = value.map_or_else(|| "—".to_owned(), |v| format!("{} ¢", thousands(v)));
    let value_color = if value.is_some() {
        palette::COIN
    } else {
        palette::TEXT_MUTED
    };
    container(
        column![
            text(label).size(10).color(palette::TEXT_MUTED),
            text(value_text).size(17).font(BOLD).color(value_color),
        ]
        .spacing(2),
    )
    .padding([6, 10])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(palette::SURFACE_RAISED.into()),
        border: Border {
            color: if highlight {
                with_alpha(palette::COIN, 0.6)
            } else {
                palette::BORDER
            },
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn recycles_section<'a, Message: 'a>(card: &ItemCard<'a>) -> Option<Element<'a, Message>> {
    if card.item.recycles_into.is_empty() {
        return None;
    }
    let parts: Vec<String> = card
        .item
        .recycles_into
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let name = card
                .recycle_names
                .get(i)
                .map_or(q.item.as_str(), String::as_str);
            format!("{name} ×{}", q.quantity)
        })
        .collect();
    Some(section("RECYCLES INTO", vec![parts.join("  ·  ")]))
}

fn needed_for_section<'a, Message: 'a>(card: &ItemCard<'a>) -> Option<Element<'a, Message>> {
    let reqs = &card.item.required_for;
    if reqs.is_empty() {
        return None;
    }
    let limit = match card.size {
        CardSize::Compact => COMPACT_LIST_LIMIT,
        CardSize::Full => usize::MAX,
    };
    let mut lines: Vec<String> = reqs
        .iter()
        .take(limit)
        .map(|r| format!("{}  {} ×{}", kind_tag(r.kind), r.name, r.quantity))
        .collect();
    if reqs.len() > limit {
        lines.push(format!("+{} more", reqs.len() - limit));
    }
    Some(section("NEEDED FOR", lines))
}

fn kind_tag(kind: RequirementKind) -> &'static str {
    match kind {
        RequirementKind::Quest => "Quest",
        RequirementKind::WorkshopUpgrade => "Workshop",
        RequirementKind::Project => "Project",
        RequirementKind::Crafting => "Crafting",
    }
}

fn section<'a, Message: 'a>(title: &'a str, lines: Vec<String>) -> Element<'a, Message> {
    lines
        .into_iter()
        .fold(
            column![text(title).size(10).color(palette::TEXT_MUTED)].spacing(3),
            |col, line| col.push(text(line).size(13)),
        )
        .into()
}

/// Decodes an icon file into an RGBA handle. Call once per icon, off the
/// render path. iced's lazy `Handle::from_path` draws nothing under
/// `iced_layershell` (verified 2026-10-03), so icons are always pre-decoded.
///
/// `max_px` bounds the decoded size (icons ship at 256²; a 32 px list
/// thumbnail does not need 256 KiB of RGBA).
pub fn decode_icon(path: &Path, max_px: u32) -> Option<image::Handle> {
    let img = ::image::open(path).ok()?;
    let img = if img.width() > max_px || img.height() > max_px {
        img.thumbnail(max_px, max_px)
    } else {
        img
    };
    let rgba = img.into_rgba8();
    Some(image::Handle::from_rgba(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}
