//! The card on the game's main menu: map conditions now and next, and the
//! workshop progress. It sits in the free space under the game's Quests
//! box (left column), and counts down on the overlay's own clock.

use crate::Message;
use arclens_ipc::{CardEvent, MenuCard};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{column, container, row, text};
use iced::{Border, Element, Font, Length, Size, font};

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

/// Where the card goes, as fractions of the screen: left column, under
/// the Quests box (measured at 2560×1440; **unverified** elsewhere).
const AREA: [f32; 3] = [0.025, 0.655, 0.208];
/// Conditions listed per group.
const SHOWN: usize = 3;

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

fn rows<'a>(
    events: &[CardEvent],
    now_ms: i64,
    label: &'a str,
    accent: iced::Color,
) -> Element<'a, Message> {
    let live: Vec<&CardEvent> = events
        .iter()
        .filter(|e| e.at_ms > now_ms)
        .take(SHOWN)
        .collect();
    let mut col = column![text(label).size(10).color(palette::TEXT_MUTED)].spacing(4);
    if live.is_empty() {
        col = col.push(text("—").size(12).color(palette::TEXT_MUTED));
    }
    for event in live {
        col = col.push(
            row![
                column![
                    text(event.name.clone()).size(13).font(BOLD),
                    text(event.map.clone()).size(11).color(palette::TEXT_MUTED),
                ]
                .width(Length::Fill),
                text(countdown(event.at_ms - now_ms)).size(12).color(accent),
            ]
            .align_y(iced::Alignment::Center),
        );
    }
    col.into()
}

pub fn view(card: &MenuCard, now_ms: i64, screen: Size) -> Element<'_, Message> {
    let [x, y, w] = AREA;
    let mut body = column![
        row![
            text("ARClens").size(12).font(BOLD).width(Length::Fill),
            text("Map conditions").size(10).color(palette::TEXT_MUTED),
        ],
        rows(
            &card.active,
            now_ms,
            "NOW · ENDS IN",
            iced::Color::from_rgb(0.30, 0.82, 0.50)
        ),
        rows(
            &card.upcoming,
            now_ms,
            "NEXT · STARTS IN",
            iced::Color::from_rgb(0.36, 0.62, 0.98)
        ),
    ]
    .spacing(10);
    if let Some((built, total)) = card.workshop {
        body = body.push(
            text(format!("Workshop: {built} of {total} levels built"))
                .size(11)
                .color(palette::TEXT_MUTED),
        );
    }
    let card = container(body)
        .padding(12)
        .width(w * screen.width)
        .style(|_| container::Style {
            background: Some(with_alpha(palette::SURFACE, 0.92).into()),
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });
    container(card)
        .padding(iced::Padding {
            top: y * screen.height,
            left: x * screen.width,
            ..iced::Padding::ZERO
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
}
