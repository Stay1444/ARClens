//! The Events tab: which map conditions run now, what's next, and the full
//! schedule per condition, in local time.

use crate::app::Message;
use crate::event_icons::EventIcons;
use arclens_core::{ScheduledEvent, agenda, countdown};
use arclens_i18n::t;
use arclens_ui::names;
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Border, Color, Element, Length};
use std::collections::BTreeMap;

pub(crate) const ACTIVE: Color = Color::from_rgb(0.30, 0.82, 0.50);
pub(crate) const UPCOMING: Color = Color::from_rgb(0.36, 0.62, 0.98);
const CARD_WIDTH: f32 = 268.0;
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

pub struct EventsView<'a> {
    pub events: &'a [ScheduledEvent],
    pub icons: &'a EventIcons,
    pub now_ms: i64,
    pub map_filter: Option<&'a str>,
    /// The region the player chose (`None`: not asked yet).
    pub region: Option<&'a str>,
    /// The region the schedule says it is for.
    pub served_region: Option<&'a str>,
}

pub fn view<'a>(page: &EventsView<'a>) -> Element<'a, Message> {
    let (events, icons, now_ms, map_filter) =
        (page.events, page.icons, page.now_ms, page.map_filter);
    let agenda = agenda(
        events
            .iter()
            .filter(|e| map_filter.is_none_or(|m| e.map == m)),
        now_ms,
    );

    let active = agenda
        .active
        .iter()
        .fold(row![].spacing(space::GAP), |r, e| {
            r.push(card(
                e,
                icons,
                t!("events-ends-in", time = countdown(e.end_ms - now_ms)),
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
    let upcoming = next.iter().fold(row![].spacing(space::GAP), |r, e| {
        r.push(card(
            e,
            icons,
            t!("events-starts-in", time = countdown(e.start_ms - now_ms)),
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
        .fold(row![].spacing(space::GAP), |r, (name, list)| {
            r.push(schedule_card(name, &list, icons, now_ms))
        });

    let content = column![
        row![
            container(theme::heading(&t!("events-title"), size::TITLE)).width(Length::Fill),
            region_pills(page.region),
        ]
        .align_y(Alignment::Center),
        region_notice(page.region, page.served_region),
        map_pills(events, map_filter),
        section(
            &t!("events-active-now"),
            active.wrap().vertical_spacing(space::GAP).into(),
            agenda.active.is_empty()
        ),
        section(
            &t!("events-next-per-map"),
            upcoming.wrap().vertical_spacing(space::GAP).into(),
            next.is_empty()
        ),
        section(
            &t!("events-schedule"),
            schedule.wrap().vertical_spacing(space::GAP).into(),
            agenda.upcoming.is_empty() && agenda.active.is_empty()
        ),
        text(t!("events-footnote"))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
    ]
    .spacing(space::SECTION)
    .padding(theme::PAGE_PADDING)
    .max_width(1400);
    scrollable(content).height(Length::Fill).into()
}

/// The player's server region; times differ per region.
pub(crate) fn region_pills<'a>(chosen: Option<&str>) -> Element<'a, Message> {
    arclens_data::metaforge::REGIONS
        .iter()
        .fold(row![].spacing(6), |r, &(id, _)| {
            r.push(pill(
                &names::region(id),
                chosen == Some(id),
                Message::SetRegion(id.to_owned()),
            ))
        })
        .into()
}

/// First launch: ask for the region. Later: say so if the schedule came
/// back for another region than chosen.
fn region_notice<'a>(chosen: Option<&str>, served: Option<&str>) -> Element<'a, Message> {
    let note = match (chosen, served) {
        (None, served) => t!(
            "events-pick-region",
            showing = served.map_or_else(
                || t!("events-default-schedule"),
                |id| t!("events-region-schedule", region = names::region(id))
            )
        ),
        (Some(chosen), Some(served)) if chosen != served => t!(
            "events-other-region",
            served = names::region(served),
            chosen = names::region(chosen)
        ),
        _ => return Space::new().into(),
    };
    container(text(note).size(size::SMALL))
        .padding([8, 12])
        .style(|_| container::Style {
            background: Some(with_alpha(UPCOMING, 0.12).into()),
            border: Border {
                color: with_alpha(UPCOMING, 0.5),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn section<'a>(title: &str, body: Element<'a, Message>, empty: bool) -> Element<'a, Message> {
    let body = if empty {
        text(t!("events-nothing"))
            .size(size::BODY)
            .color(palette::TEXT_MUTED)
            .into()
    } else {
        body
    };
    theme::panel(title, None, body)
}

fn map_pills<'a>(events: &'a [ScheduledEvent], selected: Option<&'a str>) -> Element<'a, Message> {
    let mut maps: Vec<&str> = events.iter().map(|e| e.map.as_str()).collect();
    maps.sort_unstable();
    maps.dedup();
    let all = pill(
        &t!("events-all-maps"),
        selected.is_none(),
        Message::FilterEventsMap(None),
    );
    maps.into_iter()
        .fold(row![all].spacing(6), |r, map| {
            r.push(pill(
                &names::map(map),
                selected == Some(map),
                Message::FilterEventsMap(Some(map.to_owned())),
            ))
        })
        .wrap()
        .into()
}

pub(crate) fn pill<'a>(label: &str, active: bool, on_press: Message) -> Element<'a, Message> {
    theme::chip(label, active, on_press)
}

pub(crate) fn card<'a>(
    event: &ScheduledEvent,
    icons: &'a EventIcons,
    when: String,
    accent: Color,
) -> Element<'a, Message> {
    container(
        row![
            event_icon(&event.name, icons, 48.0),
            column![
                text(names::condition(&event.name))
                    .size(size::BODY + 1.0)
                    .font(theme::STRONG),
                text(names::map(&event.map))
                    .size(size::SMALL)
                    .color(palette::TEXT_MUTED),
                text(when)
                    .size(size::SMALL)
                    .font(theme::STRONG)
                    .color(accent),
            ]
            .spacing(1),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([12, 14])
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
    let initials: String = names::condition(name)
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let hue = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let color = COLORS[hue as usize % COLORS.len()];
    container(text(initials).size(size * 0.38).font(theme::DISPLAY))
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
    let mut maps: Vec<String> = list.iter().map(|e| names::map(&e.map)).collect();
    maps.sort_unstable();
    maps.dedup();

    let rows = list
        .iter()
        .take(PER_CONDITION)
        .fold(column![].spacing(6), |col, e| {
            let (when, accent) = if e.is_active(now_ms) {
                (
                    t!("events-ends-in-short", time = countdown(e.end_ms - now_ms)),
                    ACTIVE,
                )
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
                        .size(size::SMALL),
                    ]
                    // The map only when the condition runs on several.
                    .push((maps.len() > 1).then(|| {
                        text(names::map(&e.map))
                            .size(size::TINY)
                            .color(palette::TEXT_MUTED)
                    }))
                    .width(Length::Fill),
                    text(when)
                        .size(size::SMALL)
                        .font(theme::STRONG)
                        .color(accent),
                ]
                .align_y(Alignment::Center),
            )
        });

    container(
        column![
            row![
                event_icon(name, icons, 40.0),
                column![
                    text(names::condition(name))
                        .size(size::H2)
                        .font(theme::DISPLAY),
                    text(maps.join(", "))
                        .size(size::TINY)
                        .color(palette::TEXT_MUTED),
                ],
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            Space::new().height(4),
            rows,
        ]
        .spacing(2),
    )
    .padding(space::PANEL)
    .width(320)
    .style(|_| tile(palette::BORDER))
    .into()
}

pub(crate) fn tile(accent: Color) -> container::Style {
    container::Style {
        background: Some(theme::PANEL_RAISED.into()),
        border: Border {
            color: with_alpha(accent, 0.55),
            width: 1.0,
            radius: theme::RADIUS.into(),
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
        format!("{} {}", weekday(at.weekday()), at.strftime("%H:%M"))
    }
}

/// Short weekday name in the interface language.
fn weekday(day: jiff::civil::Weekday) -> String {
    use jiff::civil::Weekday;
    match day {
        Weekday::Monday => t!("weekday-mon"),
        Weekday::Tuesday => t!("weekday-tue"),
        Weekday::Wednesday => t!("weekday-wed"),
        Weekday::Thursday => t!("weekday-thu"),
        Weekday::Friday => t!("weekday-fri"),
        Weekday::Saturday => t!("weekday-sat"),
        Weekday::Sunday => t!("weekday-sun"),
    }
}
