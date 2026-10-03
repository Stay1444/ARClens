//! The panel shown while the in-game map is open: which markers to show,
//! by kind, with their icons. It is the only clickable part of the
//! overlay; its rectangle becomes the surface's input region.

use crate::Message;
use arclens_ipc::{MapPanel, PanelCategory, ToApp};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Element, Font, Length, Rectangle, Size, font};

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

/// Where the panel sits, as `[x, y, width, height]` fractions of the screen.
/// Collapsed it fits under the game's map legend (right column, below
/// ~72 % of the height); expanded it covers the legend.
const COLLAPSED: [f32; 4] = [0.78, 0.73, 0.19, 0.11];
const EXPANDED: [f32; 4] = [0.78, 0.20, 0.19, 0.72];

/// Local UI state of the panel (the data itself comes from the app).
#[derive(Debug, Default)]
pub struct PanelState {
    pub panel: Option<MapPanel>,
    pub expanded: bool,
    pub query: String,
    /// Categories opened to show their subcategories.
    pub open: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum PanelMessage {
    ToggleExpanded,
    Query(String),
    Open(String),
    /// Forwarded to the app.
    Send(ToApp),
}

impl PanelState {
    /// The panel's rectangle on a `screen`-sized surface, while shown.
    pub fn bounds(&self, screen: Size) -> Option<Rectangle> {
        self.panel.as_ref()?;
        let [x, y, w, h] = if self.expanded { EXPANDED } else { COLLAPSED };
        Some(Rectangle {
            x: (x * screen.width).round(),
            y: (y * screen.height).round(),
            width: (w * screen.width).round(),
            height: (h * screen.height).round(),
        })
    }

    /// Applies a local message; returns what to send to the app, if any.
    pub fn update(&mut self, message: PanelMessage) -> Option<ToApp> {
        match message {
            PanelMessage::ToggleExpanded => self.expanded = !self.expanded,
            PanelMessage::Query(query) => self.query = query,
            PanelMessage::Open(category) => {
                if !self.open.remove(&category) {
                    self.open.insert(category);
                }
            }
            PanelMessage::Send(msg) => return Some(msg),
        }
        None
    }
}

/// The panel, positioned on a `screen`-sized surface.
pub fn view(state: &PanelState, screen: Size) -> Option<Element<'_, Message>> {
    let panel = state.panel.as_ref()?;
    let bounds = state.bounds(screen)?;

    let total: usize = panel.categories.iter().map(|c| c.count).sum();
    let shown: usize = panel.categories.iter().map(shown_count).sum();
    let toggle = expand_button(
        format!("Markers · {shown} of {total} shown"),
        state.expanded,
    );

    let mut content = column![
        row![
            text(panel.map_name.to_uppercase())
                .size(13)
                .font(BOLD)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill),
            text("ARClens").size(10).color(palette::TEXT_MUTED),
        ]
        .align_y(Alignment::Center),
        toggle,
    ]
    .spacing(8);

    if state.expanded {
        content = content.push(filter(state, panel));
    }

    let card = container(content)
        .padding(12)
        .width(bounds.width)
        .height(bounds.height)
        .style(|_| container::Style {
            // Opaque: the game's legend sits behind when expanded.
            background: Some(with_alpha(palette::SURFACE, 1.0).into()),
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: 10.0.into(),
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });
    Some(
        container(card)
            .padding(iced::Padding {
                top: bounds.y,
                left: bounds.x,
                ..iced::Padding::ZERO
            })
            .into(),
    )
}

fn expand_button<'a>(label: String, expanded: bool) -> Element<'a, Message> {
    button(
        row![
            text(label).size(13).width(Length::Fill),
            text(if expanded { "▾" } else { "▸" }).size(13),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([6, 10])
    .on_press(Message::Panel(PanelMessage::ToggleExpanded))
    .style(|_, status| button::Style {
        background: Some(
            with_alpha(
                palette::TEXT,
                if matches!(status, button::Status::Hovered) {
                    0.14
                } else {
                    0.07
                },
            )
            .into(),
        ),
        text_color: palette::TEXT,
        border: Border {
            color: palette::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    })
    .into()
}

fn shown_count(category: &PanelCategory) -> usize {
    match (category.shown, category.subcategories.is_empty()) {
        (false, _) => 0,
        (true, true) => category.count,
        (true, false) => category
            .subcategories
            .iter()
            .filter(|s| s.shown)
            .map(|s| s.count)
            .sum(),
    }
}

/// Search, show/hide all, and the category list.
fn filter<'a>(state: &'a PanelState, panel: &'a MapPanel) -> Element<'a, Message> {
    let query = state.query.trim().to_lowercase();
    let matches = |label: &str| query.is_empty() || label.to_lowercase().contains(&query);

    let mut list = column![].spacing(2);
    for category in &panel.categories {
        let subs: Vec<&PanelCategory> = category
            .subcategories
            .iter()
            .filter(|s| matches(&s.label) || matches(&category.label))
            .collect();
        if !matches(&category.label) && subs.is_empty() {
            continue;
        }
        let open = !query.is_empty() || state.open.contains(&category.id);
        let id = category.id.clone();
        list = list.push(
            row![
                arclens_ui::markers::badge(&category.id, None, 20.0),
                checkbox(category.shown)
                    .label(category.label.clone())
                    .size(14)
                    .text_size(13)
                    .on_toggle(move |_| send(ToApp::ToggleMarkerCategory {
                        category: id.clone()
                    }))
                    .width(Length::Fill),
                text(category.count.to_string())
                    .size(11)
                    .color(palette::TEXT_MUTED),
                expander(category, open),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
        if open {
            for sub in subs {
                let (c, s) = (category.id.clone(), sub.id.clone());
                list = list.push(
                    row![
                        Space::new().width(10),
                        arclens_ui::markers::badge(&category.id, Some(&sub.id), 16.0),
                        checkbox(sub.shown)
                            .label(sub.label.clone())
                            .size(12)
                            .text_size(12)
                            .on_toggle(move |_| send(ToApp::ToggleMarkerSubcategory {
                                category: c.clone(),
                                subcategory: s.clone(),
                            }))
                            .width(Length::Fill),
                        text(sub.count.to_string())
                            .size(11)
                            .color(palette::TEXT_MUTED),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                );
            }
        }
    }

    column![
        text_input("Search markers…", &state.query)
            .on_input(|q| Message::Panel(PanelMessage::Query(q)))
            .padding([5, 8])
            .size(13),
        row![
            small_button("Show all", send(ToApp::ShowAllMarkers)),
            small_button("Hide all", send(ToApp::HideAllMarkers)),
        ]
        .spacing(6),
        scrollable(list.padding(iced::Padding {
            right: 10.0,
            ..iced::Padding::ZERO
        }))
        .height(Length::Fill),
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn send(msg: ToApp) -> Message {
    Message::Panel(PanelMessage::Send(msg))
}

fn expander<'a>(category: &PanelCategory, open: bool) -> Element<'a, Message> {
    if category.subcategories.is_empty() {
        return Space::new().width(18).into();
    }
    button(text(if open { "▾" } else { "▸" }).size(12))
        .padding([0, 4])
        .on_press(Message::Panel(PanelMessage::Open(category.id.clone())))
        .style(|_, _| button::Style {
            text_color: palette::TEXT_MUTED,
            ..button::Style::default()
        })
        .into()
}

fn small_button(label: &str, on_press: Message) -> Element<'_, Message> {
    button(text(label).size(11))
        .padding([3, 8])
        .on_press(on_press)
        .style(|_, status| button::Style {
            background: Some(
                with_alpha(
                    palette::TEXT,
                    if matches!(status, button::Status::Hovered) {
                        0.14
                    } else {
                        0.07
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

#[cfg(test)]
mod tests {
    use super::*;

    fn state(expanded: bool) -> PanelState {
        PanelState {
            panel: Some(MapPanel {
                map_name: "Dam Battlegrounds".into(),
                categories: Vec::new(),
            }),
            expanded,
            ..PanelState::default()
        }
    }

    #[test]
    fn bounds_follow_the_screen_and_expansion() {
        let screen = Size::new(2560.0, 1440.0);
        let collapsed = state(false).bounds(screen).unwrap();
        assert_eq!((collapsed.x, collapsed.y), (1997.0, 1051.0));
        let expanded = state(true).bounds(screen).unwrap();
        assert!(expanded.y < collapsed.y && expanded.height > collapsed.height);
        // Stays on screen.
        assert!(expanded.y + expanded.height <= screen.height);
        assert!(PanelState::default().bounds(screen).is_none());
    }

    #[test]
    fn counts_only_shown_markers() {
        let sub = |shown| PanelCategory {
            id: "x".into(),
            label: "X".into(),
            count: 2,
            shown,
            subcategories: Vec::new(),
        };
        let arc = PanelCategory {
            subcategories: vec![sub(true), sub(false)],
            count: 4,
            ..sub(true)
        };
        assert_eq!(shown_count(&arc), 2);
        assert_eq!(
            shown_count(&PanelCategory {
                shown: false,
                ..arc
            }),
            0
        );
        assert_eq!(shown_count(&sub(true)), 2);
    }
}
