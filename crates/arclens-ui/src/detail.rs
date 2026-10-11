//! Pieces of an item's detailed view, shared by the overlay's item windows
//! and the app's item encyclopedia: titled sections, `(label, value)`
//! facts, and item lines (icon, name, amount, note).

use crate::palette::{self, with_alpha};
use crate::theme::{self, DISPLAY, DISPLAY_SEMI, STRONG};
use iced::widget::{Space, column, image, row, text};
use iced::{Alignment, Element, Length};

/// Line icon size, logical pixels.
pub const LINE_ICON: f32 = 32.0;
/// The size line icons are decoded at (sharp at 2× scale).
pub const LINE_ICON_PX: u32 = 64;

/// A section: small caps title over its content.
pub fn titled<'a, M: 'a>(title: &str, content: Element<'a, M>) -> Element<'a, M> {
    column![
        text(title.to_uppercase())
            .size(theme::size::TINY)
            .font(DISPLAY_SEMI)
            .color(palette::TEXT_MUTED),
        content
    ]
    .spacing(6)
    .into()
}

/// `(label, value)` pairs, two to a line when `grid`, else one per line.
pub fn pairs<'a, M: 'a>(list: &[(String, String)], grid: bool) -> Element<'a, M> {
    let cell = |(label, value): &(String, String)| {
        row![
            text(label.clone())
                .size(theme::size::SMALL)
                .color(palette::TEXT_MUTED)
                .width(Length::Fill),
            text(value.clone()).size(theme::size::SMALL).font(STRONG),
        ]
        .spacing(8)
        .width(Length::Fill)
    };
    let mut col = column![].spacing(4);
    if grid {
        for chunk in list.chunks(2) {
            let mut line = row![].spacing(18);
            for pair in chunk {
                line = line.push(cell(pair));
            }
            if chunk.len() == 1 {
                line = line.push(Space::new().width(Length::Fill));
            }
            col = col.push(line);
        }
    } else {
        for pair in list {
            col = col.push(cell(pair));
        }
    }
    col.into()
}

/// One line of a section.
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    pub icon: Option<&'a image::Handle>,
    pub name: &'a str,
    pub quantity: Option<u32>,
    pub note: Option<&'a str>,
    /// Dimmed and ticked: an upgrade built, a quest done.
    pub done: bool,
}

/// Icon, name and amount, and the note on the right.
pub fn line<'a, M: 'a>(l: Line<'_>) -> Element<'a, M> {
    let alpha = if l.done { 0.45 } else { 1.0 };
    let icon: Element<'a, M> = match l.icon {
        Some(handle) => image(handle.clone())
            .width(LINE_ICON)
            .height(LINE_ICON)
            .opacity(alpha)
            .into(),
        None => Space::new().width(0).height(LINE_ICON).into(),
    };
    let name = match l.quantity {
        Some(q) => format!("{} ×{q}", l.name),
        None => l.name.to_owned(),
    };
    let mut out = row![
        icon,
        text(if l.done { format!("✓ {name}") } else { name })
            .size(theme::size::SMALL)
            .font(if l.done { theme::BODY } else { STRONG })
            .color(with_alpha(palette::TEXT, alpha))
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(note) = l.note {
        out = out.push(
            text(note.to_owned())
                .size(theme::size::TINY + 1.0)
                .font(DISPLAY)
                .color(with_alpha(palette::TEXT_MUTED, alpha)),
        );
    }
    out.into()
}
