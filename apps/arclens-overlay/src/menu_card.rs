//! The card on the game's main menu, under the game's Quests box (left
//! column), styled after it: a cream header with the page's title and
//! dots, a dark body. Its pages (conditions now, next, the player's
//! progress) take turns every few seconds with a short slide and fade.
//! Countdowns run on the overlay's own clock.

use crate::Message;
use arclens_i18n::t;
use arclens_ipc::{CardEvent, MenuCard, ProgressLine};
use arclens_ui::names;
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, column, container, image, progress_bar, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Size, font};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

/// Where the card goes, as fractions of the screen: left column, under
/// the Quests box (measured at 2560×1440 and 2000×1125).
const AREA: [f32; 3] = [0.025, 0.655, 0.208];
/// Conditions per page.
const SHOWN: usize = 3;
/// How long each page shows, and how long it takes to come in.
const PAGE_MS: i64 = 8_000;
const FADE_MS: i64 = 350;

/// The game's header cream and the dark ink on it.
const CREAM: Color = Color::from_rgb(0.94, 0.91, 0.85);
const INK: Color = Color::from_rgb(0.08, 0.08, 0.10);
const NOW: Color = Color::from_rgb(0.30, 0.82, 0.50);
const NEXT: Color = Color::from_rgb(0.36, 0.62, 0.98);

/// Decoded condition icons by file, so `view` never decodes.
pub type Icons = HashMap<PathBuf, image::Handle>;

/// Decodes the icons of `card` not decoded yet.
pub fn load_icons(card: &MenuCard, icons: &mut Icons) {
    for path in card
        .active
        .iter()
        .chain(&card.upcoming)
        .filter_map(|e| e.icon.as_ref())
    {
        if !icons.contains_key(path)
            && let Some(handle) = arclens_ui::decode_icon(path, 64)
        {
            icons.insert(path.clone(), handle);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Now,
    Next,
    Progress,
    Controls,
}

impl Page {
    fn title(self) -> String {
        match self {
            Self::Now => t!("overlay-card-now"),
            Self::Next => t!("overlay-card-next"),
            Self::Progress => t!("overlay-card-progress"),
            Self::Controls => t!("overlay-card-controls"),
        }
        .to_uppercase()
    }
}

fn live(events: &[CardEvent], now_ms: i64) -> impl Iterator<Item = &CardEvent> {
    events.iter().filter(move |e| e.at_ms > now_ms).take(SHOWN)
}

/// The pages with something to show (at least one).
fn pages(card: &MenuCard, now_ms: i64) -> Vec<Page> {
    let mut pages = Vec::new();
    if live(&card.active, now_ms).next().is_some() {
        pages.push(Page::Now);
    }
    if live(&card.upcoming, now_ms).next().is_some() {
        pages.push(Page::Next);
    }
    if !card.progress.is_empty() {
        pages.push(Page::Progress);
    }
    // Always something to show.
    pages.push(Page::Controls);
    pages
}

/// Which page shows at `now_ms`, and how far it has come in (0 → 1).
fn current(count: usize, now_ms: i64) -> (usize, f32) {
    let count = i64::try_from(count.max(1)).unwrap_or(1);
    let index = usize::try_from((now_ms / PAGE_MS).rem_euclid(count)).unwrap_or(0);
    #[allow(clippy::cast_precision_loss, reason = "milliseconds within a page")]
    let shown = (now_ms.rem_euclid(PAGE_MS) as f32 / FADE_MS as f32).min(1.0);
    (index, shown)
}

/// How often the card needs a redraw: smoothly around a page change,
/// once a second otherwise (the countdowns).
pub fn tick_interval(now_ms: i64) -> Duration {
    let into_page = now_ms.rem_euclid(PAGE_MS);
    if into_page < FADE_MS || PAGE_MS - into_page < 1_100 {
        Duration::from_millis(33)
    } else {
        Duration::from_secs(1)
    }
}

/// `1h 05m` / `12m` / `45s`.
fn countdown(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

/// Smooth start and stop.
fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn view<'a>(
    card: &'a MenuCard,
    icons: &'a Icons,
    now_ms: i64,
    screen: Size,
) -> Element<'a, Message> {
    let [x, y, w] = AREA;
    let pages = pages(card, now_ms);
    let (index, shown) = current(pages.len(), now_ms);
    let page = pages[index];
    let alpha = ease(shown);

    let dots = pages
        .iter()
        .enumerate()
        .fold(row![].spacing(5), |r, (i, _)| {
            let on = i == index;
            r.push(
                container(Space::new().width(7).height(7)).style(move |_| container::Style {
                    background: Some(with_alpha(INK, if on { 0.9 } else { 0.25 }).into()),
                    border: Border {
                        radius: 3.5.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
            )
        });
    let header = container(
        row![
            text("ARCLENS")
                .size(11)
                .font(BOLD)
                .color(with_alpha(INK, 0.55)),
            text(page.title()).size(15).font(BOLD).color(INK),
            Space::new().width(Length::Fill),
            dots,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([7, 12])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(CREAM.into()),
        border: Border {
            radius: iced::border::Radius::default().top(6.0),
            ..Border::default()
        },
        ..container::Style::default()
    });

    let body: Element<'a, Message> = match page {
        Page::Now => conditions(
            &card.active,
            icons,
            now_ms,
            &t!("overlay-card-ends-in"),
            NOW,
            alpha,
        ),
        Page::Next => conditions(
            &card.upcoming,
            icons,
            now_ms,
            &t!("overlay-card-starts-in"),
            NEXT,
            alpha,
        ),
        Page::Progress => progress(&card.progress, alpha),
        Page::Controls => controls(alpha),
    };
    // Slides in from the right as it fades in.
    let body = container(body)
        .padding(iced::Padding {
            top: 10.0,
            bottom: 12.0,
            left: 12.0 + (1.0 - alpha) * 28.0,
            right: 12.0,
        })
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(
                with_alpha(palette::SURFACE, arclens_ui::theme::surface_alpha(0.94)).into(),
            ),
            border: Border {
                radius: iced::border::Radius::default().bottom(6.0),
                ..Border::default()
            },
            ..container::Style::default()
        });

    container(column![header, body].width(w * screen.width))
        .padding(iced::Padding {
            top: y * screen.height,
            left: x * screen.width,
            ..iced::Padding::ZERO
        })
        .into()
}

fn conditions<'a>(
    events: &'a [CardEvent],
    icons: &'a Icons,
    now_ms: i64,
    when: &str,
    accent: Color,
    alpha: f32,
) -> Element<'a, Message> {
    let mut list = column![].spacing(10);
    let mut any = false;
    for event in live(events, now_ms) {
        any = true;
        let icon: Element<'a, Message> = match event.icon.as_ref().and_then(|p| icons.get(p)) {
            Some(handle) => image(handle.clone())
                .width(36)
                .height(36)
                .opacity(alpha)
                .into(),
            None => initials(&names::condition(&event.name), accent, alpha),
        };
        list = list.push(
            row![
                icon,
                column![
                    text(names::condition(&event.name))
                        .size(16)
                        .font(BOLD)
                        .color(with_alpha(palette::TEXT, alpha)),
                    text(names::map(&event.map))
                        .size(12)
                        .color(with_alpha(palette::TEXT_MUTED, alpha)),
                ]
                .spacing(1)
                .width(Length::Fill),
                column![
                    text(countdown(event.at_ms - now_ms))
                        .size(18)
                        .font(BOLD)
                        .color(with_alpha(accent, alpha)),
                    text(when.to_owned())
                        .size(10)
                        .color(with_alpha(palette::TEXT_MUTED, alpha)),
                ]
                .align_x(Alignment::End),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    if !any {
        list = list.push(
            text(t!("overlay-card-no-schedule"))
                .size(13)
                .color(with_alpha(palette::TEXT_MUTED, alpha)),
        );
    }
    list.into()
}

/// A round badge with the condition's first letter, while its icon loads.
fn initials<'a>(name: &str, accent: Color, alpha: f32) -> Element<'a, Message> {
    let letter: String = name.chars().take(1).collect();
    container(
        text(letter)
            .size(16)
            .font(BOLD)
            .color(with_alpha(palette::TEXT, alpha)),
    )
    .center(36)
    .style(move |_| container::Style {
        background: Some(with_alpha(accent, 0.35 * alpha).into()),
        border: Border {
            radius: 18.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// The keys and what they do; the default bindings (the desktop may let
/// the player change them).
fn controls<'a>(alpha: f32) -> Element<'a, Message> {
    let keys = [
        ("Ctrl+Shift+I".to_owned(), t!("overlay-controls-search")),
        ("Ctrl+Shift+O".to_owned(), t!("overlay-controls-toggle")),
        ("Esc".to_owned(), t!("overlay-controls-esc")),
        (
            t!("overlay-controls-hover-key"),
            t!("overlay-controls-hover"),
        ),
        (t!("overlay-controls-map-key"), t!("overlay-controls-map")),
    ];
    keys.into_iter()
        .fold(column![].spacing(7), |col, (key, action)| {
            col.push(
                row![
                    container(text(key).size(12).font(BOLD).color(with_alpha(INK, alpha)),)
                        .padding([1, 6])
                        .width(118)
                        .style(move |_| container::Style {
                            background: Some(with_alpha(CREAM, 0.9 * alpha).into()),
                            border: Border {
                                radius: 3.0.into(),
                                ..Border::default()
                            },
                            ..container::Style::default()
                        }),
                    text(action)
                        .size(13)
                        .color(with_alpha(palette::TEXT, alpha))
                        .width(Length::Fill),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
        })
        .into()
}

fn progress(lines: &[ProgressLine], alpha: f32) -> Element<'_, Message> {
    lines
        .iter()
        .fold(column![].spacing(10), |col, line| {
            #[allow(clippy::cast_precision_loss, reason = "small counts")]
            let share = if line.total == 0 {
                0.0
            } else {
                line.done as f32 / line.total as f32
            };
            col.push(
                column![
                    row![
                        text(&line.label)
                            .size(14)
                            .font(BOLD)
                            .color(with_alpha(palette::TEXT, alpha))
                            .width(Length::Fill),
                        text(format!("{} / {}", line.done, line.total))
                            .size(13)
                            .color(with_alpha(palette::TEXT_MUTED, alpha)),
                    ],
                    progress_bar(0.0..=1.0, share).girth(5).style(move |_| {
                        progress_bar::Style {
                            background: with_alpha(palette::TEXT, 0.08 * alpha).into(),
                            bar: with_alpha(NOW, alpha).into(),
                            border: Border {
                                radius: 2.5.into(),
                                ..Border::default()
                            },
                        }
                    }),
                ]
                .spacing(4),
            )
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_in_readable_units() {
        assert_eq!(countdown(3_900_000), "1h 05m");
        assert_eq!(countdown(12 * 60_000 + 5_000), "12m");
        assert_eq!(countdown(45_000), "45s");
        assert_eq!(countdown(-5), "0s");
    }

    fn event(at_ms: i64) -> CardEvent {
        CardEvent {
            name: "Matriarch".into(),
            map: "Blue Gate".into(),
            at_ms,
            icon: None,
        }
    }

    #[test]
    fn pages_take_turns_and_skip_empty_ones() {
        let mut card = MenuCard {
            active: vec![event(100_000)],
            ..MenuCard::default()
        };
        assert_eq!(pages(&card, 0), [Page::Now, Page::Controls]);
        card.upcoming = vec![event(200_000)];
        card.progress = vec![ProgressLine {
            label: "Workshop".into(),
            done: 3,
            total: 33,
        }];
        assert_eq!(
            pages(&card, 0),
            [Page::Now, Page::Next, Page::Progress, Page::Controls]
        );
        // Once the running condition ended, its page goes.
        assert_eq!(
            pages(&card, 150_000),
            [Page::Next, Page::Progress, Page::Controls]
        );
        // With nothing else, the controls still show.
        assert_eq!(pages(&MenuCard::default(), 0), [Page::Controls]);

        assert_eq!(current(3, 0), (0, 0.0));
        assert_eq!(current(3, PAGE_MS + FADE_MS), (1, 1.0));
        assert_eq!(current(3, 3 * PAGE_MS + 10).0, 0);
    }

    #[test]
    fn ticks_fast_only_around_page_changes() {
        assert_eq!(tick_interval(PAGE_MS * 5 + 100), Duration::from_millis(33));
        assert_eq!(tick_interval(PAGE_MS * 5 + 4_000), Duration::from_secs(1));
        assert_eq!(tick_interval(PAGE_MS * 6 - 500), Duration::from_millis(33));
    }
}
