//! Quick item search, shown while the overlay is interactive: a search box
//! at the top of the screen with the app's best matches under it. Picking
//! one shows its card like a pick in the app.

use crate::Message;
use arclens_core::{ItemId, Rarity};
use arclens_ipc::{SearchHit, ToApp};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, column, container, image, row, text, text_input};
use iced::{Alignment, Border, Element, Length};

/// Widget id of the search box, focused when the overlay turns interactive.
pub const INPUT_ID: &str = "overlay-quick-search";
/// Box width, logical pixels.
const WIDTH: f32 = 520.0;
/// Distance from the top of the screen, as a share of its height (below
/// the game's top bar).
const TOP: f32 = 0.09;
/// Results shown at most.
pub const MAX_HITS: usize = 8;

/// A result as drawn: its icon decoded once on arrival.
#[derive(Debug)]
pub struct Hit {
    id: ItemId,
    name: String,
    rarity: Option<Rarity>,
    icon: Option<image::Handle>,
}

#[derive(Debug, Default)]
pub struct SearchState {
    pub query: String,
    hits: Vec<Hit>,
}

#[derive(Debug, Clone)]
pub enum SearchMessage {
    Query(String),
    /// Enter: the top result.
    Submit,
    Pick(ItemId),
}

impl SearchState {
    /// Applies a local message; returns what to send to the app, if any.
    pub fn update(&mut self, message: SearchMessage) -> Option<ToApp> {
        match message {
            SearchMessage::Query(query) => {
                if query.trim().is_empty() {
                    self.hits.clear();
                }
                self.query.clone_from(&query);
                Some(ToApp::Search { query })
            }
            SearchMessage::Submit => self
                .hits
                .first()
                .map(|hit| ToApp::PickItem { id: hit.id.clone() }),
            SearchMessage::Pick(id) => Some(ToApp::PickItem { id }),
        }
    }

    /// The app's answer for `query`; ignored once the box says otherwise.
    pub fn results(&mut self, query: &str, hits: Vec<SearchHit>) {
        if query != self.query || query.trim().is_empty() {
            return;
        }
        self.hits = hits
            .into_iter()
            .take(MAX_HITS)
            .map(|hit| Hit {
                icon: hit
                    .icon
                    .as_deref()
                    .and_then(|p| arclens_ui::decode_icon(p, 48)),
                id: hit.id,
                name: hit.name,
                rarity: hit.rarity,
            })
            .collect();
    }
}

/// The search panel on a `screen`-sized surface.
pub fn view(state: &SearchState, screen: iced::Size) -> Element<'_, Message> {
    let input = text_input("Search items…", &state.query)
        .id(INPUT_ID)
        .on_input(|q| Message::Search(SearchMessage::Query(q)))
        .on_submit(Message::Search(SearchMessage::Submit))
        .padding([8, 12])
        .size(16)
        .style(input_style);
    let mut body = column![input].spacing(6);
    if state.query.trim().is_empty() {
        body = body.push(
            text("Type an item name · Enter shows the top result")
                .size(12)
                .color(palette::TEXT_MUTED),
        );
    } else if state.hits.is_empty() {
        body = body.push(text("No matches").size(12).color(palette::TEXT_MUTED));
    }
    for hit in &state.hits {
        body = body.push(hit_row(hit));
    }
    let panel = container(body)
        .width(WIDTH)
        .padding(10)
        .style(|_| container::Style {
            background: Some(with_alpha(palette::SURFACE, 0.96).into()),
            border: Border {
                radius: 10.0.into(),
                width: 1.0,
                color: palette::BORDER,
            },
            ..Default::default()
        });
    column![
        Space::new().height(screen.height * TOP),
        container(panel).center_x(Length::Fill),
    ]
    .into()
}

fn input_style(_: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: palette::SURFACE_RAISED.into(),
        border: Border {
            radius: 6.0.into(),
            width: 1.0,
            color: if focused {
                with_alpha(palette::TEXT, 0.35)
            } else {
                palette::BORDER
            },
        },
        icon: palette::TEXT_MUTED,
        placeholder: palette::TEXT_MUTED,
        value: palette::TEXT,
        selection: with_alpha(palette::COIN, 0.35),
    }
}

fn hit_row(hit: &Hit) -> Element<'_, Message> {
    let color = palette::rarity(hit.rarity);
    let icon: Element<'_, Message> = match &hit.icon {
        Some(handle) => image(handle.clone()).width(32).height(32).into(),
        None => Space::new().width(32).height(32).into(),
    };
    button(
        row![
            icon,
            text(&hit.name)
                .size(14)
                .color(palette::TEXT)
                .width(Length::Fill),
            text(palette::rarity_label(hit.rarity))
                .size(11)
                .color(color),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Search(SearchMessage::Pick(hit.id.clone())))
    .padding([4, 8])
    .width(Length::Fill)
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| with_alpha(color, 0.18).into()),
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        text_color: palette::TEXT,
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str) -> SearchHit {
        SearchHit {
            id: ItemId(name.to_lowercase()),
            name: name.to_owned(),
            rarity: None,
            icon: None,
        }
    }

    #[test]
    fn typing_asks_the_app_and_enter_picks_the_top_hit() {
        let mut state = SearchState::default();
        assert_eq!(
            state.update(SearchMessage::Query("gear".into())),
            Some(ToApp::Search {
                query: "gear".into()
            })
        );
        assert_eq!(state.update(SearchMessage::Submit), None);
        state.results("gear", vec![hit("Rusted Gear"), hit("Gear Bench")]);
        assert_eq!(
            state.update(SearchMessage::Submit),
            Some(ToApp::PickItem {
                id: ItemId("rusted gear".into())
            })
        );
    }

    #[test]
    fn stale_results_are_dropped() {
        let mut state = SearchState::default();
        state.update(SearchMessage::Query("gea".into()));
        state.update(SearchMessage::Query("gear".into()));
        state.results("gea", vec![hit("Gears")]);
        assert_eq!(state.hits.len(), 0);
        state.results("gear", vec![hit("Rusted Gear")]);
        assert_eq!(state.hits.len(), 1);
    }
}
