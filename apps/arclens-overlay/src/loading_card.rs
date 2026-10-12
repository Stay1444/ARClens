//! Raid prep on the loading screen: what to look for and where, the
//! planner's goal, and the conditions around. Down the left side, clear of
//! the game's tip (centre) and spinner (bottom right).

use crate::Message;
use arclens_i18n::t;
use arclens_ipc::{CardEvent, HuntItem, LoadingCard};
use arclens_ui::detail::{self, titled};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, CREAM, DISPLAY, DISPLAY_SEMI, INK, RADIUS, STRONG};
use arclens_ui::{names, theme::size};
use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Border, Element, Length, Size};
use std::collections::HashMap;
use std::path::PathBuf;

/// Left edge, top and width, as shares of the screen.
const AREA: [f32; 3] = [0.03, 0.08, 0.24];
/// Rows per list.
const HUNT_SHOWN: usize = 6;
const EVENTS_SHOWN: usize = 4;

/// The card with its icons, decoded on arrival so `view` never decodes.
#[derive(Debug)]
pub struct Shown {
    card: LoadingCard,
    icons: HashMap<PathBuf, image::Handle>,
}

impl Shown {
    pub fn new(card: LoadingCard) -> Self {
        let icons = card
            .hunt
            .iter()
            .filter_map(|h| h.icon.clone())
            .chain(
                card.active
                    .iter()
                    .chain(&card.upcoming)
                    .filter_map(|e| e.icon.clone()),
            )
            .filter_map(|p| {
                Some((
                    p.clone(),
                    arclens_ui::decode_icon(&p, detail::LINE_ICON_PX)?,
                ))
            })
            .collect();
        Self { card, icons }
    }
}

pub fn view(shown: &Shown, now_ms: i64, screen: Size) -> Element<'_, Message> {
    let card = &shown.card;
    let mut head = column![
        text(t!("loading-kicker").to_uppercase())
            .size(size::TINY)
            .font(DISPLAY_SEMI)
            .color(with_alpha(INK, 0.65)),
        text(
            card.map
                .as_deref()
                .map_or_else(|| t!("loading-title"), |m| t!("loading-title-map", map = m))
        )
        .size(size::H2)
        .font(DISPLAY)
        .color(INK),
    ]
    .spacing(2);
    if let Some(goal) = &card.goal {
        head = head.push(text(goal.clone()).size(size::SMALL).font(STRONG).color(INK));
    }
    let head = container(head)
        .padding([12, 16])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(CREAM.into()),
            ..container::Style::default()
        });

    let mut body = column![].spacing(14);
    if card.hunt.is_empty() {
        body = body.push(
            text(t!("loading-nothing-needed"))
                .size(size::SMALL)
                .color(palette::TEXT_MUTED),
        );
    } else {
        let list = card
            .hunt
            .iter()
            .take(HUNT_SHOWN)
            .fold(column![].spacing(8), |col, item| {
                col.push(hunt_line(item, &shown.icons))
            });
        body = body.push(titled(&t!("loading-look-for"), list.into()));
    }
    let events: Vec<(&CardEvent, bool)> = card
        .active
        .iter()
        .map(|e| (e, true))
        .chain(card.upcoming.iter().map(|e| (e, false)))
        .filter(|(e, active)| *active || e.at_ms > now_ms)
        .take(EVENTS_SHOWN)
        .collect();
    if !events.is_empty() {
        let list = events
            .into_iter()
            .fold(column![].spacing(6), |col, (e, active)| {
                let when = arclens_core::countdown(e.at_ms - now_ms);
                let note = if active {
                    t!("loading-ends-in", time = when)
                } else {
                    t!("loading-starts-in", time = when)
                };
                let name = format!("{} · {}", names::condition(&e.name), names::map(&e.map));
                col.push(detail::line(detail::Line {
                    icon: e.icon.as_ref().and_then(|p| shown.icons.get(p)),
                    name: &name,
                    quantity: None,
                    note: Some(&note),
                    done: false,
                }))
            });
        body = body.push(titled(&t!("loading-conditions"), list.into()));
    }
    let body = container(body)
        .padding([12, 16])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(with_alpha(theme::PANEL, theme::surface_alpha(0.94)).into()),
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });

    let card = container(column![head, theme::stripe_band(4.0), body])
        .width((screen.width * AREA[2]).max(380.0))
        .clip(true)
        .style(|_| container::Style {
            border: Border {
                color: with_alpha(theme::ACCENT, 0.7),
                width: 2.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        });
    container(row![Space::new().width(screen.width * AREA[0]), card])
        .padding(iced::Padding {
            top: (screen.height * AREA[1]).round(),
            ..iced::Padding::ZERO
        })
        .into()
}

/// Icon tile (rarity border, like the game's slots), name and "2 / 6",
/// and where to look under the name.
fn hunt_line<'a>(
    item: &'a HuntItem,
    icons: &'a HashMap<PathBuf, image::Handle>,
) -> Element<'a, Message> {
    let rarity = palette::rarity(item.rarity);
    let icon: Element<'a, Message> = match item.icon.as_ref().and_then(|p| icons.get(p)) {
        Some(handle) => image(handle.clone()).width(38).height(38).into(),
        None => Space::new().width(38).height(38).into(),
    };
    let tile = container(icon).center(46).style(move |_| container::Style {
        background: Some(with_alpha(rarity, 0.18).into()),
        border: Border {
            color: with_alpha(rarity, 0.7),
            width: 1.5,
            radius: 5.0.into(),
        },
        ..container::Style::default()
    });
    let mut text_col = column![
        row![
            text(item.name.clone())
                .size(size::SMALL)
                .font(STRONG)
                .width(Length::Fill),
            text(t!("loading-have-need", have = item.have, need = item.need))
                .size(size::SMALL)
                .font(DISPLAY)
                .color(if item.have >= item.need {
                    palette::verdict(arclens_core::Verdict::Keep)
                } else {
                    theme::ACCENT
                }),
        ]
        .spacing(8)
    ]
    .spacing(1);
    if !item.look.is_empty() {
        text_col = text_col.push(
            text(item.look.clone())
                .size(size::TINY + 1.0)
                .color(palette::TEXT_MUTED),
        );
    }
    row![tile, text_col.width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
}
