//! The Settings tab: how the overlay looks, and the server region.

use crate::app::Message;
use crate::views::events::{pill, region_pills};
use arclens_i18n::{Lang, t};
use arclens_ipc::{Corner, OverlaySettings};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length};

/// What the Settings tab shows.
pub struct SettingsView<'a> {
    pub overlay: OverlaySettings,
    pub region: Option<&'a str>,
    pub refresh_hours: u32,
    /// The language picked, `None`: the system's.
    pub language: Option<Lang>,
}

/// Refresh intervals offered, in hours.
const REFRESH: [u32; 4] = [6, 24, 72, 168];

fn refresh_label(hours: u32) -> String {
    match hours {
        24 => t!("settings-refresh-daily"),
        168 => t!("settings-refresh-weekly"),
        h if h % 24 == 0 => t!("settings-refresh-days", count = h / 24),
        h => t!("settings-refresh-hours", count = h),
    }
}

fn corner_label(corner: Corner) -> String {
    match corner {
        Corner::TopLeft => t!("corner-top-left"),
        Corner::TopRight => t!("corner-top-right"),
        Corner::BottomLeft => t!("corner-bottom-left"),
        Corner::BottomRight => t!("corner-bottom-right"),
    }
}

pub fn view<'a>(page: &SettingsView<'a>) -> Element<'a, Message> {
    let (overlay, region) = (page.overlay, page.region);
    let opacities = OverlaySettings::OPACITIES
        .iter()
        .fold(row![].spacing(8), |r, &opacity| {
            r.push(pill(
                &opacity_label(opacity),
                (overlay.opacity - opacity).abs() < 0.01,
                Message::SetOverlayOpacity(opacity),
            ))
        });
    let refresh = REFRESH
        .iter()
        .fold(row![].spacing(8), |r, &hours| {
            r.push(pill(
                &refresh_label(hours),
                page.refresh_hours == hours,
                Message::SetRefreshHours(hours),
            ))
        })
        .push(Space::new().width(12))
        .push(theme::secondary_button(
            &t!("settings-refresh-now"),
            Some(Message::RefreshData),
        ));
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
            &corner_label(corner),
            overlay.corner == corner,
            Message::SetOverlayCorner(corner),
        ))
    });
    let content = column![
        theme::heading(&t!("settings-title"), size::TITLE),
        section(
            &t!("settings-language"),
            t!("settings-language-help"),
            language_pills(page.language),
        ),
        section(
            &t!("settings-overlay-size"),
            t!("settings-overlay-size-help"),
            scales.wrap().vertical_spacing(8).into(),
        ),
        section(
            &t!("settings-pinned-card"),
            t!("settings-pinned-card-help"),
            row![
                container(corners.wrap().vertical_spacing(8)).width(Length::Fill),
                corner_preview(overlay.corner)
            ]
            .spacing(space::SECTION)
            .align_y(Alignment::Center)
            .into(),
        ),
        section(
            &t!("settings-overlay-background"),
            t!("settings-overlay-background-help"),
            opacities.wrap().vertical_spacing(8).into(),
        ),
        section(
            &t!("settings-game-data"),
            t!("settings-game-data-help"),
            refresh.wrap().vertical_spacing(8).into(),
        ),
        section(
            &t!("settings-region"),
            t!("settings-region-help"),
            region_pills(region),
        ),
        // The version, for bug reports.
        text(t!("settings-about", version = env!("CARGO_PKG_VERSION")))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
    ]
    .spacing(space::SECTION)
    .padding(theme::PAGE_PADDING)
    .max_width(900);
    scrollable(content).height(Length::Fill).into()
}

/// "System (English)", then every language.
fn language_pills<'a>(chosen: Option<Lang>) -> Element<'a, Message> {
    Lang::ALL
        .iter()
        .fold(
            row![pill(
                &t!(
                    "settings-language-system",
                    name = Lang::system().native_name()
                ),
                chosen.is_none(),
                Message::SetLanguage(None),
            )]
            .spacing(8),
            |r, &lang| {
                r.push(pill(
                    lang.native_name(),
                    chosen == Some(lang),
                    Message::SetLanguage(Some(lang)),
                ))
            },
        )
        .wrap()
        .vertical_spacing(8)
        .into()
}

/// One setting: a panel with the help line over the controls.
fn section<'a>(title: &str, help: String, body: Element<'a, Message>) -> Element<'a, Message> {
    theme::panel(
        title,
        None,
        column![text(help).size(size::BODY).color(palette::TEXT_MUTED), body,].spacing(space::GAP),
    )
}

/// "75 %" style label for one of [`OverlaySettings::OPACITIES`].
fn opacity_label(opacity: f32) -> String {
    if opacity >= 0.999 {
        t!("settings-opacity-solid")
    } else {
        percent(opacity).to_owned()
    }
}

/// "125 %" style label for one of [`OverlaySettings::SCALES`].
fn percent(scale: f32) -> &'static str {
    const LABELS: [(f32, &str); 7] = [
        (0.6, "60 %"),
        (0.75, "75 %"),
        (0.8, "80 %"),
        (0.9, "90 %"),
        (1.0, "100 %"),
        (1.25, "125 %"),
        (1.5, "150 %"),
    ];
    LABELS
        .iter()
        .find(|(s, _)| (s - scale).abs() < 0.01)
        .map_or("custom", |(_, label)| label)
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
        for scale in OverlaySettings::SCALES
            .into_iter()
            .chain(OverlaySettings::OPACITIES)
        {
            assert_ne!(percent(scale), "custom");
        }
    }
}
