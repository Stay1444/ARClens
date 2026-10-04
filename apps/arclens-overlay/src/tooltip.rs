//! The tooltip for the marker or area under the pointer on the map.

use crate::{Message, Overlay};
use arclens_core::{MapPoint, Marker, MarkerArea, Transform, humanize};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, CREAM, DISPLAY, INK, RADIUS};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Border, Element, Length, Point, Size};
/// A badge counts as hovered within this distance of its centre (logical
/// pixels): its radius plus some slack.
const HIT: f32 = 15.0;
const WIDTH: f32 = 280.0;
/// Gap between the pointer and the tooltip.
const OFFSET: f32 = 18.0;

/// What the pointer is over.
#[derive(Debug, PartialEq)]
pub enum Hovered<'a> {
    Marker(&'a Marker),
    Area(&'a MarkerArea),
}

/// The marker under `pointer` (screen pixels), else the area around it.
pub fn hovered<'a>(
    markers: &'a [Marker],
    areas: &'a [MarkerArea],
    transform: Transform,
    screen: Size,
    pointer: Point,
) -> Option<Hovered<'a>> {
    let at = |p: MapPoint| {
        let (x, y) = transform.apply((p.x, p.y));
        Point::new(x * screen.width, y * screen.height)
    };
    let nearest = markers
        .iter()
        .map(|m| (m, at(m.position).distance(pointer)))
        .filter(|&(_, d)| d <= HIT)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((marker, _)) = nearest {
        return Some(Hovered::Marker(marker));
    }
    areas
        .iter()
        .find(|area| {
            at(area.center).distance(pointer) <= HIT || {
                let outline: Vec<Point> = area.hull.iter().map(|&p| at(p)).collect();
                outline.len() >= 3 && inside(&outline, pointer)
            }
        })
        .map(Hovered::Area)
}

fn inside(polygon: &[Point], p: Point) -> bool {
    let mut odd = false;
    let mut j = polygon.len() - 1;
    for (i, a) in polygon.iter().enumerate() {
        let b = polygon[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// The tooltip next to the pointer, if it is over something.
pub fn view(state: &Overlay) -> Option<Element<'_, Message>> {
    let (screen, transform, (px, py)) = (state.screen?, state.transform?, state.pointer?);
    let pointer = Point::new(px * screen.width, py * screen.height);
    if let Some(clip) = state.clip {
        let inside_clip = (clip.x..=clip.x + clip.width).contains(&px)
            && (clip.y..=clip.y + clip.height).contains(&py);
        if !inside_clip {
            return None;
        }
    }
    let (badge, title, body) =
        match hovered(&state.markers, &state.areas, transform, screen, pointer)? {
            Hovered::Marker(marker) => {
                let kind = match &marker.subcategory {
                    Some(sub) => format!("{} · {}", humanize(&marker.category), humanize(sub)),
                    None => humanize(&marker.category),
                };
                let mut col = column![
                    text(kind)
                        .size(theme::size::SMALL)
                        .color(palette::TEXT_MUTED)
                ]
                .spacing(4);
                if marker.locked {
                    col = col.push(
                        text("Behind a locked door")
                            .size(theme::size::SMALL)
                            .color(palette::TEXT_MUTED),
                    );
                }
                (
                    arclens_ui::markers::badge(
                        &marker.category,
                        marker.subcategory.as_deref(),
                        24.0,
                    ),
                    marker.title(),
                    col,
                )
            }
            Hovered::Area(area) => {
                let kind = area
                    .subcategory
                    .as_deref()
                    .map_or_else(|| humanize(&area.category), humanize);
                (
                    arclens_ui::markers::badge(&area.category, area.subcategory.as_deref(), 24.0),
                    format!("{} × {kind}", area.count),
                    column![
                        text("Possible spots in this area: not all are there every raid.")
                            .size(theme::size::SMALL)
                            .color(palette::TEXT_MUTED)
                    ],
                )
            }
        };
    // Right of and below the pointer, flipped near the screen's edges.
    let x = if pointer.x + OFFSET + WIDTH > screen.width {
        pointer.x - OFFSET - WIDTH
    } else {
        pointer.x + OFFSET
    };
    let y = if pointer.y + OFFSET + 110.0 > screen.height {
        pointer.y - OFFSET - 110.0
    } else {
        pointer.y + OFFSET
    };
    let card = card(badge, &title, body);
    Some(
        container(card)
            .padding(iced::Padding {
                top: y.max(0.0),
                left: x.max(0.0),
                ..iced::Padding::ZERO
            })
            .into(),
    )
}

/// The game's look: a cream header with the badge and the name in
/// condensed capitals, over a dark body.
fn card<'a>(
    badge: Element<'a, Message>,
    title: &str,
    body: iced::widget::Column<'a, Message>,
) -> Element<'a, Message> {
    let header = container(
        row![
            badge,
            text(title.to_uppercase())
                .size(theme::size::H2)
                .font(DISPLAY)
                .color(INK)
                .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([5, 10])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(CREAM.into()),
        border: Border {
            radius: iced::border::Radius::default().top(RADIUS),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let body = container(body)
        .padding([8, 12])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(with_alpha(theme::PANEL, theme::surface_alpha(0.94)).into()),
            border: Border {
                radius: iced::border::Radius::default().bottom(RADIUS),
                width: 1.0,
                color: palette::BORDER,
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });
    column![header, body].width(WIDTH).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::MapId;

    fn marker(x: f32, y: f32) -> Marker {
        Marker {
            id: format!("{x},{y}"),
            map: MapId::new("dam"),
            category: "containers".into(),
            subcategory: Some("raider_cache".into()),
            position: MapPoint::new(x, y),
            label: None,
            locked: false,
            conditions: None,
        }
    }

    #[test]
    fn finds_the_nearest_badge_then_the_area() {
        // Map units = screen pixels on a 1000×1000 screen.
        let transform = Transform::scale_translate(0.001, 0.001, 0.0, 0.0);
        let screen = Size::new(1000.0, 1000.0);
        let markers = [marker(100.0, 100.0), marker(110.0, 100.0)];
        let area = MarkerArea {
            category: "containers".into(),
            subcategory: Some("raider_cache".into()),
            center: MapPoint::new(500.0, 500.0),
            hull: vec![
                MapPoint::new(400.0, 400.0),
                MapPoint::new(600.0, 400.0),
                MapPoint::new(600.0, 600.0),
                MapPoint::new(400.0, 600.0),
            ],
            count: 7,
        };
        let areas = [area];
        let pick = |x, y| hovered(&markers, &areas, transform, screen, Point::new(x, y));
        assert_eq!(pick(108.0, 101.0), Some(Hovered::Marker(&markers[1])));
        assert_eq!(pick(450.0, 580.0), Some(Hovered::Area(&areas[0])));
        assert_eq!(pick(800.0, 800.0), None);
    }
}
