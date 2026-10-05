//! Rendering. Everything outside the drawn widgets stays fully transparent.

use crate::{Message, Overlay};
use arclens_i18n::t;
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
    if !state.stash_badges.is_empty() {
        layers = layers.push(
            Canvas::new(StashLayer { state })
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
        layers = layers.push(crate::menu_card::view(
            card,
            &state.menu_icons,
            state.now_ms,
            screen,
        ));
    }
    if let Some(screen) = state.screen
        && (state.interactive || !state.details.is_empty())
    {
        layers = layers.push(top_row(state, screen));
    }
    let hover: Element<'_, Message> = layers.push(hover).into();
    if !state.visible {
        return hover;
    }

    // The badge is always drawn while visible, so "is the overlay on screen
    // at all?" can be answered at a glance.
    let corner = state.settings.corner;
    let placed = container(status_badge(state))
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

/// The quick search (while interactive) and the item windows, side by
/// side across the top of the screen.
fn top_row(state: &Overlay, screen: iced::Size) -> Element<'_, Message> {
    use crate::{detail, search};
    let search = state
        .interactive
        .then(|| search::panel(&state.search, state.details.open.len()));
    let used = if search.is_some() {
        search::WIDTH + detail::GAP
    } else {
        0.0
    };
    let windows = detail::windows(
        &state.details,
        screen.width - 48.0 - used,
        screen.height * (1.0 - search::TOP) - 24.0,
    );
    let row = iced::widget::row![]
        .spacing(detail::GAP)
        .align_y(iced::Alignment::Start)
        .push(search)
        .push(windows);
    column![
        Space::new().height(screen.height * search::TOP),
        container(row).center_x(Length::Fill),
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
    let Some(hovered) = &state.hover else {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    };
    let shown = &hovered.shown;
    iced::widget::responsive(move |screen| {
        let at = place(
            hovered.anchor,
            hovered.item_side,
            &hovered.avoid,
            screen,
            CARD,
        );
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

/// Top-left corner for a `card` next to the game tooltip `anchor`: beside
/// it, away from the hovered item, top-aligned and clamped on screen. The
/// other side is used if the card doesn't fit or would cover a panel in
/// `avoid` (e.g. the trader's purchase panel); if every side covers one,
/// the first that fits wins.
pub fn place(
    anchor: arclens_ipc::NormRect,
    item_side: arclens_ipc::ItemSide,
    avoid: &[arclens_ipc::NormRect],
    screen: iced::Size,
    card: iced::Size,
) -> Point {
    let left = anchor.x * screen.width;
    let right = (anchor.x + anchor.width) * screen.width;
    let top = anchor.y * screen.height;
    let y = top.clamp(0.0, (screen.height - card.height).max(0.0));

    let at_right = right + GAP;
    let at_left = left - GAP - card.width;
    let fits = |x: f32| x >= 0.0 && x + card.width <= screen.width;
    let covers = |x: f32| {
        let rect = iced::Rectangle::new(Point::new(x, y), card);
        avoid.iter().any(|a| {
            rect.intersects(&iced::Rectangle {
                x: a.x * screen.width,
                y: a.y * screen.height,
                width: a.width * screen.width,
                height: a.height * screen.height,
            })
        })
    };
    let sides = if item_side == arclens_ipc::ItemSide::Right {
        [at_left, at_right]
    } else {
        [at_right, at_left]
    };
    let x = sides
        .iter()
        .copied()
        .find(|&x| fits(x) && !covers(x))
        .or_else(|| sides.iter().copied().find(|&x| fits(x)))
        .unwrap_or(at_left.max(0.0));
    Point::new(x, y)
}

/// Verdict tags in the stash slots' top-left corners.
struct StashLayer<'a> {
    state: &'a Overlay,
}

/// Tag text size and padding, layout pixels.
const STASH_TAG_TEXT: f32 = 11.0;
const STASH_TAG_PAD: f32 = 3.0;

impl canvas::Program<Message> for StashLayer<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        for badge in &self.state.stash_badges {
            let colour = arclens_ui::palette::verdict(badge.verdict);
            let mut label = arclens_ui::palette::verdict_label(badge.verdict);
            if badge.likely {
                label.push('?');
            }
            let at = Point::new(
                badge.slot.x * bounds.width + 4.0,
                badge.slot.y * bounds.height + 4.0,
            );
            // Roughly the text's width: the tag font is condensed caps.
            #[allow(clippy::cast_precision_loss, reason = "a short label")]
            let width = label.chars().count() as f32 * STASH_TAG_TEXT * 0.62 + 2.0 * STASH_TAG_PAD;
            let height = STASH_TAG_TEXT + 2.0 * STASH_TAG_PAD;
            let tag = Path::rounded_rectangle(at, iced::Size::new(width, height), 3.0.into());
            let alpha = if badge.likely { 0.78 } else { 0.95 };
            frame.fill(&tag, arclens_ui::palette::with_alpha(colour, alpha));
            frame.fill_text(canvas::Text {
                content: label,
                position: Point::new(at.x + STASH_TAG_PAD, at.y + STASH_TAG_PAD - 1.0),
                color: Color::from_rgb8(0x10, 0x12, 0x16),
                size: STASH_TAG_TEXT.into(),
                font: arclens_ui::theme::DISPLAY,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
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
        let to_frame = |nx: f32, ny: f32| Point::new(nx * bounds.width, ny * bounds.height);
        let screen_at = |p: arclens_core::MapPoint| {
            let (nx, ny) = transform.apply((p.x, p.y));
            to_frame(nx, ny)
        };
        let pointer = self.state.pointer.map(|(x, y)| to_frame(x, y));
        // Markers near the pointer are drawn apart, faded, every frame; the
        // rest come from the cache, rebuilt when the pointer changes cell.
        #[allow(clippy::cast_possible_truncation, reason = "screen cells")]
        let cell = pointer.map(|p| {
            (
                (p.x / FADE_CELL).floor() as i32,
                (p.y / FADE_CELL).floor() as i32,
            )
        });
        if self.state.fade_cell.get() != cell {
            self.state.fade_cell.set(cell);
            self.state.marker_cache.clear();
        }
        #[allow(clippy::cast_precision_loss, reason = "screen cells")]
        let cell_center = cell.map(|(cx, cy)| {
            Point::new((cx as f32 + 0.5) * FADE_CELL, (cy as f32 + 0.5) * FADE_CELL)
        });
        // Within this of the cell's centre, a marker may be near the pointer.
        let near_cell =
            |at: Point| cell_center.is_some_and(|c| c.distance(at) < FADE_RADIUS + FADE_CELL);
        let area_distance = |area: &arclens_core::MarkerArea, p: Point| {
            let outline: Vec<Point> = area.hull.iter().map(|&q| screen_at(q)).collect();
            if outline.len() >= 3 && crate::tooltip::inside(&outline, p) {
                0.0
            } else {
                screen_at(area.center).distance(p)
            }
        };
        // Only what lies in the viewport (by centre: a badge on the edge may
        // overhang a little). `Frame::with_clip` drew nothing here under
        // iced 0.14, so no clipping.
        let geometry = self
            .state
            .marker_cache
            .draw(renderer, bounds.size(), |frame| {
                for area in &self.state.areas {
                    let center = screen_at(area.center);
                    let near = cell_center
                        .is_some_and(|c| area_distance(area, c) < FADE_RADIUS + FADE_CELL);
                    if clip.contains(center) && !near && !near_cell(center) {
                        arclens_ui::markers::draw_area(frame, area, screen_at, BADGE, 1.0);
                    }
                }
                for marker in &self.state.markers {
                    let at = screen_at(marker.position);
                    if clip.contains(at) && !near_cell(at) {
                        draw_badge(frame, marker, at, 1.0);
                    }
                }
            });
        let mut layers = vec![geometry];
        if let Some(p) = pointer {
            let mut frame = Frame::new(renderer, bounds.size());
            for area in &self.state.areas {
                let center = screen_at(area.center);
                let near =
                    cell_center.is_some_and(|c| area_distance(area, c) < FADE_RADIUS + FADE_CELL);
                if clip.contains(center) && (near || near_cell(center)) {
                    let alpha = fade(area_distance(area, p));
                    arclens_ui::markers::draw_area(&mut frame, area, screen_at, BADGE, alpha);
                }
            }
            for marker in &self.state.markers {
                let at = screen_at(marker.position);
                if clip.contains(at) && near_cell(at) {
                    draw_badge(&mut frame, marker, at, fade(at.distance(p)));
                }
            }
            layers.push(frame.into_geometry());
        }
        layers
    }
}

/// Markers within this of the pointer (logical pixels) fade, so the
/// game's own map shows where the player points.
const FADE_RADIUS: f32 = 90.0;
/// Opacity right under the pointer.
const FADE_MIN: f32 = 0.25;
/// The pointer moves within cells this big before the cached markers
/// are redrawn.
pub const FADE_CELL: f32 = 48.0;

/// Opacity of something `distance` from the pointer: `FADE_MIN` at it,
/// easing up to solid at `FADE_RADIUS`.
fn fade(distance: f32) -> f32 {
    let k = (distance / FADE_RADIUS).clamp(0.0, 1.0);
    FADE_MIN + (1.0 - FADE_MIN) * k * k * (3.0 - 2.0 * k)
}

/// Marker diameter on the in-game map, logical pixels.
const BADGE: f32 = 22.0;

/// The marker's glyph on its category colour, with a dark rim so it reads
/// on the bright parts of the map.
fn draw_badge(frame: &mut Frame, marker: &arclens_core::Marker, at: Point, alpha: f32) {
    use arclens_ui::markers::{glyph, handle};
    frame.fill(
        &Path::circle(at, BADGE / 2.0 + 1.5),
        Color::from_rgba8(0, 0, 0, 0.6 * alpha),
    );
    frame.fill(
        &Path::circle(at, BADGE / 2.0),
        arclens_ui::palette::with_alpha(arclens_ui::palette::marker(&marker.category), alpha),
    );
    let inner = BADGE * 0.62;
    frame.draw_svg(
        Rectangle::new(
            Point::new(at.x - inner / 2.0, at.y - inner / 2.0),
            iced::Size::new(inner, inner),
        ),
        iced::advanced::svg::Svg::new(handle(glyph(
            &marker.category,
            marker.subcategory.as_deref(),
        )))
        .opacity(alpha),
    );
}

fn status_badge(state: &Overlay) -> Element<'_, Message> {
    let mode = if state.interactive {
        t!("top-interactive")
    } else {
        t!("top-click-through")
    };
    let hint = if state.interactive {
        String::new()
    } else {
        format!(" · {}", t!("overlay-search-shortcut"))
    };
    container(
        text(format!("ARCLENS · {mode}{hint}").to_uppercase())
            .size(arclens_ui::theme::size::TINY)
            .font(arclens_ui::theme::DISPLAY_SEMI),
    )
    .padding([4, 12])
    .style(|_| container::Style {
        background: Some(arclens_ui::palette::with_alpha(arclens_ui::theme::PANEL, 0.85).into()),
        border: iced::Border {
            radius: 12.0.into(),
            width: 1.0,
            color: arclens_ui::palette::BORDER,
        },
        text_color: Some(arclens_ui::palette::TEXT),
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
    fn markers_fade_near_the_pointer_only() {
        assert!((fade(0.0) - FADE_MIN).abs() < 1e-6);
        assert!((fade(FADE_RADIUS) - 1.0).abs() < 1e-6);
        assert!((fade(FADE_RADIUS * 3.0) - 1.0).abs() < 1e-6);
        let half = fade(FADE_RADIUS / 2.0);
        assert!(half > FADE_MIN && half < 1.0);
        assert!(fade(30.0) < fade(60.0));
    }

    #[test]
    fn places_card_right_of_tooltip_when_it_fits() {
        let anchor = NormRect {
            x: 0.2,
            y: 0.3,
            width: 0.2,
            height: 0.4,
        };
        assert_eq!(
            place(anchor, arclens_ipc::ItemSide::Left, &[], SCREEN, CARD),
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
            place(anchor, arclens_ipc::ItemSide::Left, &[], SCREEN, CARD),
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
        let at = place(anchor, arclens_ipc::ItemSide::Right, &[], SCREEN, CARD);
        assert!((at.x - (1160.0 - GAP - 360.0)).abs() < 1e-3);
        // No room on the left: the right side after all.
        let tight = NormRect { x: 0.1, ..anchor };
        let at = place(tight, arclens_ipc::ItemSide::Right, &[], SCREEN, CARD);
        assert!((at.x - (0.31 * 2000.0 + GAP)).abs() < 1e-3);
    }

    #[test]
    fn does_not_cover_the_trader_purchase_panel() {
        // Tooltip in the middle, room on both sides; the purchase panel
        // sits right of it.
        let anchor = NormRect {
            x: 0.4,
            y: 0.2,
            width: 0.15,
            height: 0.3,
        };
        let purchase = NormRect {
            x: 0.58,
            y: 0.1,
            width: 0.25,
            height: 0.7,
        };
        let at = place(
            anchor,
            arclens_ipc::ItemSide::Left,
            &[purchase],
            SCREEN,
            CARD,
        );
        assert!((at.x - (800.0 - GAP - 360.0)).abs() < 1e-3, "{at:?}");
        // Without it, the usual side.
        let at = place(anchor, arclens_ipc::ItemSide::Left, &[], SCREEN, CARD);
        assert!((at.x - (1100.0 + GAP)).abs() < 1e-3, "{at:?}");
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
            (place(anchor, arclens_ipc::ItemSide::Left, &[], SCREEN, CARD).y - (1000.0 - 420.0))
                .abs()
                < 1e-3
        );
    }
}
