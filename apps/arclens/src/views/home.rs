//! The Home tab, where the app opens: what's going on (game, capture,
//! overlay), the conditions running now, and one click to each part.

use crate::app::{Message, Tab};
use crate::event_icons::EventIcons;
use crate::views::events::{ACTIVE, UPCOMING, card};
use arclens_core::{ScheduledEvent, agenda, countdown};
use arclens_i18n::t;
use arclens_ui::names;
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, button, column, container, row, scrollable, svg, text, text_input};
use iced::{Alignment, Border, Element, Length};

/// Content width; the page centres it.
const WIDTH: f32 = 1240.0;
/// Conditions shown per row.
const ACTIVE_SHOWN: usize = 8;
const NEXT_SHOWN: usize = 4;
pub const SEARCH_ID: &str = "home-search";

/// A status chip: label, value, and whether it is good news.
pub struct Status {
    pub label: String,
    pub value: String,
    pub ok: bool,
}

/// The last map recognised in game.
pub struct LastMap<'a> {
    pub name: &'a str,
    pub condition: Option<&'a str>,
    pub preset: Option<String>,
}

pub struct HomeView<'a> {
    pub icon: &'a svg::Handle,
    pub status: Vec<Status>,
    /// `None` while the schedule loads (or failed).
    pub events: Option<&'a [ScheduledEvent]>,
    pub icons: &'a EventIcons,
    pub now_ms: i64,
    /// `(id, name)` of every map.
    pub maps: &'a [(&'static str, &'static str)],
    pub last_map: Option<LastMap<'a>>,
    /// Workshop levels built and in total; `None` when not set.
    pub workshop: Option<(u32, u32)>,
    pub item_count: Option<usize>,
}

pub fn view<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let header = row![
        svg(home.icon.clone()).width(64).height(64),
        column![
            text("ARCLENS").size(size::TITLE).font(theme::DISPLAY),
            text(t!("home-tagline"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        ]
        .width(Length::Fill),
        home.status
            .iter()
            .fold(row![].spacing(10), |r, s| r.push(chip(s))),
    ]
    .spacing(18)
    .align_y(Alignment::Center);

    let search = text_input(
        &match home.item_count {
            Some(n) => t!("home-search-count", count = n),
            None => t!("home-search"),
        },
        "",
    )
    .id(SEARCH_ID)
    // Typing goes straight to the Items tab with the query.
    .on_input(Message::HomeSearch)
    .padding([16, 20])
    .size(size::H2)
    .style(theme::input_style);

    let content = column![
        header,
        search,
        conditions(home),
        row![maps(home), workshop(home), in_game()]
            .spacing(space::GAP + 4.0)
            .height(Length::Shrink),
    ]
    .spacing(space::SECTION)
    .padding(theme::PAGE_PADDING)
    .max_width(WIDTH);

    scrollable(container(content).center_x(Length::Fill))
        .height(Length::Fill)
        .into()
}

fn chip<'a>(status: &Status) -> Element<'a, Message> {
    let color = if status.ok {
        ACTIVE
    } else {
        palette::TEXT_MUTED
    };
    container(
        row![
            container(Space::new().width(10).height(10)).style(move |_| container::Style {
                background: Some(color.into()),
                border: Border {
                    radius: 5.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
            column![
                theme::label(&status.label),
                text(status.value.clone())
                    .size(size::SMALL)
                    .font(theme::STRONG),
            ],
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .style(theme::card)
    .into()
}

/// Running now, then next up.
fn conditions<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let all = link(t!("home-all-events"), Message::SetTab(Tab::Events));
    let Some(events) = home.events else {
        return theme::panel(
            &t!("home-conditions"),
            Some(all),
            text(t!("home-loading-schedule"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        );
    };
    let now = home.now_ms;
    let agenda = agenda(events, now);
    let active = agenda
        .active
        .iter()
        .take(ACTIVE_SHOWN)
        .fold(row![].spacing(space::GAP), |r, e| {
            r.push(card(
                e,
                home.icons,
                t!("events-ends-in", time = countdown(e.end_ms - now)),
                ACTIVE,
            ))
        })
        .wrap()
        .vertical_spacing(space::GAP);
    let next = agenda
        .upcoming
        .iter()
        .take(NEXT_SHOWN)
        .fold(row![].spacing(space::GAP), |r, e| {
            r.push(card(
                e,
                home.icons,
                t!("home-in", time = countdown(e.start_ms - now)),
                UPCOMING,
            ))
        })
        .wrap()
        .vertical_spacing(space::GAP);
    let mut col = column![theme::label(&t!("home-now"))].spacing(10);
    if agenda.active.is_empty() {
        col = col.push(
            text(t!("home-nothing-running"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        );
    } else {
        col = col.push(active);
    }
    if !agenda.upcoming.is_empty() {
        col = col
            .push(Space::new().height(6))
            .push(theme::label(&t!("home-next")))
            .push(next);
    }
    theme::panel(&t!("home-conditions"), Some(all), col)
}

fn maps<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let last: Element<'a, Message> = match &home.last_map {
        Some(last) => column![
            theme::label(&t!("home-last-seen")),
            text(match last.condition {
                Some(c) => format!("{} · {}", names::map(last.name), names::condition(c)),
                None => names::map(last.name),
            })
            .size(size::BODY)
            .font(theme::STRONG),
        ]
        .push(last.preset.as_ref().map(|p| {
            text(t!("home-preset", name = p.as_str()))
                .size(size::SMALL)
                .color(palette::TEXT_MUTED)
        }))
        .spacing(3)
        .into(),
        None => text(t!("home-open-map-hint"))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED)
            .into(),
    };
    let buttons = home
        .maps
        .iter()
        .fold(column![].spacing(6), |col, &(id, _)| {
            col.push(list_button(names::map(id), Message::OpenMap(id.to_owned())))
        });
    panel(&t!("home-maps"), column![last, buttons].spacing(14))
}

fn workshop<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let body: Element<'a, Message> = match home.workshop {
        Some((built, total)) => {
            #[allow(clippy::cast_precision_loss, reason = "levels are small")]
            let share = if total == 0 {
                0.0
            } else {
                built as f32 / total as f32
            };
            column![
                row![
                    text(built.to_string())
                        .size(size::TITLE)
                        .font(theme::DISPLAY),
                    text(t!("home-levels-built", total = total))
                        .size(size::BODY)
                        .color(palette::TEXT_MUTED),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                iced::widget::progress_bar(0.0..=1.0, share)
                    .girth(8)
                    .style(|_| iced::widget::progress_bar::Style {
                        background: with_alpha(palette::TEXT, 0.08).into(),
                        bar: theme::ACCENT.into(),
                        border: Border {
                            radius: 4.0.into(),
                            ..Border::default()
                        },
                    }),
                text(t!("home-levels-help"))
                    .size(size::SMALL)
                    .color(palette::TEXT_MUTED),
            ]
            .spacing(10)
            .into()
        }
        None => text(t!("home-levels-unset"))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED)
            .into(),
    };
    panel(
        &t!("home-progress"),
        column![
            body,
            list_button(t!("home-edit-progress"), Message::SetTab(Tab::Progress))
        ]
        .spacing(14),
    )
}

fn in_game<'a>() -> Element<'a, Message> {
    let tip = |title: String, body: String| {
        column![
            text(title).size(size::BODY).font(theme::STRONG),
            text(body).size(size::SMALL).color(palette::TEXT_MUTED),
        ]
        .spacing(2)
    };
    panel(
        &t!("home-in-game"),
        column![
            tip(t!("home-tip-hover"), t!("home-tip-hover-body")),
            tip(t!("home-tip-map"), t!("home-tip-map-body")),
            tip("Ctrl+Shift+O".to_owned(), t!("home-tip-toggle-body")),
        ]
        .spacing(12),
    )
}

fn panel<'a>(title: &str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(theme::panel(title, None, body))
        .width(Length::FillPortion(1))
        .into()
}

fn list_button<'a>(label: String, on_press: Message) -> Element<'a, Message> {
    button(
        row![
            text(label).size(size::BODY).width(Length::Fill),
            text("›").size(size::H2).font(theme::DISPLAY),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 14])
    .on_press(on_press)
    .style(|_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(if hovered {
                theme::CREAM.into()
            } else {
                with_alpha(palette::TEXT, 0.05).into()
            }),
            text_color: if hovered { theme::INK } else { palette::TEXT },
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: theme::RADIUS.into(),
            },
            ..button::Style::default()
        }
    })
    .into()
}

fn link<'a>(label: String, on_press: Message) -> Element<'a, Message> {
    button(text(label).size(size::SMALL).font(theme::STRONG))
        .padding(0)
        .on_press(on_press)
        .style(|_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered) {
                theme::INK
            } else {
                with_alpha(theme::INK, 0.65)
            },
            ..button::Style::default()
        })
        .into()
}
