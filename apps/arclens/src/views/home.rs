//! The Home tab, where the app opens: what's going on (game, capture,
//! overlay), the conditions running now, and one click to each part.

use crate::app::{Message, Tab};
use crate::event_icons::EventIcons;
use crate::views::events::{ACTIVE, UPCOMING, card, tile};
use arclens_core::{ScheduledEvent, agenda, countdown};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, column, container, row, scrollable, svg, text, text_input};
use iced::{Alignment, Border, Color, Element, Font, Length, font};

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};
/// Content width; the page centres it.
const WIDTH: f32 = 1040.0;
/// Conditions shown per row.
const ACTIVE_SHOWN: usize = 8;
const NEXT_SHOWN: usize = 4;
pub const SEARCH_ID: &str = "home-search";

/// A status chip: label, value, and whether it is good news.
pub struct Status<'a> {
    pub label: &'a str,
    pub value: String,
    pub ok: bool,
}

/// The last map recognised in game.
pub struct LastMap<'a> {
    pub name: &'a str,
    pub condition: Option<&'a str>,
    pub preset: Option<&'a str>,
}

pub struct HomeView<'a> {
    pub icon: &'a svg::Handle,
    pub status: Vec<Status<'a>>,
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
        svg(home.icon.clone()).width(56).height(56),
        column![
            text("ARClens").size(28).font(BOLD),
            text("Companion and overlay for ARC Raiders")
                .size(14)
                .color(palette::TEXT_MUTED),
        ]
        .spacing(2)
        .width(Length::Fill),
        home.status
            .iter()
            .fold(row![].spacing(8), |r, s| r.push(chip(s))),
    ]
    .spacing(16)
    .align_y(Alignment::Center);

    let search = text_input(
        &match home.item_count {
            Some(n) => format!("Look up any of {n} items…"),
            None => "Look up an item…".to_owned(),
        },
        "",
    )
    .id(SEARCH_ID)
    // Typing goes straight to the Items tab with the query.
    .on_input(Message::HomeSearch)
    .padding([12, 16])
    .size(17);

    let content = column![
        header,
        search,
        conditions(home),
        row![maps(home), workshop(home), in_game()]
            .spacing(16)
            .height(Length::Shrink),
    ]
    .spacing(24)
    .padding([28, 24])
    .max_width(WIDTH);

    scrollable(container(content).center_x(Length::Fill))
        .height(Length::Fill)
        .into()
}

fn chip<'a>(status: &Status<'a>) -> Element<'a, Message> {
    let color = if status.ok {
        ACTIVE
    } else {
        palette::TEXT_MUTED
    };
    container(
        row![
            container(Space::new().width(8).height(8)).style(move |_| container::Style {
                background: Some(color.into()),
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
            column![
                text(status.label).size(10).color(palette::TEXT_MUTED),
                text(status.value.clone()).size(13),
            ],
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(|_| tile(palette::BORDER))
    .into()
}

/// Running now, then next up.
fn conditions<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let title = row![
        text("MAP CONDITIONS")
            .size(12)
            .color(palette::TEXT_MUTED)
            .width(Length::Fill),
        link("All events →", Message::SetTab(Tab::Events)),
    ]
    .align_y(Alignment::Center);
    let Some(events) = home.events else {
        return column![
            title,
            text("Loading the schedule…").color(palette::TEXT_MUTED)
        ]
        .spacing(10)
        .into();
    };
    let now = home.now_ms;
    let agenda = agenda(events, now);
    let active = agenda
        .active
        .iter()
        .take(ACTIVE_SHOWN)
        .fold(row![].spacing(10), |r, e| {
            r.push(card(
                e,
                home.icons,
                format!("Ends in {}", countdown(e.end_ms - now)),
                ACTIVE,
            ))
        })
        .wrap();
    let next = agenda
        .upcoming
        .iter()
        .take(NEXT_SHOWN)
        .fold(row![].spacing(10), |r, e| {
            r.push(card(
                e,
                home.icons,
                format!("In {}", countdown(e.start_ms - now)),
                UPCOMING,
            ))
        })
        .wrap();
    let mut col = column![title].spacing(10);
    if agenda.active.is_empty() {
        col = col.push(text("Nothing running right now.").color(palette::TEXT_MUTED));
    } else {
        col = col.push(active);
    }
    if !agenda.upcoming.is_empty() {
        col = col
            .push(text("NEXT").size(11).color(palette::TEXT_MUTED))
            .push(next);
    }
    col.into()
}

fn maps<'a>(home: &HomeView<'a>) -> Element<'a, Message> {
    let last: Element<'a, Message> = match &home.last_map {
        Some(last) => column![
            text("Last seen in game")
                .size(11)
                .color(palette::TEXT_MUTED),
            text(match last.condition {
                Some(c) => format!("{} · {c}", last.name),
                None => last.name.to_owned(),
            })
            .size(14),
        ]
        .push(last.preset.map(|p| {
            text(format!("Preset: {p}"))
                .size(12)
                .color(palette::TEXT_MUTED)
        }))
        .spacing(2)
        .into(),
        None => text("Open the map in game and markers appear on it.")
            .size(12)
            .color(palette::TEXT_MUTED)
            .into(),
    };
    let buttons = home
        .maps
        .iter()
        .fold(column![].spacing(4), |col, &(id, name)| {
            col.push(list_button(name, Message::OpenMap(id.to_owned())))
        });
    panel("Maps", column![last, buttons].spacing(12))
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
                text(format!("{built} of {total} levels built")).size(14),
                iced::widget::progress_bar(0.0..=1.0, share)
                    .girth(6)
                    .style(|_| iced::widget::progress_bar::Style {
                        background: with_alpha(palette::TEXT, 0.08).into(),
                        bar: ACTIVE.into(),
                        border: Border {
                            radius: 3.0.into(),
                            ..Border::default()
                        },
                    }),
                text("Advice keeps what your next upgrades need.")
                    .size(12)
                    .color(palette::TEXT_MUTED),
            ]
            .spacing(8)
            .into()
        }
        None => text(
            "Set your workshop levels so keep / sell / recycle advice knows what you still need.",
        )
        .size(12)
        .color(palette::TEXT_MUTED)
        .into(),
    };
    panel(
        "Workshop",
        column![
            body,
            list_button("Edit levels", Message::SetTab(Tab::Progress))
        ]
        .spacing(12),
    )
}

fn in_game<'a>() -> Element<'a, Message> {
    let tip = |title: &'a str, body: &'a str| {
        column![
            text(title).size(13).font(BOLD),
            text(body).size(12).color(palette::TEXT_MUTED),
        ]
        .spacing(2)
    };
    panel(
        "In game",
        column![
            tip(
                "Hover an item",
                "A card beside the game's tooltip says keep, sell or recycle."
            ),
            tip(
                "Open the map",
                "Markers follow the map; the panel bottom right switches presets."
            ),
            tip("Ctrl+Shift+O", "Show or hide the overlay."),
        ]
        .spacing(10),
    )
}

fn panel<'a>(title: &'a str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(
        column![
            text(title.to_uppercase())
                .size(12)
                .color(palette::TEXT_MUTED),
            body.into(),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::FillPortion(1))
    .style(|_| tile(palette::BORDER))
    .into()
}

fn list_button(label: &str, on_press: Message) -> Element<'_, Message> {
    button(
        row![text(label).size(13).width(Length::Fill), text("→").size(13)]
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([6, 10])
    .on_press(on_press)
    .style(|_, status| button::Style {
        background: Some(
            with_alpha(
                palette::TEXT,
                if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    0.12
                } else {
                    0.05
                },
            )
            .into(),
        ),
        text_color: palette::TEXT,
        border: Border {
            color: palette::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    })
    .into()
}

fn link(label: &str, on_press: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding(0)
        .on_press(on_press)
        .style(|_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered) {
                palette::TEXT
            } else {
                Color::from_rgb(0.36, 0.62, 0.98)
            },
            ..button::Style::default()
        })
        .into()
}
