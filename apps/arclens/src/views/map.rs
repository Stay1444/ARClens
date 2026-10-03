//! The Map tab: every marker of a map, searchable, with per-kind toggles
//! and icons. The same filter decides what the overlay shows in game.

use crate::app::Message;
use arclens_core::{Marker, MarkerFilter, humanize, marker_counts};
use arclens_ui::markers::{badge, glyph, handle};
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
/// Marker icon diameter on the plot.
const ICON: f32 = 16.0;
pub const SEARCH_ID: &str = "marker-search";

/// Markers of the selected map, as far as they are loaded.
pub enum Markers<'a> {
    Loading,
    Ready(&'a [Marker]),
    Failed(&'a str),
}

/// Everything the tab derives from (markers, query, filter). Built by the
/// app when one of those changes, never per frame.
#[derive(Debug, Default)]
pub struct MapSummary {
    /// Indices of markers matching the query.
    pub matching: Vec<usize>,
    /// Indices of matching markers the filter shows.
    pub visible: Vec<usize>,
    /// Matching markers with a proper name.
    pub named: Vec<usize>,
    pub categories: Vec<CategorySummary>,
}

#[derive(Debug)]
pub struct CategorySummary {
    pub id: String,
    pub label: String,
    pub count: usize,
    pub shown: bool,
    /// `(id, label, count, shown)`.
    pub subcategories: Vec<(String, String, usize, bool)>,
}

impl MapSummary {
    pub fn new(markers: &[Marker], query: &str, filter: &MarkerFilter) -> Self {
        let matching: Vec<usize> = (0..markers.len())
            .filter(|&i| markers[i].matches(query))
            .collect();
        let visible = matching
            .iter()
            .copied()
            .filter(|&i| filter.shows(&markers[i]))
            .collect();
        let named = matching
            .iter()
            .copied()
            .filter(|&i| markers[i].label.is_some())
            .collect();
        let subset: Vec<Marker> = matching.iter().map(|&i| markers[i].clone()).collect();
        let categories = marker_counts(&subset)
            .into_iter()
            .map(|(category, subs)| CategorySummary {
                id: category.to_owned(),
                label: humanize(category),
                count: subs.values().sum(),
                shown: filter.shows_category(category),
                subcategories: subs
                    .into_iter()
                    .filter(|(sub, _)| !sub.is_empty())
                    .map(|(sub, count)| {
                        (
                            sub.to_owned(),
                            humanize(sub),
                            count,
                            filter.shows_subcategory(category, sub),
                        )
                    })
                    .collect(),
            })
            .collect();
        Self {
            matching,
            visible,
            named,
            categories,
        }
    }
}

pub struct MapView<'a> {
    /// `(id, name)` of every map.
    pub maps: &'a [(&'static str, &'static str)],
    pub selected: &'a str,
    pub markers: Markers<'a>,
    pub summary: &'a MapSummary,
    /// Cached marker layer of the plot; the app clears it on changes.
    pub plot: &'a canvas::Cache,
    pub query: &'a str,
    /// Categories opened to show their subcategories.
    pub expanded: &'a BTreeSet<String>,
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
                    summary: map.summary,
                    cache: map.plot,
                    searching: !map.query.trim().is_empty(),
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

    column![container(pills).padding([12, 16]), body]
        .height(Length::Fill)
        .into()
}

/// Left panel: search, show/hide all, categories with icons and counts.
fn panel<'a>(map: &MapView<'a>, markers: &'a [Marker]) -> Element<'a, Message> {
    let summary = map.summary;
    let searching = !map.query.trim().is_empty();

    let mut categories = column![].spacing(2);
    for category in &summary.categories {
        let open = searching || map.expanded.contains(&category.id);
        let has_subs = !category.subcategories.is_empty();
        categories = categories.push(category_row(category, has_subs.then_some(open)));
        if open {
            for (sub, label, count, shown) in &category.subcategories {
                let (c, s) = (category.id.clone(), sub.clone());
                categories = categories.push(
                    row![
                        Space::new().width(20),
                        badge(&category.id, Some(sub), 18.0),
                        checkbox(*shown)
                            .label(label.clone())
                            .text_size(13)
                            .size(14)
                            .on_toggle(move |_| Message::ToggleMarkerSubcategory(
                                c.clone(),
                                s.clone()
                            ))
                            .width(Length::Fill),
                        text(count.to_string()).size(12).color(palette::TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .padding([2, 8]),
                );
            }
        }
    }

    let header = row![
        text(format!(
            "{} OF {} MARKERS SHOWN",
            summary.visible.len(),
            summary.matching.len()
        ))
        .size(11)
        .color(palette::TEXT_MUTED)
        .width(Length::Fill),
        small_button("Show all", Message::ShowAllMarkers),
        small_button("Hide all", Message::HideAllMarkers),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // Named places only: unnamed markers are covered by the counts above.
    let matches = (searching && !summary.named.is_empty()).then(|| {
        summary
            .named
            .iter()
            .take(50)
            .fold(column![].spacing(4), |col, &i| {
                let m = &markers[i];
                col.push(
                    row![
                        badge(&m.category, m.subcategory.as_deref(), 18.0),
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

/// A category: icon, toggle, name, count and (if it has subcategories) an
/// expander.
fn category_row<'a>(category: &CategorySummary, open: Option<bool>) -> Element<'a, Message> {
    let name = category.id.clone();
    let toggle = checkbox(category.shown)
        .label(category.label.clone())
        .text_size(14)
        .size(16)
        .on_toggle(move |_| Message::ToggleMarkerCategory(name.clone()))
        .width(Length::Fill);
    let expander: Element<'a, Message> = match open {
        Some(open) => button(text(if open { "▾" } else { "▸" }).size(13))
            .padding([0, 6])
            .on_press(Message::ExpandMarkerCategory(category.id.clone()))
            .style(|_, _| button::Style {
                text_color: palette::TEXT_MUTED,
                ..button::Style::default()
            })
            .into(),
        None => Space::new().width(22).into(),
    };
    row![
        badge(&category.id, None, 22.0),
        toggle,
        text(category.count.to_string())
            .size(12)
            .color(palette::TEXT_MUTED),
        expander,
    ]
    .spacing(8)
    .padding([4, 8])
    .align_y(Alignment::Center)
    .into()
}

/// The map's markers as icons, fitted to the canvas. The marker layer is
/// cached; only the hover tooltip is drawn per frame. A background map
/// image comes once its alignment with the marker coordinates is known.
struct Plot<'a> {
    markers: &'a [Marker],
    summary: &'a MapSummary,
    cache: &'a canvas::Cache,
    searching: bool,
}

impl Plot<'_> {
    fn draw_markers(&self, frame: &mut Frame, fit: &Fit) {
        // `visible` already holds only search matches the filter shows.
        let visible: Vec<&Marker> = self
            .summary
            .visible
            .iter()
            .map(|&i| &self.markers[i])
            .collect();
        for marker in visible.iter().filter(|m| !is_named_place(m)) {
            let at = fit.point(marker);
            frame.fill(
                &Path::circle(at, ICON / 2.0),
                palette::marker(&marker.category),
            );
            let inner = ICON * 0.62;
            frame.draw_svg(
                Rectangle::new(
                    Point::new(at.x - inner / 2.0, at.y - inner / 2.0),
                    Size::new(inner, inner),
                ),
                &handle(glyph(&marker.category, marker.subcategory.as_deref())),
            );
            if self.searching {
                // Ring the matches so they stand out.
                frame.stroke(
                    &Path::circle(at, ICON / 2.0 + 1.5),
                    Stroke::default().with_color(Color::WHITE).with_width(1.5),
                );
            }
        }
        // Names of places, on top of the icons.
        for marker in visible.iter().filter(|m| is_named_place(m)) {
            let at = fit.point(marker);
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
            // A dark outline keeps names readable over icons.
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                frame.fill_text(label(Point::new(at.x + dx, at.y + dy), Color::BLACK));
            }
            frame.fill_text(label(at, palette::TEXT));
        }
    }
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
        let fit = Fit::new(self.markers, bounds.size());
        let markers = self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill(
                &Path::rounded_rectangle(Point::ORIGIN, bounds.size(), 8.0.into()),
                Color::from_rgb8(0x12, 0x15, 0x1a),
            );
            if let Some(fit) = &fit {
                self.draw_markers(frame, fit);
            }
        });
        let mut geometry = vec![markers];

        // Tooltip for the marker under the cursor (not cached).
        if let (Some(fit), Some(cursor)) = (&fit, cursor.position_in(bounds))
            && let Some((marker, at)) = self
                .summary
                .visible
                .iter()
                .map(|&i| {
                    let m = &self.markers[i];
                    let at = fit.point(m);
                    (m, at, at.distance(cursor))
                })
                .filter(|(_, _, d)| *d <= ICON / 2.0 + 2.0)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(m, at, _)| (m, at))
        {
            let mut frame = Frame::new(renderer, bounds.size());
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
                position: Point::new(at.x + ICON / 2.0 + 6.0, at.y),
                color: Color::WHITE,
                size: 13.0.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
            geometry.push(frame.into_geometry());
        }
        geometry
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
