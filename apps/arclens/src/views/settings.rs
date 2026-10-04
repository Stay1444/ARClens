//! The Settings tab: how the overlay looks, and the server region.

use crate::app::Message;
use crate::views::events::{pill, region_pills};
use arclens_ipc::{Corner, OverlaySettings};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length};

pub fn view(overlay: OverlaySettings, region: Option<&str>) -> Element<'_, Message> {
    let scales = OverlaySettings::SCALES
        .iter()
        .fold(row![].spacing(8), |r, &scale| {
            r.push(pill(
                percent(scale),
                (overlay.scale - scale).abs() < 0.01,
                Message::SetOverlayScale(scale),
            ))
        });
    let corners = Corner::ALL.iter().fold(row![].spacing(8), |r, &corner| {
        r.push(pill(
            corner.label(),
            overlay.corner == corner,
            Message::SetOverlayCorner(corner),
        ))
    });
    let content = column![
        theme::heading("Settings", size::TITLE),
        section(
            "Overlay size",
            "Scales everything the overlay draws: cards, the map panel, markers and \
             the quick search.",
            scales.wrap().vertical_spacing(8).into(),
        ),
        section(
            "Pinned item card",
            "Where the card of the item you pick (in the app or the overlay's quick \
             search) sits while the overlay is shown.",
            row![
                container(corners.wrap().vertical_spacing(8)).width(Length::Fill),
                corner_preview(overlay.corner)
            ]
            .spacing(space::SECTION)
            .align_y(Alignment::Center)
            .into(),
        ),
        section(
            "Server region",
            "Map condition times differ per region.",
            region_pills(region),
        ),
    ]
    .spacing(space::SECTION)
    .padding(theme::PAGE_PADDING)
    .max_width(900);
    scrollable(content).height(Length::Fill).into()
}

/// One setting: a panel with the help line over the controls.
fn section<'a>(title: &'a str, help: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    theme::panel(
        title,
        None,
        column![text(help).size(size::BODY).color(palette::TEXT_MUTED), body,].spacing(space::GAP),
    )
}

/// "125 %" style label for one of [`OverlaySettings::SCALES`].
fn percent(scale: f32) -> &'static str {
    const LABELS: [&str; 5] = ["80 %", "90 %", "100 %", "125 %", "150 %"];
    OverlaySettings::SCALES
        .iter()
        .position(|&s| (s - scale).abs() < 0.01)
        .map_or("custom", |i| LABELS[i])
}

/// A small screen with the card in the chosen corner: a cream card with
/// a yellow header line, like the overlay's.
fn corner_preview<'a>(corner: Corner) -> Element<'a, Message> {
    let card = column![
        container(Space::new().width(30).height(6)).style(|_| container::Style {
            background: Some(theme::ACCENT.into()),
            border: Border {
                radius: iced::border::Radius::default().top(3.0),
                ..Border::default()
            },
            ..container::Style::default()
        }),
        container(Space::new().width(30).height(30)).style(|_| container::Style {
            background: Some(theme::CREAM.into()),
            border: Border {
                radius: iced::border::Radius::default().bottom(3.0),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    ];
    let screen = container(card)
        .width(176)
        .height(99)
        .padding(8)
        .style(|_| container::Style {
            background: Some(theme::BG.into()),
            border: Border {
                color: with_alpha(theme::CREAM, 0.35),
                width: 1.0,
                radius: theme::RADIUS.into(),
            },
            ..container::Style::default()
        });
    screen
        .align_x(if corner.is_left() {
            Alignment::Start
        } else {
            Alignment::End
        })
        .align_y(if corner.is_top() {
            Alignment::Start
        } else {
            Alignment::End
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_scale_has_a_label() {
        for scale in OverlaySettings::SCALES {
            assert_ne!(percent(scale), "custom");
        }
    }
}
