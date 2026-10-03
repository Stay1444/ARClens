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
use crate::theme::{self, size};
use arclens_core::{Advice, Item, Place, RequirementKind, Verdict, breakdown};
use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Border, Color, Element, Length};
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
    /// Display names aligned with the breakdown outputs for `advice.place`
    /// (`recycles_into` at the workshop, `salvages_into` in raid).
    pub recycle_names: Vec<String>,
    pub size: CardSize,
}

const COMPACT_WIDTH: f32 = 380.0;
const COMPACT_LIST_LIMIT: usize = 3;

pub fn item_card<'a, Message: 'a>(card: &ItemCard<'a>) -> Element<'a, Message> {
    let rarity_color = palette::rarity(card.item.rarity);
    let icon_size = match card.size {
        CardSize::Compact => 56.0,
        CardSize::Full => 96.0,
    };

    let mut body = column![verdict_bar(card), values(&card.advice)].spacing(12);

    if let Some(section) = recycles_section(card) {
        body = body.push(section);
    }
    if let Some(section) = needed_for_section(card) {
        body = body.push(section);
    }
    if card.advice.verdict == Verdict::Sell
        && let Some(upgrade) = &card.advice.parts_for
    {
        body = body.push(section(
            "PARTS WOULD HELP WITH",
            vec![format!("{upgrade} (recycle if you're short on parts)")],
        ));
    }
    if let Some(section) = crafts_section(card) {
        body = body.push(section);
    }
    if card.size == CardSize::Full
        && let Some(description) = &card.item.description
    {
        body = body.push(
            text(description)
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        );
    }

    let width = match card.size {
        CardSize::Compact => Length::Fixed(COMPACT_WIDTH),
        CardSize::Full => Length::Fill,
    };

    // Like the game's tooltip: a cream header with the name in capitals,
    // over a dark body. The border carries the rarity colour so the tier
    // registers even in peripheral vision.
    let head = container(header(card, rarity_color, icon_size))
        .padding(14)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(theme::CREAM.into()),
            border: Border {
                radius: iced::border::Radius::default().top(theme::RADIUS),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(column![head, body.padding(14)])
        .width(width)
        .style(move |_| container::Style {
            background: Some(with_alpha(theme::PANEL, 0.97).into()),
            border: Border {
                color: with_alpha(rarity_color, 0.7),
                width: 2.0,
                radius: theme::RADIUS.into(),
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
            background: Some(theme::PANEL.into()),
            border: Border {
                color: rarity_color,
                width: 2.0,
                radius: theme::RADIUS.into(),
            },
            ..container::Style::default()
        });

    // Tags like the game's: the type, then the rarity on its colour.
    let mut tags = row![].spacing(3);
    if let Some(category) = &card.item.category {
        tags = tags.push(tag(&category.to_uppercase(), theme::INK));
    }
    let rarity = palette::rarity_label(card.item.rarity);
    if !rarity.is_empty() {
        tags = tags.push(tag(rarity, rarity_color));
    }

    let name_size = match card.size {
        CardSize::Compact => 26.0,
        CardSize::Full => size::TITLE,
    };
    row![
        tile,
        column![
            tags,
            text(card.item.name.to_uppercase())
                .size(name_size)
                .font(theme::DISPLAY)
                .color(theme::INK),
        ]
        .spacing(4),
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

/// A small tag with light text on `color`.
fn tag<'a, Message: 'a>(label: &str, color: Color) -> Element<'a, Message> {
    container(
        text(label.to_owned())
            .size(size::TINY)
            .font(theme::DISPLAY_SEMI)
            .color(Color::WHITE),
    )
    .padding([1, 6])
    .style(move |_| container::Style {
        background: Some(color.into()),
        ..container::Style::default()
    })
    .into()
}

fn verdict_bar<'a, Message: 'a>(card: &ItemCard<'a>) -> Element<'a, Message> {
    let verdict = card.advice.verdict;
    let color = palette::verdict(verdict);
    container(
        row![
            text(palette::verdict_label_in(verdict, card.advice.place))
                .size(30)
                .font(theme::DISPLAY)
                .color(theme::INK),
            text(reason(card))
                .size(size::SMALL)
                .font(theme::STRONG)
                .color(theme::INK),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(color.into()),
        border: Border {
            radius: theme::RADIUS.into(),
            ..Border::default()
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
            let n = advice.needs.len();
            match advice.needs.first() {
                None => "Worth keeping".to_owned(),
                Some(first) if n == 1 => format!("Needed for {}", first.name),
                Some(first) => format!("Needed for {} +{} more", first.name, n - 1),
            }
        }
        Verdict::Recycle => match (&advice.parts_for, advice.recycle_value, advice.sell_value) {
            (Some(upgrade), _, _) => format!("Parts needed for {upgrade}"),
            (None, Some(r), Some(s)) if r > s => format!("+{} more than selling", thousands(r - s)),
            _ => "Worth more as parts".to_owned(),
        },
        Verdict::Sell => match (advice.sell_value, advice.recycle_value) {
            (Some(s), Some(r)) if s > r => match advice.place {
                Place::Workshop => format!("+{} more than recycling", thousands(s - r)),
                Place::Raid => format!("Carry out: +{} more than salvaging", thousands(s - r)),
            },
            (Some(_), Some(_)) => "Same value either way".to_owned(),
            _ => "Doesn't recycle into anything useful".to_owned(),
        },
        Verdict::Learn => "Learn it to unlock crafting rather than selling it".to_owned(),
        Verdict::Unknown => "No value data for this item".to_owned(),
    }
}

fn values<'a, Message: 'a>(advice: &Advice) -> Element<'a, Message> {
    let best_is_recycle = advice.verdict == Verdict::Recycle;
    let in_raid = advice.place == Place::Raid;
    // In raid the tooltip shows no value, so ours is the undamaged base.
    let sell_label = if in_raid && !advice.value_from_game {
        "SELL (BASE VALUE)"
    } else {
        "SELL"
    };
    row![
        stat(
            sell_label,
            advice.sell_value,
            advice.verdict == Verdict::Sell
        ),
        stat(
            if in_raid { "SALVAGE" } else { "RECYCLE" },
            advice.recycle_value,
            best_is_recycle
        ),
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
            text(label)
                .size(size::TINY)
                .font(theme::DISPLAY_SEMI)
                .color(palette::TEXT_MUTED),
            text(value_text)
                .size(size::H2)
                .font(theme::STRONG)
                .color(value_color),
        ]
        .spacing(1),
    )
    .padding([7, 12])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme::PANEL_RAISED.into()),
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
    let outputs = breakdown(card.item, card.advice.place);
    if outputs.is_empty() {
        return None;
    }
    let parts: Vec<String> = outputs
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
    let title = match card.advice.place {
        Place::Workshop => "RECYCLES INTO",
        Place::Raid => "SALVAGES INTO",
    };
    Some(section(title, vec![parts.join("  ·  ")]))
}

fn needed_for_section<'a, Message: 'a>(card: &ItemCard<'a>) -> Option<Element<'a, Message>> {
    let reqs = &card.advice.needs;
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

/// For materials: what they're used to craft. Informational only — every
/// common material feeds *something*, so it doesn't change the verdict.
fn crafts_section<'a, Message: 'a>(card: &ItemCard<'a>) -> Option<Element<'a, Message>> {
    let products = &card.item.ingredient_of;
    if products.is_empty() {
        return None;
    }
    let limit = match card.size {
        CardSize::Compact => 4,
        CardSize::Full => usize::MAX,
    };
    let mut line = products
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if products.len() > limit {
        line = format!("{line} +{} more", products.len() - limit);
    }
    Some(section("USED TO CRAFT", vec![line]))
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
            column![
                text(title)
                    .size(size::TINY)
                    .font(theme::DISPLAY_SEMI)
                    .color(palette::TEXT_MUTED)
            ]
            .spacing(3),
            |col, line| col.push(text(line).size(size::SMALL)),
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
