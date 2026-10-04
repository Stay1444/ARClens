//! Quick item search, shown while the overlay is interactive: a search box
//! at the top of the screen with the app's best matches under it. Picking
//! one shows its card like a pick in the app.

use crate::Message;
use arclens_core::{ItemId, Rarity};
use arclens_ipc::{SearchHit, ToApp};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, CREAM, DISPLAY, DISPLAY_SEMI, INK, RADIUS, STRONG};
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
        .padding([10, 14])
        .size(theme::size::BODY)
        .font(theme::BODY)
        .style(theme::input_style);
    let mut body = column![input].spacing(6);
    if state.query.trim().is_empty() {
        body = body.push(
            text("Type an item name · Enter shows the top result")
                .size(theme::size::SMALL)
                .color(palette::TEXT_MUTED),
        );
    } else if state.hits.is_empty() {
        body = body.push(
            text("No matches")
                .size(theme::size::SMALL)
                .color(palette::TEXT_MUTED),
        );
    }
    for hit in &state.hits {
        body = body.push(hit_row(hit));
    }

    let header = container(
        row![
            text("QUICK SEARCH")
                .size(theme::size::H2)
                .font(DISPLAY)
                .color(INK)
                .width(Length::Fill),
            text("ARCLENS")
                .size(theme::size::TINY)
                .font(DISPLAY)
                .color(with_alpha(INK, 0.55)),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
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
        .padding([12, 14])
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
    column![
        Space::new().height(screen.height * TOP),
        container(column![header, body].width(WIDTH)).center_x(Length::Fill),
    ]
    .into()
}

fn hit_row(hit: &Hit) -> Element<'_, Message> {
    let color = palette::rarity(hit.rarity);
    let icon: Element<'_, Message> = match &hit.icon {
        Some(handle) => image(handle.clone()).width(36).height(36).into(),
        None => Space::new().width(36).height(36).into(),
    };
    button(
        row![
            icon,
            text(&hit.name)
                .size(theme::size::BODY)
                .font(STRONG)
                .color(palette::TEXT)
                .width(Length::Fill),
            text(palette::rarity_label(hit.rarity).to_uppercase())
                .size(theme::size::SMALL)
                .font(DISPLAY_SEMI)
                .color(color),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Search(SearchMessage::Pick(hit.id.clone())))
    .padding([5, 8])
    .width(Length::Fill)
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| with_alpha(color, 0.18).into()),
        border: Border {
            radius: RADIUS.into(),
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
