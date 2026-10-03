//! Rendering. Everything outside the drawn widgets stays fully transparent.

use crate::{Message, Overlay};
use arclens_core::MarkerKind;
use arclens_ui::{CardSize, ItemCard, item_card};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{Space, column, container, stack, text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};

pub fn view(state: &Overlay) -> Element<'_, Message> {
    if !state.visible {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
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
    ]
    .into()
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
