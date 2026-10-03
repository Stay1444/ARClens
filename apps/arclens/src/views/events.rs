//! The Events tab: which map conditions run now, what's next, and the full
//! schedule per condition, in local time.

use crate::app::Message;
use crate::event_icons::EventIcons;
use arclens_core::{ScheduledEvent, agenda, countdown};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Color, Element, Font, Length, font};
use std::collections::BTreeMap;

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};
const ACTIVE: Color = Color::from_rgb(0.30, 0.82, 0.50);
const UPCOMING: Color = Color::from_rgb(0.36, 0.62, 0.98);
const CARD_WIDTH: f32 = 230.0;
/// Backgrounds for conditions without an icon.
const COLORS: [Color; 5] = [
    Color::from_rgb(0.55, 0.36, 0.85),
    Color::from_rgb(0.20, 0.55, 0.75),
    Color::from_rgb(0.80, 0.45, 0.25),
    Color::from_rgb(0.25, 0.60, 0.45),
    Color::from_rgb(0.75, 0.30, 0.45),
];

/// Instances listed per condition in the schedule.
const PER_CONDITION: usize = 4;

pub fn view<'a>(
    events: &'a [ScheduledEvent],
    icons: &'a EventIcons,
    now_ms: i64,
    map_filter: Option<&'a str>,
) -> Element<'a, Message> {
    let agenda = agenda(
        events
            .iter()
            .filter(|e| map_filter.is_none_or(|m| e.map == m)),
        now_ms,
    );

    let active = agenda.active.iter().fold(row![].spacing(10), |r, e| {
        r.push(card(
            e,
            icons,
            format!("Ends in {}", countdown(e.end_ms - now_ms)),
            ACTIVE,
        ))
    });

    // The next occurrence on each map.
    let mut next_per_map: BTreeMap<&str, &ScheduledEvent> = BTreeMap::new();
    for e in &agenda.upcoming {
        next_per_map.entry(e.map.as_str()).or_insert(e);
    }
    let mut next: Vec<_> = next_per_map.into_values().collect();
    next.sort_by_key(|e| e.start_ms);
    let upcoming = next.iter().fold(row![].spacing(10), |r, e| {
        r.push(card(
            e,
            icons,
            format!("Starts in {}", countdown(e.start_ms - now_ms)),
            UPCOMING,
        ))
    });

    // Full schedule grouped by condition.
    let mut by_condition: BTreeMap<&str, Vec<&ScheduledEvent>> = BTreeMap::new();
    for e in agenda.active.iter().chain(&agenda.upcoming) {
        by_condition.entry(e.name.as_str()).or_default().push(e);
    }
    let schedule = by_condition
        .into_iter()
        .fold(row![].spacing(12), |r, (name, list)| {
            r.push(schedule_card(name, &list, icons, now_ms))
        });

    let content = column![
        text("Events").size(24).font(BOLD),
        map_pills(events, map_filter),
        section(
            "ACTIVE NOW",
            active.wrap().vertical_spacing(10).into(),
            agenda.active.is_empty()
        ),
        section(
            "UPCOMING NEXT",
            upcoming.wrap().vertical_spacing(10).into(),
            next.is_empty()
        ),
        section(
            "SCHEDULE",
            schedule.wrap().vertical_spacing(12).into(),
            agenda.upcoming.is_empty() && agenda.active.is_empty()
        ),
        text(
            "Times in your local time zone. Some sites shift the rotation per server \
             region; if times look off for you, tell us."
        )
        .size(11)
        .color(palette::TEXT_MUTED),
    ]
    .spacing(18)
    .padding(24);
    scrollable(content).height(Length::Fill).into()
}

fn section<'a>(title: &'a str, body: Element<'a, Message>, empty: bool) -> Element<'a, Message> {
    let body = if empty {
        text("Nothing here right now.")
            .size(13)
            .color(palette::TEXT_MUTED)
            .into()
    } else {
        body
    };
    column![text(title).size(11).color(palette::TEXT_MUTED), body]
        .spacing(8)
        .into()
}

fn map_pills<'a>(events: &'a [ScheduledEvent], selected: Option<&'a str>) -> Element<'a, Message> {
    let mut maps: Vec<&str> = events.iter().map(|e| e.map.as_str()).collect();
    maps.sort_unstable();
    maps.dedup();
    let all = pill(
        "All maps",
        selected.is_none(),
        Message::FilterEventsMap(None),
    );
    maps.into_iter()
        .fold(row![all].spacing(6), |r, map| {
            r.push(pill(
                map,
                selected == Some(map),
                Message::FilterEventsMap(Some(map.to_owned())),
            ))
        })
        .wrap()
        .into()
}

fn pill(label: &str, active: bool, on_press: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding([5, 12])
        .on_press(on_press)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let alpha = match (active, hovered) {
                (true, _) => 0.18,
                (false, true) => 0.10,
                (false, false) => 0.04,
            };
            button::Style {
                background: Some(with_alpha(palette::TEXT, alpha).into()),
                text_color: if active {
                    palette::TEXT
                } else {
                    palette::TEXT_MUTED
                },
                border: Border {
                    color: palette::BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn card<'a>(
    event: &ScheduledEvent,
    icons: &'a EventIcons,
    when: String,
    accent: Color,
) -> Element<'a, Message> {
    container(
        row![
            event_icon(&event.name, icons, 40.0),
            column![
                text(event.name.clone()).size(15).font(BOLD),
                text(event.map.clone()).size(12).color(palette::TEXT_MUTED),
                text(when).size(12).color(accent),
            ]
            .spacing(2),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([10, 12])
    .width(CARD_WIDTH)
    .style(move |_| tile(accent))
    .into()
}

/// The condition's icon, or its initials on a colour derived from the name
/// while the icon loads or when there is none.
fn event_icon<'a>(name: &str, icons: &'a EventIcons, size: f32) -> Element<'a, Message> {
    if let Some(handle) = icons.get(name) {
        return iced::widget::image(handle.clone())
            .width(size)
            .height(size)
            .into();
    }
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let hue = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let color = COLORS[hue as usize % COLORS.len()];
    container(text(initials).size(size * 0.38).font(BOLD))
        .center_x(size)
        .center_y(size)
        .style(move |_| container::Style {
            background: Some(with_alpha(color, 0.85).into()),
            text_color: Some(Color::WHITE),
            border: Border {
                radius: (size / 4.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn schedule_card<'a>(
    name: &str,
    list: &[&ScheduledEvent],
    icons: &'a EventIcons,
    now_ms: i64,
) -> Element<'a, Message> {
    let mut maps: Vec<&str> = list.iter().map(|e| e.map.as_str()).collect();
    maps.sort_unstable();
    maps.dedup();

    let rows = list
        .iter()
        .take(PER_CONDITION)
        .fold(column![].spacing(6), |col, e| {
            let (when, accent) = if e.is_active(now_ms) {
                (format!("ends in {}", countdown(e.end_ms - now_ms)), ACTIVE)
            } else {
                (countdown(e.start_ms - now_ms), UPCOMING)
            };
            col.push(
                row![
                    column![
                        text(format!(
                            "{} – {}",
                            local_time(e.start_ms),
                            local_time(e.end_ms)
                        ))
                        .size(13),
                    ]
                    // The map only when the condition runs on several.
                    .push(
                        (maps.len() > 1)
                            .then(|| { text(e.map.clone()).size(11).color(palette::TEXT_MUTED) })
                    )
                    .width(Length::Fill),
                    text(when).size(12).color(accent),
                ]
                .align_y(Alignment::Center),
            )
        });

    container(
        column![
            row![
                event_icon(name, icons, 32.0),
                column![
                    text(name.to_owned()).size(16).font(BOLD),
                    text(maps.join(", ")).size(11).color(palette::TEXT_MUTED),
                ],
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            Space::new().height(4),
            rows,
        ]
        .spacing(2),
    )
    .padding(14)
    .width(300)
    .style(|_| tile(palette::BORDER))
    .into()
}

fn tile(accent: Color) -> container::Style {
    container::Style {
        background: Some(palette::SURFACE.into()),
        border: Border {
            color: with_alpha(accent, 0.45),
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Some(palette::TEXT),
        ..container::Style::default()
    }
}

/// "19:00", or "Sat 19:00" when not today, in the system time zone.
fn local_time(ms: i64) -> String {
    let tz = jiff::tz::TimeZone::system();
    let Ok(at) = jiff::Timestamp::from_millisecond(ms) else {
        return "?".into();
    };
    let at = at.to_zoned(tz.clone());
    let today = jiff::Timestamp::now().to_zoned(tz).date();
    if at.date() == today {
        at.strftime("%H:%M").to_string()
    } else {
        at.strftime("%a %H:%M").to_string()
    }
}
