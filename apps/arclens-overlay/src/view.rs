//! Rendering. Everything outside the drawn widgets stays fully transparent.

use crate::{Message, Overlay};
use arclens_core::{Advice, Item, MarkerKind, Verdict};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{Space, column, container, stack, text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};

const CARD_WIDTH: f32 = 320.0;

pub fn view(state: &Overlay) -> Element<'_, Message> {
    if !state.visible {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    }

    let markers = Canvas::new(MarkerLayer { state })
        .width(Length::Fill)
        .height(Length::Fill);

    let card: Element<'_, Message> = match &state.item {
        Some((item, advice)) => item_card(item, advice),
        None => Space::new().into(),
    };

    stack![
        markers,
        container(card)
            .width(Length::Fill)
            .align_right(Length::Fill)
            .padding(24),
    ]
    .into()
}

fn item_card<'a>(item: &'a Item, advice: &'a Advice) -> Element<'a, Message> {
    let (label, color) = match advice.verdict {
        Verdict::Keep => ("KEEP", Color::from_rgb8(0x4c, 0xaf, 0x50)),
        Verdict::Recycle => ("RECYCLE", Color::from_rgb8(0x29, 0x8f, 0xd6)),
        Verdict::Sell => ("SELL", Color::from_rgb8(0xf2, 0xb1, 0x34)),
        Verdict::Unknown => ("?", Color::from_rgb8(0x9e, 0x9e, 0x9e)),
    };

    let mut lines =
        column![text(&item.name).size(20), text(label).size(16).color(color),].spacing(4);
    if let Some(value) = advice.sell_value {
        lines = lines.push(text(format!("Sell: {value}")));
    }
    if let Some(value) = advice.recycle_value {
        lines = lines.push(text(format!("Recycle: {value}")));
    }
    for req in item.required_for.iter().take(4) {
        lines = lines.push(text(format!("• {} ×{}", req.name, req.quantity)).size(13));
    }

    container(lines)
        .width(CARD_WIDTH)
        .padding(12)
        .style(|_| container::Style {
            background: Some(Color::from_rgba8(0x10, 0x12, 0x16, 0.85).into()),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            text_color: Some(Color::WHITE),
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
