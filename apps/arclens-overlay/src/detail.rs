//! Item windows: what a pick in the quick search opens. Several can be
//! open side by side (newest first, next to the search box); each closes
//! with its ✕, the newest with Esc, all of them from the search panel.

use crate::Message;
use arclens_core::ItemId;
use arclens_i18n::t;
use arclens_ipc::{DetailRow, ItemDetail};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, DISPLAY, DISPLAY_SEMI, INK, RADIUS, STRONG};
use arclens_ui::{CardSize, ItemCard, card};
use iced::widget::{Space, button, column, container, image, row, scrollable, text};
use iced::{Alignment, Border, Element, Length};
use std::collections::HashMap;
use std::path::PathBuf;

/// Windows kept open at most; opening another drops the oldest.
pub const MAX_OPEN: usize = 4;
/// Window width, logical pixels.
pub const WIDTH: f32 = 470.0;
/// Space between windows.
pub const GAP: f32 = 14.0;
/// Row icon size, and the size it is decoded at (sharp at 2× scale).
const ROW_ICON: f32 = 32.0;
const ROW_ICON_PX: u32 = 64;

/// An open window: its data and its icons, decoded once on arrival.
#[derive(Debug)]
pub struct OpenItem {
    detail: Box<ItemDetail>,
    icon: Option<image::Handle>,
    row_icons: HashMap<PathBuf, image::Handle>,
}

impl OpenItem {
    fn new(detail: Box<ItemDetail>) -> Self {
        let icon = detail
            .icon
            .as_deref()
            .and_then(|p| arclens_ui::decode_icon(p, 160));
        let row_icons = detail
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .filter_map(|r| r.icon.clone())
            .filter_map(|p| Some((p.clone(), arclens_ui::decode_icon(&p, ROW_ICON_PX)?)))
            .collect();
        Self {
            detail,
            icon,
            row_icons,
        }
    }

    pub fn id(&self) -> &ItemId {
        &self.detail.item.id
    }
}

/// The open windows, newest first.
#[derive(Debug, Default)]
pub struct Details {
    pub open: Vec<OpenItem>,
}

#[derive(Debug, Clone)]
pub enum DetailMessage {
    Close(ItemId),
    CloseAll,
}

impl Details {
    /// Opens `detail`, or brings its window to the front with new data.
    pub fn open(&mut self, detail: Box<ItemDetail>) {
        self.open.retain(|o| o.id() != &detail.item.id);
        self.open.insert(0, OpenItem::new(detail));
        self.open.truncate(MAX_OPEN);
    }

    /// Replaces an open window's data in place; nothing if it is closed.
    pub fn update(&mut self, detail: Box<ItemDetail>) {
        if let Some(slot) = self.open.iter_mut().find(|o| o.id() == &detail.item.id) {
            *slot = OpenItem::new(detail);
        }
    }

    pub fn close_newest(&mut self) -> bool {
        if self.open.is_empty() {
            return false;
        }
        self.open.remove(0);
        true
    }

    pub fn update_message(&mut self, msg: DetailMessage) {
        match msg {
            DetailMessage::Close(id) => self.open.retain(|o| o.id() != &id),
            DetailMessage::CloseAll => self.open.clear(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }
}

/// The windows that fit in `width` (newest first), side by side.
pub fn windows(details: &Details, width: f32, height: f32) -> Option<Element<'_, Message>> {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a handful of windows"
    )]
    let fit = (((width + GAP) / (WIDTH + GAP)).floor() as usize).max(1);
    let shown: Vec<_> = details.open.iter().take(fit).collect();
    if shown.is_empty() {
        return None;
    }
    Some(
        shown
            .into_iter()
            .fold(row![].spacing(GAP).align_y(Alignment::Start), |r, open| {
                r.push(window(open, height))
            })
            .into(),
    )
}

/// One window: the card's header and verdict, then everything else,
/// scrolling past `height`.
fn window(open: &OpenItem, height: f32) -> Element<'_, Message> {
    let detail = &open.detail;
    let card = ItemCard {
        item: &detail.item,
        advice: detail.advice.clone(),
        icon: open.icon.as_ref(),
        recycle_names: Vec::new(),
        size: CardSize::Full,
    };
    let head = head(&card, &detail.item.id);

    let mut body = column![card::verdict(&card)].spacing(14);
    if !detail.facts.is_empty() {
        body = body.push(pairs(&detail.facts, true));
    }
    if let Some(description) = &detail.item.description {
        body = body.push(
            text(description.clone())
                .size(theme::size::SMALL)
                .color(palette::TEXT_MUTED),
        );
    }
    if let Some(tip) = &detail.item.details.tip {
        body = body.push(
            text(tip.clone())
                .size(theme::size::SMALL)
                .color(with_alpha(theme::ACCENT, 0.9)),
        );
    }
    if !detail.stats.is_empty() {
        body = body.push(titled(
            &t!("overlay-detail-stats"),
            pairs(&detail.stats, false),
        ));
    }
    for section in &detail.sections {
        let rows = section.rows.iter().fold(column![].spacing(4), |col, r| {
            col.push(row_view(r, &open.row_icons))
        });
        body = body.push(titled(&section.title, rows.into()));
    }

    let body = container(
        // A thin bar in its own gutter, clear of the values on the right.
        scrollable(body)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(4)
                    .scroller_width(4)
                    .spacing(8),
            ))
            .height(Length::Shrink),
    )
    .padding(14)
    .max_height((height - 140.0).max(200.0))
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(with_alpha(theme::PANEL, theme::surface_alpha(0.97)).into()),
        border: Border {
            radius: iced::border::Radius::default().bottom(RADIUS),
            width: 1.0,
            color: palette::BORDER,
        },
        text_color: Some(palette::TEXT),
        ..container::Style::default()
    });
    let rarity = palette::rarity(detail.item.rarity);
    container(column![head, body])
        .width(WIDTH)
        .style(move |_| container::Style {
            border: Border {
                color: with_alpha(rarity, 0.7),
                width: 2.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The cream header: the card's (icon, tags, name) and a close button.
fn head<'a>(card: &ItemCard<'a>, id: &ItemId) -> Element<'a, Message> {
    let close = button(text("✕").size(18).font(STRONG).color(INK))
        .padding([2, 10])
        .on_press(Message::Detail(DetailMessage::Close(id.clone())))
        .style(|_, status| button::Style {
            background: Some(
                with_alpha(
                    INK,
                    if matches!(status, button::Status::Hovered) {
                        0.15
                    } else {
                        0.0
                    },
                )
                .into(),
            ),
            text_color: INK,
            border: Border {
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    container(
        row![container(card::heading(card)).width(Length::Fill), close].align_y(Alignment::Start),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(theme::CREAM.into()),
        border: Border {
            radius: iced::border::Radius::default().top(RADIUS),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// A section: small caps title over its content.
fn titled<'a>(title: &str, content: Element<'a, Message>) -> Element<'a, Message> {
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
fn pairs(list: &[(String, String)], grid: bool) -> Element<'_, Message> {
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

/// One row: icon, name, amount, and a note on the right.
fn row_view<'a>(
    r: &'a DetailRow,
    icons: &'a HashMap<PathBuf, image::Handle>,
) -> Element<'a, Message> {
    let alpha = if r.done { 0.45 } else { 1.0 };
    let icon: Element<'a, Message> = match r.icon.as_ref().and_then(|p| icons.get(p)) {
        Some(handle) => image(handle.clone())
            .width(ROW_ICON)
            .height(ROW_ICON)
            .opacity(alpha)
            .into(),
        None => Space::new().width(0).height(ROW_ICON).into(),
    };
    let name = match r.quantity {
        Some(q) => format!("{} ×{q}", r.name),
        None => r.name.clone(),
    };
    let mut line = row![
        icon,
        text(if r.done { format!("✓ {name}") } else { name })
            .size(theme::size::SMALL)
            .font(if r.done { theme::BODY } else { STRONG })
            .color(with_alpha(palette::TEXT, alpha))
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(note) = &r.note {
        line = line.push(
            text(note.clone())
                .size(theme::size::TINY + 1.0)
                .font(DISPLAY)
                .color(with_alpha(palette::TEXT_MUTED, alpha)),
        );
    }
    line.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::Item;

    fn detail(id: &str) -> Box<ItemDetail> {
        let item: Item =
            serde_json::from_value(serde_json::json!({"id": id, "name": id})).expect("item");
        Box::new(ItemDetail {
            advice: arclens_core::advise(&item, |_| None),
            item,
            icon: None,
            facts: Vec::new(),
            stats: Vec::new(),
            sections: Vec::new(),
        })
    }

    fn ids(d: &Details) -> Vec<&str> {
        d.open.iter().map(|o| o.id().as_str()).collect()
    }

    #[test]
    fn opens_newest_first_without_duplicates_and_caps() {
        let mut d = Details::default();
        for id in ["a", "b", "c"] {
            d.open(detail(id));
        }
        assert_eq!(ids(&d), ["c", "b", "a"]);
        d.open(detail("a"));
        assert_eq!(ids(&d), ["a", "c", "b"]);
        d.open(detail("d"));
        d.open(detail("e"));
        assert_eq!(ids(&d), ["e", "d", "a", "c"]);
    }

    #[test]
    fn closes_one_the_newest_or_all() {
        let mut d = Details::default();
        for id in ["a", "b", "c"] {
            d.open(detail(id));
        }
        d.update_message(DetailMessage::Close(ItemId::new("b")));
        assert_eq!(ids(&d), ["c", "a"]);
        assert!(d.close_newest());
        assert_eq!(ids(&d), ["a"]);
        d.update_message(DetailMessage::CloseAll);
        assert!(d.is_empty());
        assert!(!d.close_newest());
    }

    #[test]
    fn updates_only_open_windows() {
        let mut d = Details::default();
        d.open(detail("a"));
        d.update(detail("b"));
        assert_eq!(ids(&d), ["a"]);
    }
}
