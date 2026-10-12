//! A short notice at the top of the screen (a followed map condition about
//! to start), in the cards' style: the cream header, the game's colour
//! stripes, the condition's icon. It goes away by itself.

use crate::Message;
use arclens_ipc::Toast;
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, CREAM, DISPLAY, DISPLAY_SEMI, INK, RADIUS, STRONG};
use iced::widget::{Space, column, container, image, row, text};
use iced::{Alignment, Border, Element, Length, Size};
use std::time::{Duration, Instant};

const WIDTH: f32 = 420.0;
/// Top of the toast, as a share of the screen height (clear of the game's
/// top bar).
const TOP: f32 = 0.09;

/// A toast on screen, its icon decoded on arrival.
#[derive(Debug)]
pub struct Shown {
    toast: Toast,
    icon: Option<image::Handle>,
    until: Instant,
}

impl Shown {
    pub fn new(toast: Toast, now: Instant) -> Self {
        let icon = toast
            .icon
            .as_deref()
            .and_then(|p| arclens_ui::decode_icon(p, 112));
        let until = now + Duration::from_secs(u64::from(toast.seconds.clamp(2, 60)));
        Self { toast, icon, until }
    }

    pub fn expired(&self, now: Instant) -> bool {
        now >= self.until
    }
}

pub fn view(shown: &Shown, screen: Size) -> Element<'_, Message> {
    let toast = &shown.toast;
    let icon: Element<'_, Message> = match &shown.icon {
        Some(handle) => image(handle.clone()).width(56).height(56).into(),
        None => Space::new().width(0).height(56).into(),
    };
    let head = container(
        row![
            icon,
            column![
                text(toast.kicker.to_uppercase())
                    .size(theme::size::TINY)
                    .font(DISPLAY_SEMI)
                    .color(with_alpha(INK, 0.65)),
                text(toast.title.clone())
                    .size(theme::size::H2)
                    .font(DISPLAY)
                    .color(INK),
            ]
            .spacing(2),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([12, 16])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(CREAM.into()),
        ..container::Style::default()
    });
    let mut card = column![head, theme::stripe_band(4.0)];
    if !toast.body.is_empty() {
        card = card.push(
            container(
                text(toast.body.clone())
                    .size(theme::size::SMALL)
                    .font(STRONG)
                    .color(palette::TEXT),
            )
            .padding([10, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(with_alpha(theme::PANEL, theme::surface_alpha(0.97)).into()),
                ..container::Style::default()
            }),
        );
    }
    let card = container(card)
        .width(WIDTH)
        .clip(true)
        .style(|_| container::Style {
            border: Border {
                color: with_alpha(theme::ACCENT, 0.8),
                width: 2.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        });
    container(card)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(iced::Padding {
            top: (screen.height * TOP).round(),
            ..iced::Padding::ZERO
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_after_its_time() {
        let now = Instant::now();
        let shown = Shown::new(
            Toast {
                kicker: "Condition alert".into(),
                title: "Night Raid in 5 min".into(),
                body: String::new(),
                icon: None,
                seconds: 8,
            },
            now,
        );
        assert!(!shown.expired(now + Duration::from_secs(7)));
        assert!(shown.expired(now + Duration::from_secs(8)));
    }
}
