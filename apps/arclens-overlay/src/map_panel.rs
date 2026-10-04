//! The panel shown while the in-game map is open: which markers to show,
//! by kind, with their icons. It is the only clickable part of the
//! overlay; its rectangle becomes the surface's input region.

use crate::Message;
use arclens_ipc::{MapPanel, PanelCategory, ToApp};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, CREAM, DISPLAY, DISPLAY_SEMI, INK, RADIUS, STRONG};
use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Element, Length, Rectangle, Size};

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
    /// The pointer is over the panel: only then may it take the keyboard
    /// (for the search box). Taking it while the game is in front costs
    /// the game its focus, and with it audio and input.
    pub hovered: bool,
    /// Categories opened to show their subcategories.
    pub open: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub enum PanelMessage {
    ToggleExpanded,
    Query(String),
    /// Pointer entered (`true`) or left the panel.
    Hover(bool),
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
            PanelMessage::Hover(hovered) => self.hovered = hovered,
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
        format!("MARKERS · {shown} OF {total} SHOWN"),
        state.expanded,
    );

    let title = match &panel.condition {
        Some(condition) => format!("{} · {condition}", panel.map_name),
        None => panel.map_name.clone(),
    };
    let header = container(
        row![
            text(title.to_uppercase())
                .size(17)
                .font(DISPLAY)
                .color(INK)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill),
            text("ARCLENS")
                .size(11)
                .font(DISPLAY)
                .color(with_alpha(INK, 0.55)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(CREAM.into()),
        border: Border {
            radius: iced::border::Radius::default().top(RADIUS),
            ..Border::default()
        },
        ..container::Style::default()
    });

    let mut content = column![]
        .push(preset_switcher(panel))
        .push(toggle)
        .spacing(6);
    if state.expanded {
        content = content.push(filter(state, panel));
    }
    // Opaque when expanded: the game's legend sits behind it then.
    let alpha = if state.expanded { 1.0 } else { 0.94 };
    let body = container(content)
        .padding([8, 12])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(with_alpha(theme::PANEL, alpha).into()),
            border: Border {
                radius: iced::border::Radius::default().bottom(RADIUS),
                ..Border::default()
            },
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        });

    let card = container(column![header, body])
        .width(bounds.width)
        .height(bounds.height)
        .style(|_| container::Style {
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        });
    let card = iced::widget::mouse_area(card)
        .on_enter(Message::Panel(PanelMessage::Hover(true)))
        .on_exit(Message::Panel(PanelMessage::Hover(false)));
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

/// `◂ First Wave caches ▸`: steps through the presets suited to the map
/// and condition.
fn preset_switcher(panel: &MapPanel) -> Option<Element<'_, Message>> {
    if panel.presets.is_empty() {
        return None;
    }
    let active = panel
        .presets
        .iter()
        .find(|p| Some(&p.id) == panel.active_preset.as_ref());
    let name = active.map_or("Custom", |p| p.name.as_str());
    let label = if panel.edited && active.is_some() {
        format!("{name} *")
    } else {
        name.to_owned()
    };
    let step = |by| cycle(panel, by).map(|id| send(ToApp::ApplyPreset { id }));
    let arrow = |glyph, by| {
        button(text(glyph).size(15).font(STRONG))
            .padding([2, 10])
            .on_press_maybe(step(by))
            .style(dark_button)
    };
    Some(
        row![
            arrow("◂", -1),
            column![
                text("PRESET")
                    .size(11)
                    .font(DISPLAY_SEMI)
                    .color(palette::TEXT_MUTED),
                text(label.to_uppercase())
                    .size(16)
                    .font(DISPLAY)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill),
            arrow("▸", 1),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into(),
    )
}

/// The preset `by` steps from the active one (wrapping); from no or an
/// unlisted preset, forward starts at the first and back at the last.
fn cycle(panel: &MapPanel, by: isize) -> Option<String> {
    let n = isize::try_from(panel.presets.len())
        .ok()
        .filter(|&n| n > 0)?;
    let current = panel
        .presets
        .iter()
        .position(|p| Some(&p.id) == panel.active_preset.as_ref())
        .and_then(|i| isize::try_from(i).ok());
    let next = match current {
        Some(i) => (i + by).rem_euclid(n),
        None if by > 0 => 0,
        None => n - 1,
    };
    let next = usize::try_from(next).ok()?;
    Some(panel.presets[next].id.clone())
}

fn expand_button<'a>(label: String, expanded: bool) -> Element<'a, Message> {
    button(
        row![
            text(label).size(14).font(DISPLAY_SEMI).width(Length::Fill),
            text(if expanded { "▾" } else { "▸" }).size(14),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([4, 10])
    .on_press(Message::Panel(PanelMessage::ToggleExpanded))
    .style(dark_button)
    .into()
}

/// The panel's buttons: a faint fill that brightens on hover, cream
/// outline while pressed.
fn dark_button(_: &iced::Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(with_alpha(palette::TEXT, if hovered { 0.14 } else { 0.06 }).into()),
        text_color: if matches!(status, button::Status::Disabled) {
            palette::TEXT_MUTED
        } else {
            palette::TEXT
        },
        border: Border {
            color: if matches!(status, button::Status::Pressed) {
                CREAM
            } else {
                with_alpha(palette::TEXT, 0.15)
            },
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..button::Style::default()
    }
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

    let mut list = column![].spacing(4);
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
                arclens_ui::markers::badge(&category.id, None, 22.0),
                checkbox(category.shown)
                    .label(category.label.clone())
                    .size(15)
                    .text_size(15)
                    .font(STRONG)
                    .style(check_style)
                    .on_toggle(move |_| send(ToApp::ToggleMarkerCategory {
                        category: id.clone()
                    }))
                    .width(Length::Fill),
                text(category.count.to_string())
                    .size(13)
                    .font(DISPLAY_SEMI)
                    .color(palette::TEXT_MUTED),
                expander(category, open),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
        if open {
            for sub in subs {
                let (c, s) = (category.id.clone(), sub.id.clone());
                list = list.push(
                    row![
                        Space::new().width(10),
                        arclens_ui::markers::badge(&category.id, Some(&sub.id), 18.0),
                        checkbox(sub.shown)
                            .label(sub.label.clone())
                            .size(13)
                            .text_size(14)
                            .font(theme::BODY)
                            .style(check_style)
                            .on_toggle(move |_| send(ToApp::ToggleMarkerSubcategory {
                                category: c.clone(),
                                subcategory: s.clone(),
                            }))
                            .width(Length::Fill),
                        text(sub.count.to_string())
                            .size(13)
                            .font(DISPLAY_SEMI)
                            .color(palette::TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }
        }
    }

    column![
        text_input("Search markers…", &state.query)
            .on_input(|q| Message::Panel(PanelMessage::Query(q)))
            .padding([6, 10])
            .size(15)
            .font(theme::BODY)
            .style(theme::input_style),
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
    .spacing(10)
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
    button(text(if open { "▾" } else { "▸" }).size(14))
        .padding([0, 4])
        .on_press(Message::Panel(PanelMessage::Open(category.id.clone())))
        .style(|_, _| button::Style {
            text_color: palette::TEXT_MUTED,
            ..button::Style::default()
        })
        .into()
}

fn small_button(label: &str, on_press: Message) -> Element<'_, Message> {
    button(text(label.to_uppercase()).size(13).font(DISPLAY_SEMI))
        .padding([3, 12])
        .on_press(on_press)
        .style(dark_button)
        .into()
}

/// Checkboxes: cream when ticked, like the game's toggles.
fn check_style(_: &iced::Theme, status: checkbox::Status) -> checkbox::Style {
    let (checked, hovered) = match status {
        checkbox::Status::Active { is_checked } | checkbox::Status::Disabled { is_checked } => {
            (is_checked, false)
        }
        checkbox::Status::Hovered { is_checked } => (is_checked, true),
    };
    checkbox::Style {
        background: if checked {
            CREAM.into()
        } else {
            with_alpha(palette::TEXT, if hovered { 0.12 } else { 0.05 }).into()
        },
        icon_color: INK,
        border: Border {
            color: if checked {
                CREAM
            } else {
                with_alpha(palette::TEXT, 0.3)
            },
            width: 1.0,
            radius: 3.0.into(),
        },
        text_color: Some(if checked {
            palette::TEXT
        } else {
            palette::TEXT_MUTED
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(expanded: bool) -> PanelState {
        PanelState {
            panel: Some(MapPanel {
                map_name: "Dam Battlegrounds".into(),
                categories: Vec::new(),
                condition: None,
                presets: Vec::new(),
                active_preset: None,
                edited: false,
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
    fn preset_arrows_wrap_around() {
        let preset = |id: &str| arclens_ipc::PanelPreset {
            id: id.into(),
            name: id.into(),
            for_condition: false,
        };
        let mut panel = MapPanel {
            map_name: String::new(),
            categories: Vec::new(),
            condition: None,
            presets: vec![preset("a"), preset("b"), preset("c")],
            active_preset: Some("a".into()),
            edited: false,
        };
        assert_eq!(cycle(&panel, 1).as_deref(), Some("b"));
        assert_eq!(cycle(&panel, -1).as_deref(), Some("c"));
        panel.active_preset = Some("gone".into());
        assert_eq!(cycle(&panel, 1).as_deref(), Some("a"));
        assert_eq!(cycle(&panel, -1).as_deref(), Some("c"));
        panel.presets.clear();
        assert_eq!(cycle(&panel, 1), None);
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
