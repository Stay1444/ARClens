//! Companion window state, update loop and view.

use crate::overlay_link::OverlayHandle;
use crate::paths::Paths;
use crate::{data, hotkeys, overlay_link};
use arclens_core::{Item, ItemId, Verdict, advise};
use arclens_data::{Catalog, ItemSearch};
use arclens_hotkeys::Action;
use arclens_ipc::ToOverlay;
use iced::widget::{button, column, container, row, rule, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};
use std::sync::Arc;

const MAX_RESULTS: usize = 50;
const SEARCH_ID: &str = "search";

#[derive(Debug)]
pub struct App {
    catalog: Load<Arc<Catalog>>,
    search: ItemSearch,
    query: String,
    results: Vec<ItemId>,
    selected: Option<ItemId>,
    overlay: Option<OverlayHandle>,
    overlay_visible: bool,
    overlay_interactive: bool,
    status: Vec<String>,
}

#[derive(Debug)]
enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    CatalogLoaded(Result<Arc<Catalog>, String>),
    QueryChanged(String),
    Select(ItemId),
    ToggleOverlay,
    ToggleInteractive,
    Overlay(overlay_link::Event),
    Hotkey(hotkeys::Event),
}

impl App {
    pub fn boot(paths: Paths) -> (Self, Task<Message>) {
        let app = Self {
            catalog: Load::Loading,
            search: ItemSearch::default(),
            query: String::new(),
            results: Vec::new(),
            selected: None,
            overlay: None,
            overlay_visible: false,
            overlay_interactive: false,
            status: Vec::new(),
        };
        let tasks = Task::batch([
            Task::perform(data::load(paths), Message::CatalogLoaded),
            iced::widget::operation::focus(SEARCH_ID),
        ]);
        (app, tasks)
    }

    #[allow(clippy::unused_self, reason = "signature required by iced")]
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            overlay_link::subscription().map(Message::Overlay),
            hotkeys::subscription().map(Message::Hotkey),
        ])
    }

    #[allow(clippy::unused_self, reason = "signature required by iced")]
    pub fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CatalogLoaded(Ok(catalog)) => self.catalog = Load::Ready(catalog),
            Message::CatalogLoaded(Err(error)) => self.catalog = Load::Failed(error),
            Message::QueryChanged(query) => {
                self.query = query;
                self.refresh_results();
            }
            Message::Select(id) => {
                self.selected = Some(id);
                self.push_selected_to_overlay();
            }
            Message::ToggleOverlay
            | Message::Hotkey(hotkeys::Event::Pressed(Action::ToggleOverlay)) => {
                self.overlay_visible = !self.overlay_visible;
                self.send(ToOverlay::SetVisible {
                    visible: self.overlay_visible,
                });
            }
            Message::ToggleInteractive
            | Message::Hotkey(hotkeys::Event::Pressed(Action::ToggleInteractive)) => {
                self.overlay_interactive = !self.overlay_interactive;
                self.send(ToOverlay::SetInteractive {
                    interactive: self.overlay_interactive,
                });
            }
            Message::Hotkey(hotkeys::Event::Unavailable(error)) => {
                tracing::debug!(%error, "hotkeys unavailable");
                self.status.push(
                    "Global hotkeys unavailable (no GlobalShortcuts portal); use the buttons above."
                        .to_owned(),
                );
            }
            Message::Overlay(event) => self.on_overlay_event(event),
        }
        Task::none()
    }

    fn on_overlay_event(&mut self, event: overlay_link::Event) {
        match event {
            overlay_link::Event::Connected(handle) => {
                self.overlay = Some(handle);
                // Bring a (re)started overlay up to date.
                self.send(ToOverlay::SetVisible {
                    visible: self.overlay_visible,
                });
                self.send(ToOverlay::SetInteractive {
                    interactive: self.overlay_interactive,
                });
                self.push_selected_to_overlay();
            }
            overlay_link::Event::Disconnected => self.overlay = None,
            overlay_link::Event::Message(arclens_ipc::ToApp::Search { query }) => {
                self.query = query;
                self.refresh_results();
            }
            overlay_link::Event::Listening | overlay_link::Event::Message(_) => {}
            overlay_link::Event::Failed(error) => {
                self.status.push(format!("Overlay link failed: {error}"));
            }
        }
    }

    fn send(&self, msg: ToOverlay) {
        if let Some(overlay) = &self.overlay {
            overlay.send(msg);
        }
    }

    fn refresh_results(&mut self) {
        let Load::Ready(catalog) = &self.catalog else {
            return;
        };
        self.results = self
            .search
            .search(&catalog.items, &self.query, MAX_RESULTS)
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
    }

    fn push_selected_to_overlay(&self) {
        let (Load::Ready(catalog), Some(id)) = (&self.catalog, &self.selected) else {
            return;
        };
        if let Some(item) = catalog.item(id) {
            let advice = advise(item, |id| catalog.item(id));
            self.send(ToOverlay::ShowItem {
                item: Box::new(item.clone()),
                advice,
            });
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match &self.catalog {
            Load::Loading => text("Loading game data…").into(),
            Load::Failed(error) => text(format!("Could not load game data: {error}")).into(),
            Load::Ready(catalog) => self.view_catalog(catalog),
        };

        column![
            self.view_toolbar(),
            rule::horizontal(1),
            body,
            self.view_footer()
        ]
        .spacing(12)
        .padding(16)
        .into()
    }

    fn view_toolbar(&self) -> Element<'_, Message> {
        let overlay_state = match (&self.overlay, self.overlay_visible) {
            (None, _) => "Overlay: not running",
            (Some(_), true) => "Overlay: shown",
            (Some(_), false) => "Overlay: hidden",
        };
        row![
            text_input("Search items…", &self.query)
                .id(SEARCH_ID)
                .on_input(Message::QueryChanged)
                .padding(8)
                .width(Length::Fill),
            button(if self.overlay_visible {
                "Hide overlay"
            } else {
                "Show overlay"
            })
            .on_press(Message::ToggleOverlay),
            button(if self.overlay_interactive {
                "Click-through"
            } else {
                "Interactive"
            })
            .on_press(Message::ToggleInteractive),
            text(overlay_state),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_catalog<'a>(&'a self, catalog: &'a Catalog) -> Element<'a, Message> {
        let list = self.results.iter().filter_map(|id| catalog.item(id)).fold(
            column![].spacing(2),
            |col, item| {
                col.push(
                    button(text(&item.name))
                        .width(Length::Fill)
                        .style(button::text)
                        .on_press(Message::Select(item.id.clone())),
                )
            },
        );

        let detail: Element<'_, Message> =
            match self.selected.as_ref().and_then(|id| catalog.item(id)) {
                Some(item) => view_item(item, catalog),
                None => text(if self.query.is_empty() {
                    "Type to search the item database."
                } else {
                    "Select an item."
                })
                .into(),
            };

        row![
            scrollable(list)
                .width(Length::FillPortion(2))
                .height(Length::Fill),
            container(detail).width(Length::FillPortion(3)).padding(8),
        ]
        .spacing(16)
        .height(Length::Fill)
        .into()
    }

    fn view_footer(&self) -> Element<'_, Message> {
        let mut footer = column![].spacing(2);
        if let Load::Ready(catalog) = &self.catalog {
            footer = footer.push(text(format!("Data: {}", catalog.source)).size(12));
        }
        for line in &self.status {
            footer = footer.push(text(line).size(12));
        }
        footer.into()
    }
}

fn view_item<'a>(item: &'a Item, catalog: &'a Catalog) -> Element<'a, Message> {
    let advice = advise(item, |id| catalog.item(id));
    let verdict = match advice.verdict {
        Verdict::Keep => "Keep",
        Verdict::Recycle => "Recycle",
        Verdict::Sell => "Sell",
        Verdict::Unknown => "Unknown",
    };

    let mut col = column![
        text(&item.name).size(24),
        text(format!("Verdict: {verdict}")).size(18),
    ]
    .spacing(6);
    if let Some(rarity) = item.rarity {
        col = col.push(text(format!("Rarity: {rarity:?}")));
    }
    if let Some(category) = &item.category {
        col = col.push(text(format!("Type: {category}")));
    }
    if let Some(value) = advice.sell_value {
        col = col.push(text(format!("Sell value: {value}")));
    }
    if let Some(value) = advice.recycle_value {
        col = col.push(text(format!("Recycle value: {value}")));
    }
    if !item.recycles_into.is_empty() {
        let outputs: Vec<String> = item
            .recycles_into
            .iter()
            .map(|q| {
                let name = catalog
                    .item(&q.item)
                    .map_or(q.item.as_str(), |i| i.name.as_str());
                format!("{name} ×{}", q.quantity)
            })
            .collect();
        col = col.push(text(format!("Recycles into: {}", outputs.join(", "))));
    }
    if !item.required_for.is_empty() {
        col = col.push(text("Needed for:"));
        for req in &item.required_for {
            col = col.push(text(format!("  • {} ×{}", req.name, req.quantity)));
        }
    }
    if let Some(description) = &item.description {
        col = col.push(text(description).size(13));
    }
    scrollable(col).into()
}
