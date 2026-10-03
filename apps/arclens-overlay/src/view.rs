//! Rendering. Everything outside the drawn widgets stays fully transparent.

use crate::{Message, Overlay};
use arclens_core::MarkerKind;
use arclens_ui::{CardSize, ItemCard, item_card};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{Space, column, container, stack, text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};

pub fn view(state: &Overlay) -> Element<'_, Message> {
    let hover = hover_layer(state);
    if !state.visible {
        return hover;
    }

    let markers = Canvas::new(MarkerLayer { state })
        .width(Length::Fill)
        .height(Length::Fill);

    // The badge is always drawn while visible, so "is the overlay on screen
    // at all?" can be answered at a glance, even with nothing selected.
    let mut panel = column![status_badge(state)]
        .spacing(8)
        .align_x(iced::Alignment::End);
    if let Some(shown) = &state.item {
        panel = panel.push(item_card(&ItemCard {
            item: &shown.item,
            advice: shown.advice.clone(),
            icon: shown.icon.as_ref(),
            recycle_names: shown.recycle_names.clone(),
            size: CardSize::Compact,
        }));
    }

    stack![
        markers,
        container(panel)
            .width(Length::Fill)
            .align_right(Length::Fill)
            .padding(24),
        hover,
    ]
    .into()
}

/// Gap between the game's tooltip and our card, in logical pixels.
const GAP: f32 = 12.0;
/// Compact card width (see `arclens_ui::card`) and a conservative height
/// used only to keep the card on screen.
const CARD: iced::Size = iced::Size::new(360.0, 420.0);

/// The detected-hover card, placed beside the game's tooltip.
fn hover_layer(state: &Overlay) -> Element<'_, Message> {
    let Some((shown, anchor)) = &state.hover else {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    };
    let anchor = *anchor;
    iced::widget::responsive(move |screen| {
        let at = place(anchor, screen, CARD);
        container(item_card(&ItemCard {
            item: &shown.item,
            advice: shown.advice.clone(),
            icon: shown.icon.as_ref(),
            recycle_names: shown.recycle_names.clone(),
            size: CardSize::Compact,
        }))
        .padding(iced::Padding {
            top: at.y,
            left: at.x,
            ..iced::Padding::ZERO
        })
        .into()
    })
    .into()
}

/// Top-left corner for a `card` next to the game tooltip `anchor`: to its
/// right if it fits, otherwise to its left; top-aligned, clamped on screen.
pub fn place(anchor: arclens_ipc::NormRect, screen: iced::Size, card: iced::Size) -> Point {
    let left = anchor.x * screen.width;
    let right = (anchor.x + anchor.width) * screen.width;
    let top = anchor.y * screen.height;

    let x = if right + GAP + card.width <= screen.width {
        right + GAP
    } else {
        (left - GAP - card.width).max(0.0)
    };
    let y = top.clamp(0.0, (screen.height - card.height).max(0.0));
    Point::new(x, y)
}

struct MarkerLayer<'a> {
    state: &'a Overlay,
}

impl canvas::Program<Message> for MarkerLayer<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let Some(transform) = self.state.transform else {
            return Vec::new();
        };
        let mut frame = Frame::new(renderer, bounds.size());
        for marker in &self.state.markers {
            let (x, y) = transform.apply((marker.position.x, marker.position.y));
            if !(0.0..=bounds.width).contains(&x) || !(0.0..=bounds.height).contains(&y) {
                continue;
            }
            frame.fill(
                &Path::circle(Point::new(x, y), 6.0),
                marker_color(&marker.kind),
            );
        }
        vec![frame.into_geometry()]
    }
}

fn marker_color(kind: &MarkerKind) -> Color {
    match kind {
        MarkerKind::Extraction => Color::from_rgb8(0x4c, 0xaf, 0x50),
        MarkerKind::RaiderHatch => Color::from_rgb8(0x8e, 0x5c, 0xd9),
        MarkerKind::QuestObjective => Color::from_rgb8(0xf2, 0xb1, 0x34),
        MarkerKind::Arc => Color::from_rgb8(0xe5, 0x39, 0x35),
        _ => Color::from_rgb8(0xe0, 0xe0, 0xe0),
    }
}

fn status_badge(state: &Overlay) -> Element<'_, Message> {
    let mode = if state.interactive {
        "interactive"
    } else {
        "click-through"
    };
    let hint = if state.item.is_none() {
        " · pick an item in the app"
    } else {
        ""
    };
    container(text(format!("ARClens · {mode}{hint}")).size(12))
        .padding([4, 10])
        .style(|_| container::Style {
            background: Some(Color::from_rgba8(0x10, 0x12, 0x16, 0.75).into()),
            border: iced::Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            text_color: Some(Color::from_rgb8(0xc8, 0xcc, 0xd4)),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_ipc::NormRect;

    const SCREEN: iced::Size = iced::Size::new(2000.0, 1000.0);

    #[test]
    fn places_card_right_of_tooltip_when_it_fits() {
        let anchor = NormRect {
            x: 0.2,
            y: 0.3,
            width: 0.2,
            height: 0.4,
        };
        assert_eq!(place(anchor, SCREEN, CARD), Point::new(800.0 + GAP, 300.0));
    }

    #[test]
    fn flips_left_near_the_right_edge() {
        let anchor = NormRect {
            x: 0.7,
            y: 0.1,
            width: 0.2,
            height: 0.4,
        };
        assert_eq!(
            place(anchor, SCREEN, CARD),
            Point::new(1400.0 - GAP - 360.0, 100.0)
        );
    }

    #[test]
    fn stays_on_screen_vertically() {
        let anchor = NormRect {
            x: 0.1,
            y: 0.9,
            width: 0.2,
            height: 0.1,
        };
        assert!((place(anchor, SCREEN, CARD).y - (1000.0 - 420.0)).abs() < 1e-3);
    }
}
