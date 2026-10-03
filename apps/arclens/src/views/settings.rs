//! The Settings tab: how the overlay looks, and the server region.

use crate::app::Message;
use crate::views::events::{pill, region_pills};
use arclens_ipc::{Corner, OverlaySettings};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Font, Length, font};

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

pub fn view(overlay: OverlaySettings, region: Option<&str>) -> Element<'_, Message> {
    let scales = OverlaySettings::SCALES
        .iter()
        .fold(row![].spacing(6), |r, &scale| {
            r.push(pill(
                percent(scale),
                (overlay.scale - scale).abs() < 0.01,
                Message::SetOverlayScale(scale),
            ))
        });
    let corners = Corner::ALL.iter().fold(row![].spacing(6), |r, &corner| {
        r.push(pill(
            corner.label(),
            overlay.corner == corner,
            Message::SetOverlayCorner(corner),
        ))
    });
    let content = column![
        text("Settings").size(24).font(BOLD),
        section(
            "Overlay size",
            "Scales everything the overlay draws: cards, the map panel, markers and \
             the quick search.",
            scales.into(),
        ),
        section(
            "Pinned item card",
            "Where the card of the item you pick (in the app or the overlay's quick \
             search) sits while the overlay is shown.",
            row![corners, corner_preview(overlay.corner)]
                .spacing(24)
                .align_y(Alignment::Center)
                .into(),
        ),
        section(
            "Server region",
            "Map condition times differ per region.",
            region_pills(region),
        ),
    ]
    .spacing(28)
    .padding(24)
    .max_width(900);
    scrollable(content).height(Length::Fill).into()
}

fn section<'a>(title: &'a str, help: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    column![
        text(title).size(16).font(BOLD),
        text(help).size(13).color(palette::TEXT_MUTED),
        body,
    ]
    .spacing(8)
    .into()
}

/// "125 %" style label for one of [`OverlaySettings::SCALES`].
fn percent(scale: f32) -> &'static str {
    const LABELS: [&str; 5] = ["80 %", "90 %", "100 %", "125 %", "150 %"];
    OverlaySettings::SCALES
        .iter()
        .position(|&s| (s - scale).abs() < 0.01)
        .map_or("custom", |i| LABELS[i])
}

/// A tiny screen with the card in the chosen corner.
fn corner_preview<'a>(corner: Corner) -> Element<'a, Message> {
    let card = container(Space::new().width(22).height(28)).style(|_| container::Style {
        background: Some(with_alpha(palette::COIN, 0.8).into()),
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let screen = container(card)
        .width(128)
        .height(72)
        .padding(6)
        .style(|_| container::Style {
            background: Some(with_alpha(palette::TEXT, 0.05).into()),
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: 4.0.into(),
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
