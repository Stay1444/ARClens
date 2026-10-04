//! The Map tab: every marker of a map, searchable, with per-kind toggles
//! and icons. The same filter decides what the overlay shows in game.

use crate::app::Message;
use arclens_core::{Marker, MarkerFilter, Preset, humanize, marker_counts};
use arclens_ui::markers::{badge, draw_area, draw_badge};
use arclens_ui::palette;
use arclens_ui::theme::{self, size, space};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{
    Canvas, Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};
use std::collections::BTreeSet;

const PANEL_WIDTH: f32 = 400.0;
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
    /// `visible`, with dense same-kind groups merged into areas.
    pub layout: arclens_core::MarkerLayout,
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
    /// `condition`: the bit of the map's current condition; markers of
    /// other conditions are left out.
    pub fn new(
        markers: &[Marker],
        query: &str,
        filter: &MarkerFilter,
        condition: Option<u8>,
    ) -> Self {
        let matching: Vec<usize> = (0..markers.len())
            .filter(|&i| markers[i].occurs_in(condition) && markers[i].matches(query))
            .collect();
        let visible: Vec<usize> = matching
            .iter()
            .copied()
            .filter(|&i| filter.shows(&markers[i]))
            .collect();
        let named = matching
            .iter()
            .copied()
            .filter(|&i| markers[i].label.is_some())
            .collect();
        let categories = marker_counts(matching.iter().map(|&i| &markers[i]))
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
        let layout = arclens_core::layout(markers, visible.iter().copied());
        Self {
            matching,
            visible,
            named,
            categories,
            layout,
        }
    }
}

/// The map's own image behind the markers: its handle (decoded once) and
/// its corners in marker coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Background<'a> {
    pub handle: &'a iced::widget::image::Handle,
    pub min: arclens_core::MapPoint,
    pub max: arclens_core::MapPoint,
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
    /// The selected map's conditions, and the one markers are shown for.
    pub conditions: &'a [(&'static str, u8)],
    pub condition: Option<&'static str>,
    pub presets: PresetsView<'a>,
    /// The map image, once loaded (not every map has one).
    pub background: Option<Background<'a>>,
}

/// The preset section of the panel.
pub struct PresetsView<'a> {
    /// Presets suited to the map and condition, best first.
    pub suited: Vec<&'a Preset>,
    pub presets: &'a crate::presets::Presets,
}

pub fn view<'a>(map: &MapView<'a>) -> Element<'a, Message> {
    let body: Element<'a, Message> = match map.markers {
        Markers::Loading => centered(
            text("Loading markers…")
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        ),
        Markers::Failed(error) => centered(
            column![
                theme::heading("Could not load markers", size::H1),
                text(error).size(size::BODY).color(palette::TEXT_MUTED),
            ]
            .spacing(space::GAP)
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
                    background: map.background,
                })
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(space::SECTION)
        .height(Length::Fill)
        .into(),
    };

    column![top_bar(map), body]
        .spacing(space::SECTION)
        .padding(theme::PAGE_PADDING)
        .height(Length::Fill)
        .into()
}

/// Map chips, then (if the map has any) its conditions.
fn top_bar<'a>(map: &MapView<'a>) -> Element<'a, Message> {
    let maps = map
        .maps
        .iter()
        .fold(row![].spacing(space::GAP / 2.0), |r, &(id, name)| {
            r.push(pill(
                name,
                id == map.selected,
                Message::SelectMap(id.to_owned()),
            ))
        })
        .wrap();

    let conditions = (!map.conditions.is_empty()).then(|| {
        map.conditions
            .iter()
            .fold(
                row![
                    container(theme::label("Condition")).width(100),
                    pill(
                        "Any",
                        map.condition.is_none(),
                        Message::SelectCondition(None)
                    ),
                ]
                .spacing(space::GAP / 2.0)
                .align_y(Alignment::Center),
                |r, &(name, _)| {
                    r.push(pill(
                        name,
                        map.condition == Some(name),
                        Message::SelectCondition(Some(name)),
                    ))
                },
            )
            .wrap()
    });

    column![
        row![
            container(theme::heading("Map", size::TITLE)).padding([0.0, space::GAP]),
            maps
        ]
        .spacing(space::GAP)
        .align_y(Alignment::Center),
    ]
    .push(conditions)
    .spacing(space::GAP)
    .into()
}

/// Left side: presets, search, then the categories with icons and counts.
fn panel<'a>(map: &MapView<'a>, markers: &'a [Marker]) -> Element<'a, Message> {
    let summary = map.summary;
    let searching = !map.query.trim().is_empty();

    let count = text(format!(
        "{} / {} shown",
        summary.visible.len(),
        summary.matching.len()
    ))
    .size(size::SMALL)
    .font(theme::DISPLAY_SEMI)
    .color(theme::INK);

    let actions = row![
        small_button("Show all", Message::ShowAllMarkers),
        small_button("Hide all", Message::HideAllMarkers),
    ]
    .spacing(space::GAP / 2.0);

    let list = scrollable(
        column![categories(map)]
            .push(matches(summary, markers, searching))
            .spacing(space::SECTION)
            .padding(iced::Padding::default().right(space::GAP)),
    )
    .height(Length::Fill);

    column![
        theme::panel("Preset", None, presets(&map.presets, map.condition)),
        text_input("Search markers…", map.query)
            .id(SEARCH_ID)
            .on_input(Message::MarkerQuery)
            .padding([10, 16])
            .size(size::BODY)
            .style(theme::input_style),
        theme::panel(
            "Markers",
            Some(count.into()),
            column![actions, list].spacing(space::GAP)
        ),
    ]
    .spacing(space::GAP)
    .width(PANEL_WIDTH)
    .height(Length::Fill)
    .into()
}

/// Every category, opened ones (or all, while searching) with their
/// subcategories.
fn categories<'a>(map: &MapView<'a>) -> Element<'a, Message> {
    let searching = !map.query.trim().is_empty();
    let mut list = column![].spacing(4);
    for category in &map.summary.categories {
        let open = searching || map.expanded.contains(&category.id);
        let has_subs = !category.subcategories.is_empty();
        list = list.push(category_row(category, has_subs.then_some(open)));
        if open {
            for (sub, label, count, shown) in &category.subcategories {
                list = list.push(subcategory_row(&category.id, sub, label, *count, *shown));
            }
        }
    }
    list.into()
}

fn subcategory_row<'a>(
    category: &str,
    sub: &str,
    label: &str,
    count: usize,
    shown: bool,
) -> Element<'a, Message> {
    let (c, s) = (category.to_owned(), sub.to_owned());
    row![
        Space::new().width(28),
        badge(category, Some(sub), 20.0),
        checkbox(shown)
            .label(label.to_owned())
            .text_size(size::SMALL)
            .size(16)
            .on_toggle(move |_| Message::ToggleMarkerSubcategory(c.clone(), s.clone()))
            .width(Length::Fill),
        text(count.to_string())
            .size(size::TINY)
            .color(palette::TEXT_MUTED),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([3, 4])
    .into()
}

/// Named places matching the search; unnamed markers are covered by the
/// category counts.
fn matches<'a>(
    summary: &MapSummary,
    markers: &'a [Marker],
    searching: bool,
) -> Option<Element<'a, Message>> {
    (searching && !summary.named.is_empty()).then(|| {
        let list = summary
            .named
            .iter()
            .take(50)
            .fold(column![].spacing(8), |col, &i| {
                let m = &markers[i];
                col.push(
                    row![
                        badge(&m.category, m.subcategory.as_deref(), 20.0),
                        text(m.title())
                            .size(size::SMALL)
                            .font(theme::STRONG)
                            .width(Length::Fill),
                        text(humanize(&m.category))
                            .size(size::TINY)
                            .color(palette::TEXT_MUTED),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
            });
        column![theme::label("Matches"), list]
            .spacing(space::GAP / 2.0)
            .into()
    })
}

/// Preset chips, then saving: update the active one, or save as new.
fn presets<'a>(view: &PresetsView<'a>, condition: Option<&'static str>) -> Element<'a, Message> {
    let book = view.presets;
    let active = book.active();
    let pills = view
        .suited
        .iter()
        .fold(row![].spacing(space::GAP / 2.0), |r, p| {
            let active = active.is_some_and(|a| a.id == p.id);
            let label = if active && book.edited {
                format!("{} *", p.name)
            } else {
                p.name.clone()
            };
            r.push(pill(&label, active, Message::ApplyPreset(p.id.clone())))
        })
        .wrap();

    let description = active
        .map(|p| p.description.as_str())
        .filter(|d| !d.is_empty())
        .map(|d| text(d).size(size::SMALL).color(palette::TEXT_MUTED));

    let mut actions = row![].spacing(space::GAP / 2.0).align_y(Alignment::Center);
    if let Some(active) = active {
        if book.edited {
            actions = actions.push(small_button(
                &format!("Save to \"{}\"", active.name),
                Message::UpdatePreset,
            ));
        }
        if book.is_customised(&active.id) {
            actions = actions.push(small_button(
                if crate::presets::Presets::is_builtin(&active.id) {
                    "Reset to default"
                } else {
                    "Delete"
                },
                Message::DeletePreset(active.id.clone()),
            ));
        }
    }

    let save_as = row![
        text_input("New preset name…", &book.draft)
            .on_input(Message::PresetDraft)
            .on_submit(Message::SavePresetAs)
            .padding([10, 16])
            .size(size::SMALL)
            .style(theme::input_style),
        small_button("Save as new", Message::SavePresetAs),
    ]
    .spacing(space::GAP / 2.0)
    .align_y(Alignment::Center);

    column![pills]
        .push(description)
        .push(actions)
        .push(save_as)
        .push(preset_scope(book, condition))
        .spacing(space::GAP)
        .into()
}

/// Where a saved preset applies: this map, this condition.
fn preset_scope<'a>(
    book: &crate::presets::Presets,
    condition: Option<&'static str>,
) -> Element<'a, Message> {
    let mut scope = row![
        checkbox(book.for_map)
            .label("This map only")
            .text_size(size::SMALL)
            .size(16)
            .on_toggle(Message::PresetForMap)
    ]
    .spacing(space::GAP);
    if let Some(condition) = condition {
        scope = scope.push(
            checkbox(book.for_condition)
                .label(format!("{condition} only"))
                .text_size(size::SMALL)
                .size(16)
                .on_toggle(Message::PresetForCondition),
        );
    }
    scope.wrap().into()
}

/// A category: icon, toggle, name, count and (if it has subcategories) an
/// expander.
fn category_row<'a>(category: &CategorySummary, open: Option<bool>) -> Element<'a, Message> {
    let name = category.id.clone();
    let toggle = checkbox(category.shown)
        .label(category.label.clone())
        .text_size(size::BODY)
        .font(theme::STRONG)
        .size(18)
        .on_toggle(move |_| Message::ToggleMarkerCategory(name.clone()))
        .width(Length::Fill);
    let expander: Element<'a, Message> = match open {
        Some(open) => button(text(if open { "▾" } else { "▸" }).size(size::BODY))
            .padding([2, 8])
            .on_press(Message::ExpandMarkerCategory(category.id.clone()))
            .style(|_, status| button::Style {
                text_color: if matches!(status, button::Status::Hovered) {
                    theme::ACCENT
                } else {
                    palette::TEXT_MUTED
                },
                ..button::Style::default()
            })
            .into(),
        None => Space::new().width(28).into(),
    };
    row![
        badge(&category.id, None, 26.0),
        toggle,
        text(category.count.to_string())
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
        expander,
    ]
    .spacing(10)
    .padding([5, 4])
    .align_y(Alignment::Center)
    .into()
}

/// The map's markers as icons over the map's image, fitted to the canvas;
/// the wheel zooms about the pointer, dragging pans, a double click resets.
/// The marker layer is cached; only the hover tooltip is drawn per frame.
struct Plot<'a> {
    markers: &'a [Marker],
    summary: &'a MapSummary,
    cache: &'a canvas::Cache,
    searching: bool,
    background: Option<Background<'a>>,
}

/// The plot's zoom and pan (the canvas's own state).
#[derive(Debug, Clone, Copy)]
pub struct PlotView {
    zoom: f32,
    pan: iced::Vector,
    /// Pointer position when a drag started, and the pan then.
    drag: Option<(Point, iced::Vector)>,
    last_click: Option<std::time::Instant>,
}

impl Default for PlotView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: iced::Vector::ZERO,
            drag: None,
            last_click: None,
        }
    }
}

impl PlotView {
    const MAX_ZOOM: f32 = 12.0;

    /// Zooms by `factor` keeping the point under `at` (canvas coordinates)
    /// in place.
    fn zoom_at(&mut self, factor: f32, at: Point, size: Size) {
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let zoom = (self.zoom * factor).clamp(1.0, Self::MAX_ZOOM);
        // The unzoomed point under the cursor stays under it.
        let base = center + (at - center - self.pan) * (1.0 / self.zoom);
        self.pan = at - center - (base - center) * zoom;
        self.zoom = zoom;
        if (zoom - 1.0).abs() < f32::EPSILON {
            self.pan = iced::Vector::ZERO;
        }
    }
}

impl Plot<'_> {
    fn draw_markers(&self, frame: &mut Frame, fit: &Fit) {
        // Areas first, under the single markers.
        for area in &self.summary.layout.areas {
            draw_area(frame, area, |p| fit.map_point(p), ICON);
        }
        let visible: Vec<&Marker> = self
            .summary
            .layout
            .singles
            .iter()
            .map(|&i| &self.markers[i])
            .collect();
        for marker in visible.iter().filter(|m| !is_named_place(m)) {
            let at = fit.point(marker);
            draw_badge(
                frame,
                &marker.category,
                marker.subcategory.as_deref(),
                at,
                ICON,
            );
            if self.searching {
                // Ring the matches so they stand out.
                frame.stroke(
                    &Path::circle(at, ICON / 2.0 + 1.5),
                    Stroke::default().with_color(theme::ACCENT).with_width(2.0),
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
                size: 15.0.into(),
                font: theme::DISPLAY,
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
    type State = PlotView;

    fn draw(
        &self,
        state: &PlotView,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let fit = Fit::new(
            self.markers,
            self.background.map(|b| (b.min, b.max)),
            bounds.size(),
            state,
        );
        let markers = self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill(
                &Path::rounded_rectangle(Point::ORIGIN, bounds.size(), theme::RADIUS.into()),
                theme::PANEL,
            );
            if let Some(fit) = &fit {
                if let Some(background) = self.background {
                    let (a, b) = (fit.map_point(background.min), fit.map_point(background.max));
                    frame.draw_image(
                        Rectangle::new(a, Size::new(b.x - a.x, b.y - a.y)),
                        canvas::Image::new(background.handle.clone()).opacity(0.75_f32),
                    );
                }
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
                size: 16.0.into(),
                font: theme::STRONG,
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
            geometry.push(frame.into_geometry());
        }
        geometry
    }

    /// Zoom, pan and the hover tooltip.
    fn update(
        &self,
        state: &mut PlotView,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let at = cursor.position_in(bounds);
        let changed = match (event, at) {
            (iced::Event::Mouse(mouse::Event::WheelScrolled { delta }), Some(at)) => {
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
                };
                state.zoom_at(1.2_f32.powf(lines), at, bounds.size());
                true
            }
            (iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)), Some(at)) => {
                let now = std::time::Instant::now();
                let double = state
                    .last_click
                    .is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(350));
                state.last_click = Some(now);
                if double {
                    *state = PlotView::default();
                    true
                } else {
                    state.drag = Some((at, state.pan));
                    false
                }
            }
            (iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)), _) => {
                state.drag = None;
                false
            }
            (iced::Event::Mouse(mouse::Event::CursorMoved { .. }), Some(at)) => {
                if let Some((from, pan)) = state.drag
                    && state.zoom > 1.0
                {
                    state.pan = pan + (at - from);
                    true
                } else {
                    return Some(canvas::Action::request_redraw());
                }
            }
            _ => false,
        };
        changed.then(|| {
            self.cache.clear();
            canvas::Action::request_redraw().and_capture()
        })
    }
}

/// Named places are drawn as their name instead of a dot.
fn is_named_place(marker: &Marker) -> bool {
    let c = marker.category.to_lowercase();
    marker.label.is_some() && ["label", "zone", "poi"].iter().any(|w| c.contains(w))
}

/// Uniform scale + offset fitting the markers (else the map image) into the
/// canvas (aspect kept), then the user's zoom and pan.
struct Fit {
    scale: f32,
    min: Point,
    offset: Point,
}

impl Fit {
    const MARGIN: f32 = 24.0;

    fn new(
        markers: &[Marker],
        image: Option<(arclens_core::MapPoint, arclens_core::MapPoint)>,
        size: Size,
        view: &PlotView,
    ) -> Option<Self> {
        let (mut min, mut max) = (
            Point::new(f32::MAX, f32::MAX),
            Point::new(f32::MIN, f32::MIN),
        );
        // The markers' extent (the playable area; the image around it is
        // mostly scenery), else the image's.
        for m in markers {
            min = Point::new(min.x.min(m.position.x), min.y.min(m.position.y));
            max = Point::new(max.x.max(m.position.x), max.y.max(m.position.y));
        }
        if let (true, Some((a, b))) = (markers.is_empty(), image) {
            (min, max) = (Point::new(a.x, a.y), Point::new(b.x, b.y));
        }
        let (w, h) = ((max.x - min.x).max(1.0), (max.y - min.y).max(1.0));
        let avail = Size::new(
            size.width - 2.0 * Self::MARGIN,
            size.height - 2.0 * Self::MARGIN,
        );
        if (markers.is_empty() && image.is_none()) || avail.width <= 0.0 || avail.height <= 0.0 {
            return None;
        }
        let scale = (avail.width / w).min(avail.height / h);
        let offset = Point::new(
            Self::MARGIN + (avail.width - w * scale) / 2.0,
            Self::MARGIN + (avail.height - h * scale) / 2.0,
        );
        // Zoom about the canvas centre, then pan.
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let offset = center + (offset - center) * view.zoom + view.pan;
        Some(Self {
            scale: scale * view.zoom,
            min,
            offset,
        })
    }

    fn point(&self, marker: &Marker) -> Point {
        self.map_point(marker.position)
    }

    fn map_point(&self, p: arclens_core::MapPoint) -> Point {
        Point::new(
            self.offset.x + (p.x - self.min.x) * self.scale,
            self.offset.y + (p.y - self.min.y) * self.scale,
        )
    }
}

fn pill<'a>(label: &str, active: bool, on_press: Message) -> Element<'a, Message> {
    theme::chip(label, active, on_press)
}

fn small_button<'a>(label: &str, on_press: Message) -> Element<'a, Message> {
    theme::secondary_button(label, Some(on_press))
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
