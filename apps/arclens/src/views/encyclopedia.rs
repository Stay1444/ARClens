//! The Items tab as an encyclopedia: filters over the item list, and
//! everything the data knows about the picked item (where it's found, what
//! drops it, crafting, recycling, traders, upgrades, what needs it), with
//! every item it mentions one click away.

use crate::app::Message;
use crate::icons::Icons;
use arclens_core::{ItemFilter, Rarity};
use arclens_i18n::t;
use arclens_ipc::{DetailRow, ItemDetail};
use arclens_ui::detail::{self, pairs, titled};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, RADIUS};
use arclens_ui::{ItemCard, card, names};
use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Border, Element, Length};
use std::fmt;

const RARITIES: [Rarity; 5] = [
    Rarity::Common,
    Rarity::Uncommon,
    Rarity::Rare,
    Rarity::Epic,
    Rarity::Legendary,
];

/// A choice in the type picker (`None`: every type).
#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeChoice {
    category: Option<String>,
    label: String,
}

impl fmt::Display for TypeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

impl TypeChoice {
    fn new(category: Option<&str>) -> Self {
        Self {
            label: category.map_or_else(|| t!("items-all-types"), names::item_type),
            category: category.map(str::to_owned),
        }
    }
}

/// Rarity chips and the type picker, over the item list.
pub fn filters<'a>(filter: &ItemFilter, categories: &[String]) -> Element<'a, Message> {
    let with = |rarity: Option<Rarity>| ItemFilter {
        rarity,
        ..filter.clone()
    };
    let rarities = RARITIES.iter().fold(
        row![theme::chip(
            &t!("items-all-rarities"),
            filter.rarity.is_none(),
            Message::SetItemFilter(with(None)),
        )]
        .spacing(4),
        |r, &rarity| {
            r.push(theme::chip(
                &names::rarity(rarity),
                filter.rarity == Some(rarity),
                Message::SetItemFilter(with(Some(rarity))),
            ))
        },
    );
    let choices: Vec<TypeChoice> = std::iter::once(TypeChoice::new(None))
        .chain(categories.iter().map(|c| TypeChoice::new(Some(c))))
        .collect();
    let selected = TypeChoice::new(filter.category.as_deref());
    let base = filter.clone();
    let types = pick_list(choices, Some(selected), move |choice: TypeChoice| {
        Message::SetItemFilter(ItemFilter {
            category: choice.category,
            ..base.clone()
        })
    })
    .text_size(theme::size::SMALL)
    .padding([6, 12])
    .width(Length::Fill);
    column![rarities.wrap().vertical_spacing(4), types]
        .spacing(8)
        .into()
}

/// The picked item: the card's header and verdict, then every fact and
/// list the data has.
pub fn page<'a>(
    card: &ItemCard<'a>,
    detail: &ItemDetail,
    icons: &'a Icons,
) -> Element<'a, Message> {
    let mut body = column![card::verdict(card)].spacing(18);
    if !detail.facts.is_empty() {
        body = body.push(pairs(&detail.facts, true));
    }
    if let Some(description) = &detail.item.description {
        body = body.push(
            text(description.clone())
                .size(theme::size::BODY)
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
            pairs(&detail.stats, true),
        ));
    }
    for section in &detail.sections {
        let rows = section
            .rows
            .iter()
            .fold(column![].spacing(2), |col, r| col.push(line(r, icons)));
        body = body.push(titled(&section.title, rows.into()));
    }

    let rarity = palette::rarity(detail.item.rarity);
    let head = container(card::heading(card))
        .padding(16)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(theme::CREAM.into()),
            border: Border {
                radius: iced::border::Radius::default().top(RADIUS),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let body = container(body)
        .padding(18)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(theme::PANEL.into()),
            border: Border {
                radius: iced::border::Radius::default().bottom(RADIUS),
                width: 1.0,
                color: palette::BORDER,
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });
    container(column![head, body])
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

/// A row; one naming a catalogue item opens it.
fn line<'a>(r: &DetailRow, icons: &'a Icons) -> Element<'a, Message> {
    let content = detail::line(detail::Line {
        icon: r
            .id
            .as_ref()
            .and_then(|id| icons.get(id))
            .map(|icon| &icon.thumb),
        name: &r.name,
        quantity: r.quantity,
        note: r.note.as_deref(),
        done: r.done,
    });
    let Some(id) = r.id.clone() else {
        return container(content).padding([3, 6]).into();
    };
    button(content)
        .padding([3, 6])
        .width(Length::Fill)
        .on_press(Message::Select(id))
        .style(|_, status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then(|| with_alpha(palette::TEXT, 0.08).into()),
            text_color: palette::TEXT,
            border: Border {
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .into()
}
