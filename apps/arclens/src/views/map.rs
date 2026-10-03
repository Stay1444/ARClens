//! The Map tab: every marker of a map, searchable, with per-kind toggles.
//! The same filter decides what the overlay draws on the in-game map.

use crate::app::Message;
use arclens_core::{Marker, MarkerFilter, humanize, marker_counts};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{
    Canvas, Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{
    Alignment, Border, Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Theme, font,
    mouse,
};
use std::collections::BTreeSet;

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};
const PANEL_WIDTH: f32 = 340.0;
pub const SEARCH_ID: &str = "marker-search";

/// Markers of the selected map, as far as they are loaded.
pub enum Markers<'a> {
    Loading,
    Ready(&'a [Marker]),
    Failed(&'a str),
}

pub struct MapView<'a> {
    /// `(id, name)` of every map.
    pub maps: &'a [(&'static str, &'static str)],
    pub selected: &'a str,
    pub markers: Markers<'a>,
    pub filter: &'a MarkerFilter,
    pub query: &'a str,
    /// Categories opened to show their subcategories.
    pub expanded: &'a BTreeSet<String>,
    pub attribution: &'a str,
}

pub fn view<'a>(map: &MapView<'a>) -> Element<'a, Message> {
    let pills = map
        .maps
        .iter()
        .fold(row![].spacing(6), |r, &(id, name)| {
            r.push(pill(
                name,
                id == map.selected,
                Message::SelectMap(id.to_owned()),
            ))
        })
        .wrap();

    let body: Element<'a, Message> = match map.markers {
        Markers::Loading => centered(text("Loading markers…").color(palette::TEXT_MUTED)),
        Markers::Failed(error) => centered(
            column![
                text("Could not load markers").size(20).font(BOLD),
                text(error).color(palette::TEXT_MUTED),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        ),
        Markers::Ready(markers) => row![
            panel(map, markers),
            container(
                Canvas::new(Plot {
                    markers,
                    filter: map.filter,
                    query: map.query,
                })
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .height(Length::Fill)
        .into(),
    };

    column![
        container(pills).padding([12, 16]),
        body,
        container(
            text(format!("Markers: {}", map.attribution))
                .size(11)
                .color(palette::TEXT_MUTED)
        )
        .padding([4, 16]),
    ]
    .height(Length::Fill)
    .into()
}

/// Left panel: search, show/hide all, categories with counts.
fn panel<'a>(map: &MapView<'a>, markers: &'a [Marker]) -> Element<'a, Message> {
    let matching: Vec<Marker> = markers
        .iter()
        .filter(|m| m.matches(map.query))
        .cloned()
        .collect();
    let shown = matching.iter().filter(|m| map.filter.shows(m)).count();

    let mut categories = column![].spacing(2);
    for (category, subs) in marker_counts(&matching) {
        let total: usize = subs.values().sum();
        let open = map.expanded.contains(category) || !map.query.trim().is_empty();
        let has_subs = subs.keys().any(|s| !s.is_empty());
        categories = categories.push(category_row(
            category,
            total,
            map.filter.shows_category(category),
            has_subs.then_some(open),
        ));
        if open && has_subs {
            for (sub, count) in subs.into_iter().filter(|(s, _)| !s.is_empty()) {
                let (c, s) = (category.to_owned(), sub.to_owned());
                categories = categories.push(
                    row![
                        Space::new().width(28),
                        checkbox(map.filter.shows_subcategory(category, sub))
                            .label(humanize(sub))
                            .text_size(13)
                            .size(14)
                            .on_toggle(move |_| Message::ToggleMarkerSubcategory(
                                c.clone(),
                                s.clone()
                            ))
                            .width(Length::Fill),
                        text(count.to_string()).size(12).color(palette::TEXT_MUTED),
                    ]
                    .align_y(Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }
    }

    let header = row![
        text(format!("{shown} OF {} MARKERS SHOWN", matching.len()))
            .size(11)
            .color(palette::TEXT_MUTED)
            .width(Length::Fill),
        small_button("Show all", Message::ShowAllMarkers),
        small_button("Hide all", Message::HideAllMarkers),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // Named places only: unnamed markers are covered by the counts above.
    let named: Vec<&Marker> = matching.iter().filter(|m| m.label.is_some()).collect();
    let matches = (!map.query.trim().is_empty() && !named.is_empty()).then(|| {
        named.iter().take(50).fold(column![].spacing(4), |col, m| {
            col.push(
                row![
                    dot(palette::marker(&m.category)),
                    text(m.title()).size(13).width(Length::Fill),
                    text(humanize(&m.category))
                        .size(11)
                        .color(palette::TEXT_MUTED),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
        })
    });

    container(
        column![
            text_input("Search markers…", map.query)
                .id(SEARCH_ID)
                .on_input(Message::MarkerQuery)
                .padding([8, 12])
                .size(14),
            header,
            scrollable(
                column![categories]
                    .push(matches.map(|m| {
                        column![text("MATCHES").size(11).color(palette::TEXT_MUTED), m]
                            .spacing(6)
                            .padding([12, 8])
                    }))
                    .padding([0, 8])
            )
            .height(Length::Fill),
        ]
        .spacing(10),
    )
    .padding([0, 16])
    .width(PANEL_WIDTH)
    .height(Length::Fill)
    .into()
}

/// A category: toggle, colour, name, count and (if it has subcategories)
/// an expander.
fn category_row<'a>(
    category: &str,
    count: usize,
    shown: bool,
    open: Option<bool>,
) -> Element<'a, Message> {
    let name = category.to_owned();
    let toggle = checkbox(shown)
        .label(humanize(category))
        .text_size(14)
        .size(16)
        .on_toggle(move |_| Message::ToggleMarkerCategory(name.clone()))
        .width(Length::Fill);
    let expander: Element<'a, Message> = match open {
        Some(open) => button(text(if open { "▾" } else { "▸" }).size(13))
            .padding([0, 6])
            .on_press(Message::ExpandMarkerCategory(category.to_owned()))
            .style(|_, _| button::Style {
                text_color: palette::TEXT_MUTED,
                ..button::Style::default()
            })
            .into(),
        None => Space::new().width(22).into(),
    };
    row![
        dot(palette::marker(category)),
        toggle,
        text(count.to_string()).size(12).color(palette::TEXT_MUTED),
        expander,
    ]
    .spacing(8)
    .padding([4, 8])
    .align_y(Alignment::Center)
    .into()
}

/// The map's markers as dots, fitted to the canvas. A background map image
/// comes once its alignment with the marker coordinates is known.
struct Plot<'a> {
    markers: &'a [Marker],
    filter: &'a MarkerFilter,
    query: &'a str,
}

impl canvas::Program<Message> for Plot<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill(
            &Path::rounded_rectangle(Point::ORIGIN, bounds.size(), 8.0.into()),
            Color::from_rgb8(0x12, 0x15, 0x1a),
        );
        let Some(fit) = Fit::new(self.markers, bounds.size()) else {
            return vec![frame.into_geometry()];
        };
        let searching = !self.query.trim().is_empty();
        let visible: Vec<(&Marker, Point)> = self
            .markers
            .iter()
            .filter(|m| self.filter.shows(m))
            .map(|m| (m, fit.point(m)))
            .collect();

        for (marker, at) in &visible {
            if is_named_place(marker) {
                continue;
            }
            let hit = searching && marker.matches(self.query);
            let color = palette::marker(&marker.category);
            let color = if searching && !hit {
                with_alpha(color, 0.25)
            } else {
                color
            };
            let radius = if hit { 5.5 } else { 3.5 };
            frame.fill(&Path::circle(*at, radius), color);
            if hit {
                frame.stroke(
                    &Path::circle(*at, radius + 2.0),
                    Stroke::default().with_color(Color::WHITE).with_width(1.5),
                );
            }
        }
        // Names of places, on top of the dots.
        for (marker, at) in visible.iter().filter(|(m, _)| is_named_place(m)) {
            let dim = searching && !marker.matches(self.query);
            let label = |position: Point, color: Color| canvas::Text {
                content: marker.title(),
                position,
                color,
                size: 12.0.into(),
                font: BOLD,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            };
            // A dark outline keeps names readable over dots.
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                frame.fill_text(label(Point::new(at.x + dx, at.y + dy), Color::BLACK));
            }
            let color = if dim {
                with_alpha(palette::TEXT, 0.35)
            } else {
                palette::TEXT
            };
            frame.fill_text(label(*at, color));
        }
        // Tooltip for the marker under the cursor.
        if let Some(cursor) = cursor.position_in(bounds)
            && let Some((marker, at)) = visible
                .iter()
                .map(|(m, at)| (m, at, at.distance(cursor)))
                .filter(|(_, _, d)| *d <= 8.0)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(m, at, _)| (m, at))
        {
            let mut label = marker.title();
            if let Some(sub) = &marker.subcategory
                && marker.label.is_some()
            {
                label = format!("{label} · {}", humanize(sub));
            }
            if marker.locked {
                label.push_str(" · locked");
            }
            frame.fill_text(canvas::Text {
                content: label,
                position: Point::new(at.x + 10.0, at.y),
                color: Color::WHITE,
                size: 13.0.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }

    /// Redraw on pointer moves inside the plot, for the hover tooltip.
    fn update(
        &self,
        _state: &mut (),
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if cursor.is_over(bounds) => {
                Some(canvas::Action::request_redraw())
            }
            _ => None,
        }
    }
}

/// Named places are drawn as their name instead of a dot.
fn is_named_place(marker: &Marker) -> bool {
    let c = marker.category.to_lowercase();
    marker.label.is_some() && ["label", "zone", "poi"].iter().any(|w| c.contains(w))
}

/// Uniform scale + offset fitting all markers into the canvas (aspect kept).
struct Fit {
    scale: f32,
    min: Point,
    offset: Point,
}

impl Fit {
    const MARGIN: f32 = 24.0;

    fn new(markers: &[Marker], size: Size) -> Option<Self> {
        let (mut min, mut max) = (
            Point::new(f32::MAX, f32::MAX),
            Point::new(f32::MIN, f32::MIN),
        );
        for m in markers {
            min = Point::new(min.x.min(m.position.x), min.y.min(m.position.y));
            max = Point::new(max.x.max(m.position.x), max.y.max(m.position.y));
        }
        let (w, h) = ((max.x - min.x).max(1.0), (max.y - min.y).max(1.0));
        let avail = Size::new(
            size.width - 2.0 * Self::MARGIN,
            size.height - 2.0 * Self::MARGIN,
        );
        if markers.is_empty() || avail.width <= 0.0 || avail.height <= 0.0 {
            return None;
        }
        let scale = (avail.width / w).min(avail.height / h);
        Some(Self {
            scale,
            min,
            offset: Point::new(
                Self::MARGIN + (avail.width - w * scale) / 2.0,
                Self::MARGIN + (avail.height - h * scale) / 2.0,
            ),
        })
    }

    fn point(&self, marker: &Marker) -> Point {
        Point::new(
            self.offset.x + (marker.position.x - self.min.x) * self.scale,
            self.offset.y + (marker.position.y - self.min.y) * self.scale,
        )
    }
}

fn dot<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new().width(10).height(10))
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 5.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn pill(label: &str, active: bool, on_press: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding([5, 12])
        .on_press(on_press)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let alpha = match (active, hovered) {
                (true, _) => 0.18,
                (false, true) => 0.10,
                (false, false) => 0.04,
            };
            button::Style {
                background: Some(with_alpha(palette::TEXT, alpha).into()),
                text_color: if active {
                    palette::TEXT
                } else {
                    palette::TEXT_MUTED
                },
                border: Border {
                    color: palette::BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn small_button(label: &str, on_press: Message) -> Element<'_, Message> {
    button(text(label).size(12))
        .padding([3, 8])
        .on_press(on_press)
        .style(|_, status| button::Style {
            background: Some(
                with_alpha(
                    palette::TEXT,
                    if matches!(status, button::Status::Hovered) {
                        0.12
                    } else {
                        0.06
                    },
                )
                .into(),
            ),
            text_color: palette::TEXT,
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: 5.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
