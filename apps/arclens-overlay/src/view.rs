//! Rendering. Everything outside the drawn widgets stays fully transparent.

use crate::{Message, Overlay};
use arclens_ui::{CardSize, ItemCard, item_card};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{Space, column, container, stack, text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};

pub fn view(state: &Overlay) -> Element<'_, Message> {
    let hover = hover_layer(state);
    let panel = state
        .screen
        .and_then(|screen| crate::map_panel::view(&state.panel, screen));
    // Markers on the in-game map, under the panel and cards; shown whenever
    // the app sent some, like the hover card.
    let mut layers = stack![];
    if state.transform.is_some() && !(state.markers.is_empty() && state.areas.is_empty()) {
        layers = layers.push(
            Canvas::new(MarkerLayer { state })
                .width(Length::Fill)
                .height(Length::Fill),
        );
    }
    if let Some(panel) = panel {
        layers = layers.push(panel);
    }
    if let Some(tooltip) = crate::tooltip::view(state) {
        layers = layers.push(tooltip);
    }
    if let (Some(card), Some(screen)) = (&state.menu_card, state.screen) {
        layers = layers.push(crate::menu_card::view(card, state.now_ms, screen));
    }
    if let (true, Some(screen)) = (state.interactive, state.screen) {
        layers = layers.push(crate::search::view(&state.search, screen));
    }
    let hover: Element<'_, Message> = layers.push(hover).into();
    if !state.visible {
        return hover;
    }

    // The badge is always drawn while visible, so "is the overlay on screen
    // at all?" can be answered at a glance, even with nothing selected.
    let corner = state.settings.corner;
    let card = state.item.as_ref().map(|shown| {
        item_card(&ItemCard {
            item: &shown.item,
            advice: shown.advice.clone(),
            icon: shown.icon.as_ref(),
            recycle_names: shown.recycle_names.clone(),
            size: CardSize::Compact,
        })
    });
    // The badge stays on the screen's edge, the card towards the middle.
    let panel = if corner.is_top() {
        column![status_badge(state)].push(card)
    } else {
        column![].push(card).push(status_badge(state))
    }
    .spacing(8)
    .align_x(if corner.is_left() {
        iced::Alignment::Start
    } else {
        iced::Alignment::End
    });
    let placed = container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24);
    let placed = if corner.is_left() {
        placed.align_left(Length::Fill)
    } else {
        placed.align_right(Length::Fill)
    };
    let placed = if corner.is_top() {
        placed.align_top(Length::Fill)
    } else {
        placed.align_bottom(Length::Fill)
    };

    stack![placed, hover].into()
}

/// Gap between the game's tooltip and our card, in logical pixels.
const GAP: f32 = 12.0;
/// Compact card width (see `arclens_ui::card`) and a conservative height
/// used only to keep the card on screen.
const CARD: iced::Size = iced::Size::new(360.0, 420.0);

/// The detected-hover card, placed beside the game's tooltip.
fn hover_layer(state: &Overlay) -> Element<'_, Message> {
    let Some((shown, anchor, item_side)) = &state.hover else {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    };
    let (anchor, item_side) = (*anchor, *item_side);
    iced::widget::responsive(move |screen| {
        let at = place(anchor, item_side, screen, CARD);
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
pub fn place(
    anchor: arclens_ipc::NormRect,
    item_side: arclens_ipc::ItemSide,
    screen: iced::Size,
    card: iced::Size,
) -> Point {
    let left = anchor.x * screen.width;
    let right = (anchor.x + anchor.width) * screen.width;
    let top = anchor.y * screen.height;

    // Beside the tooltip, away from the hovered item; the other side only
    // if the card doesn't fit there.
    let at_right = right + GAP;
    let at_left = left - GAP - card.width;
    let fits_right = at_right + card.width <= screen.width;
    let fits_left = at_left >= 0.0;
    let x = if item_side == arclens_ipc::ItemSide::Right && fits_left {
        at_left
    } else if fits_right {
        at_right
    } else {
        at_left.max(0.0)
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
        // The map's viewport, else the whole surface.
        let clip = self
            .state
            .clip
            .map_or(Rectangle::new(Point::ORIGIN, bounds.size()), |c| {
                Rectangle {
                    x: c.x * bounds.width,
                    y: c.y * bounds.height,
                    width: c.width * bounds.width,
                    height: c.height * bounds.height,
                }
            });
        // Cached: redrawn only when the markers, the view or the surface
        // change.
        let geometry = self
            .state
            .marker_cache
            .draw(renderer, bounds.size(), |frame| {
                // Only what lies in the viewport (by centre: a badge on the
                // edge may overhang a little). `Frame::with_clip` drew
                // nothing here under iced 0.14, so no clipping.
                let to_frame = |nx: f32, ny: f32| Point::new(nx * bounds.width, ny * bounds.height);
                for area in &self.state.areas {
                    let (cx, cy) = transform.apply((area.center.x, area.center.y));
                    if !clip.contains(to_frame(cx, cy)) {
                        continue;
                    }
                    arclens_ui::markers::draw_area(
                        frame,
                        area,
                        |p| {
                            let (nx, ny) = transform.apply((p.x, p.y));
                            to_frame(nx, ny)
                        },
                        BADGE,
                    );
                }
                for marker in &self.state.markers {
                    // The transform targets the screen normalised to 0..=1.
                    let (nx, ny) = transform.apply((marker.position.x, marker.position.y));
                    let at = to_frame(nx, ny);
                    if clip.contains(at) {
                        draw_badge(frame, marker, at);
                    }
                }
            });
        vec![geometry]
    }
}

/// Marker diameter on the in-game map, logical pixels.
const BADGE: f32 = 22.0;

/// The marker's glyph on its category colour, with a dark rim so it reads
/// on the bright parts of the map.
fn draw_badge(frame: &mut Frame, marker: &arclens_core::Marker, at: Point) {
    use arclens_ui::markers::{glyph, handle};
    frame.fill(
        &Path::circle(at, BADGE / 2.0 + 1.5),
        Color::from_rgba8(0, 0, 0, 0.6),
    );
    frame.fill(
        &Path::circle(at, BADGE / 2.0),
        arclens_ui::palette::marker(&marker.category),
    );
    let inner = BADGE * 0.62;
    frame.draw_svg(
        Rectangle::new(
            Point::new(at.x - inner / 2.0, at.y - inner / 2.0),
            iced::Size::new(inner, inner),
        ),
        &handle(glyph(&marker.category, marker.subcategory.as_deref())),
    );
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
        assert_eq!(
            place(anchor, arclens_ipc::ItemSide::Left, SCREEN, CARD),
            Point::new(800.0 + GAP, 300.0)
        );
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
            place(anchor, arclens_ipc::ItemSide::Left, SCREEN, CARD),
            Point::new(1400.0 - GAP - 360.0, 100.0)
        );
    }

    #[test]
    fn goes_left_when_the_item_is_on_the_right() {
        // The user's Heavy Fuze Grenade case: room on both sides, item right.
        let anchor = NormRect {
            x: 0.58,
            y: 0.2,
            width: 0.21,
            height: 0.37,
        };
        let at = place(anchor, arclens_ipc::ItemSide::Right, SCREEN, CARD);
        assert!((at.x - (1160.0 - GAP - 360.0)).abs() < 1e-3);
        // No room on the left: the right side after all.
        let tight = NormRect { x: 0.1, ..anchor };
        let at = place(tight, arclens_ipc::ItemSide::Right, SCREEN, CARD);
        assert!((at.x - (0.31 * 2000.0 + GAP)).abs() < 1e-3);
    }

    #[test]
    fn stays_on_screen_vertically() {
        let anchor = NormRect {
            x: 0.1,
            y: 0.9,
            width: 0.2,
            height: 0.1,
        };
        assert!(
            (place(anchor, arclens_ipc::ItemSide::Left, SCREEN, CARD).y - (1000.0 - 420.0)).abs()
                < 1e-3
        );
    }
}
