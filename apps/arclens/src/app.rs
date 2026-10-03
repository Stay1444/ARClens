//! Companion window state, update loop and view.

use crate::icons::{Icon, Icons};
use crate::overlay_link::OverlayHandle;
use crate::paths::Paths;
use crate::{data, hotkeys, overlay_link, overlay_process, vision};
use arclens_core::{Item, ItemId, advise};
use arclens_data::{Catalog, ItemSearch};
use arclens_hotkeys::Action;
use arclens_ipc::ToOverlay;
use arclens_ui::format::thousands;
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::{CardSize, ItemCard, item_card};
use iced::widget::{Space, button, column, container, image, row, scrollable, text, text_input};
use iced::{Alignment, Border, Color, Element, Font, Length, Subscription, Task, Theme, font};
use std::sync::Arc;

/// Rows shown in the result list (the whole catalogue is ~600 items).
const MAX_RESULTS: usize = 100;
const SEARCH_ID: &str = "search";
const LIST_WIDTH: f32 = 400.0;
const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

#[derive(Debug)]
pub struct App {
    catalog: Load<Arc<Catalog>>,
    search: ItemSearch,
    icons: Icons,
    query: String,
    results: Vec<ItemId>,
    selected: Option<ItemId>,
    overlay: Option<OverlayHandle>,
    overlay_visible: bool,
    overlay_interactive: bool,
    /// Whether screen-based item detection runs (opt-in: starting it opens
    /// the desktop's screen-share dialog the first time).
    vision_enabled: bool,
    /// Item currently detected under the cursor in game, and where.
    hover: Option<(ItemId, arclens_ipc::NormRect)>,
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
    /// Enter in the search box: open the top result.
    SelectFirst,
    IconLoaded(ItemId, Option<Icon>),
    ToggleOverlay,
    ToggleInteractive,
    ToggleVision,
    Overlay(overlay_link::Event),
    Hotkey(hotkeys::Event),
    OverlayProcess(overlay_process::Event),
    Vision(vision::Event),
}

impl App {
    pub fn boot(paths: Paths) -> (Self, Task<Message>) {
        let app = Self {
            catalog: Load::Loading,
            search: ItemSearch::default(),
            icons: Icons::new(&paths),
            query: String::new(),
            results: Vec::new(),
            selected: None,
            overlay: None,
            overlay_visible: false,
            overlay_interactive: false,
            vision_enabled: std::env::var(vision::ENABLE_ENV).is_ok_and(|v| v == "1")
                || std::env::var_os(vision::REPLAY_DIR_ENV).is_some(),
            hover: None,
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
        let mut subscriptions = vec![
            overlay_link::subscription().map(Message::Overlay),
            hotkeys::subscription().map(Message::Hotkey),
        ];
        if self.vision_enabled {
            subscriptions.push(vision::subscription().map(Message::Vision));
        }
        if overlay_process::enabled() {
            subscriptions.push(overlay_process::subscription().map(Message::OverlayProcess));
        }
        Subscription::batch(subscriptions)
    }

    #[allow(clippy::unused_self, reason = "signature required by iced")]
    pub fn theme(&self) -> Theme {
        Theme::custom(
            "ARClens".to_owned(),
            iced::theme::Palette {
                background: Color::from_rgb8(0x0d, 0x0f, 0x13),
                text: palette::TEXT,
                primary: Color::from_rgb8(0x3c, 0xc8, 0xe6),
                success: palette::verdict(arclens_core::Verdict::Keep),
                warning: palette::verdict(arclens_core::Verdict::Sell),
                danger: Color::from_rgb8(0xe5, 0x48, 0x4d),
            },
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CatalogLoaded(Ok(catalog)) => {
                self.catalog = Load::Ready(catalog);
                return self.refresh_results();
            }
            Message::CatalogLoaded(Err(error)) => self.catalog = Load::Failed(error),
            Message::QueryChanged(query) => {
                self.query = query;
                return self.refresh_results();
            }
            Message::Select(id) => {
                self.selected = Some(id);
                self.push_selected_to_overlay();
            }
            Message::SelectFirst => {
                if let Some(first) = self.results.first().cloned() {
                    return self.update(Message::Select(first));
                }
            }
            Message::IconLoaded(id, icon) => {
                let is_selected = self.selected.as_ref() == Some(&id);
                let is_hovered = self.hover.as_ref().is_some_and(|(h, _)| *h == id);
                self.icons.insert(id, icon);
                // Resend so the overlay picks up the icon path.
                if is_selected {
                    self.push_selected_to_overlay();
                }
                if is_hovered {
                    self.push_hover_to_overlay();
                }
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
            Message::Overlay(event) => return self.on_overlay_event(event),
            Message::ToggleVision => {
                self.vision_enabled = !self.vision_enabled;
                if !self.vision_enabled {
                    self.hover = None;
                    self.send(ToOverlay::ClearHover);
                }
            }
            Message::Vision(event) => return self.on_vision_event(event),
            Message::OverlayProcess(overlay_process::Event::Unavailable(error)) => {
                self.status.push(format!("Overlay not started: {error}"));
            }
        }
        Task::none()
    }

    fn on_overlay_event(&mut self, event: overlay_link::Event) -> Task<Message> {
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
                return self.refresh_results();
            }
            overlay_link::Event::Listening | overlay_link::Event::Message(_) => {}
            overlay_link::Event::Failed(error) => {
                self.status.push(format!("Overlay link failed: {error}"));
            }
        }
        Task::none()
    }

    fn on_vision_event(&mut self, event: vision::Event) -> Task<Message> {
        match event {
            vision::Event::Hover(hover) => {
                let Load::Ready(catalog) = &self.catalog else {
                    return Task::none();
                };
                let Some((item, confidence)) =
                    arclens_data::match_name(&hover.name, &catalog.items)
                else {
                    tracing::debug!(text = %hover.name, "no catalogue match");
                    self.hover = None;
                    self.send(ToOverlay::ClearHover);
                    return Task::none();
                };
                tracing::info!(text = %hover.name, item = %item.id, confidence, "hovered item");
                let [x, y, width, height] = hover.panel_normalized();
                let anchor = arclens_ipc::NormRect {
                    x,
                    y,
                    width,
                    height,
                };
                let load = self.icons.request(item);
                self.hover = Some((item.id.clone(), anchor));
                self.push_hover_to_overlay();
                // The icon arrives later; `IconLoaded` resends the card.
                if let Some(load) = load {
                    return Task::perform(load, |(id, icon)| Message::IconLoaded(id, icon));
                }
            }
            vision::Event::Gone => {
                self.hover = None;
                self.send(ToOverlay::ClearHover);
            }
            vision::Event::Unavailable(reason) => {
                tracing::info!(%reason, "item detection off");
                self.status.push(format!("Item detection off: {reason}"));
            }
        }
        Task::none()
    }

    fn send(&self, msg: ToOverlay) {
        if let Some(overlay) = &self.overlay {
            overlay.send(msg);
        }
    }

    /// Recomputes the result list and starts loading icons for it. An empty
    /// query lists the catalogue alphabetically so it can be browsed.
    fn refresh_results(&mut self) -> Task<Message> {
        let Load::Ready(catalog) = &self.catalog else {
            return Task::none();
        };
        self.results = if self.query.trim().is_empty() {
            let mut all: Vec<&Item> = catalog.items.iter().collect();
            all.sort_by(|a, b| a.name.cmp(&b.name));
            all.into_iter()
                .take(MAX_RESULTS)
                .map(|i| i.id.clone())
                .collect()
        } else {
            self.search
                .search(&catalog.items, &self.query, MAX_RESULTS)
                .into_iter()
                .map(|item| item.id.clone())
                .collect()
        };

        let loads: Vec<_> = self
            .results
            .iter()
            .filter_map(|id| catalog.item(id))
            .filter_map(|item| self.icons.request(item))
            .map(|load| Task::perform(load, |(id, icon)| Message::IconLoaded(id, icon)))
            .collect();
        Task::batch(loads)
    }

    fn push_hover_to_overlay(&self) {
        let (Load::Ready(catalog), Some((id, anchor))) = (&self.catalog, &self.hover) else {
            return;
        };
        if let Some(item) = catalog.item(id) {
            self.send(ToOverlay::ShowHover {
                item: Box::new(item.clone()),
                advice: advise(item, |id| catalog.item(id)),
                icon: self.icons.get(id).map(|icon| icon.path.clone()),
                recycle_names: recycle_names(item, catalog),
                anchor: *anchor,
            });
        }
    }

    fn push_selected_to_overlay(&self) {
        let (Load::Ready(catalog), Some(id)) = (&self.catalog, &self.selected) else {
            return;
        };
        if let Some(item) = catalog.item(id) {
            self.send(ToOverlay::ShowItem {
                item: Box::new(item.clone()),
                advice: advise(item, |id| catalog.item(id)),
                icon: self.icons.get(id).map(|icon| icon.path.clone()),
                recycle_names: recycle_names(item, catalog),
            });
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match &self.catalog {
            Load::Loading => centered(text("Loading game data…").color(palette::TEXT_MUTED)),
            Load::Failed(error) => centered(
                column![
                    text("Could not load game data").size(20).font(BOLD),
                    text(error).color(palette::TEXT_MUTED),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            ),
            Load::Ready(catalog) => self.view_catalog(catalog),
        };

        column![self.view_top_bar(), body, self.view_footer()].into()
    }

    fn view_top_bar(&self) -> Element<'_, Message> {
        let (dot, label) = match (&self.overlay, self.overlay_visible) {
            (None, _) => (palette::TEXT_MUTED, "Overlay offline"),
            (Some(_), true) => (
                palette::verdict(arclens_core::Verdict::Keep),
                "Overlay shown",
            ),
            (Some(_), false) => (
                palette::verdict(arclens_core::Verdict::Sell),
                "Overlay hidden",
            ),
        };
        let status = row![
            container(Space::new().width(8).height(8)).style(move |_| container::Style {
                background: Some(dot.into()),
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
            text(label).size(13).color(palette::TEXT_MUTED),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let bar = row![
            text("ARClens").size(20).font(BOLD),
            text_input("Search items…", &self.query)
                .id(SEARCH_ID)
                .on_input(Message::QueryChanged)
                .on_submit(Message::SelectFirst)
                .padding([8, 12])
                .size(15)
                .width(Length::Fill),
            status,
            pill_button(
                "Detect items",
                if self.vision_enabled { "on" } else { "off" },
                Message::ToggleVision,
            ),
            pill_button(
                if self.overlay_visible {
                    "Hide overlay"
                } else {
                    "Show overlay"
                },
                "Ctrl+Shift+O",
                Message::ToggleOverlay,
            ),
            pill_button(
                if self.overlay_interactive {
                    "Click-through"
                } else {
                    "Interactive"
                },
                "Ctrl+Shift+I",
                Message::ToggleInteractive,
            ),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        container(bar)
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color::from_rgb8(0x14, 0x17, 0x1c).into()),
                border: Border {
                    color: palette::BORDER,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    fn view_catalog<'a>(&'a self, catalog: &'a Catalog) -> Element<'a, Message> {
        let rows = self
            .results
            .iter()
            .filter_map(|id| catalog.item(id))
            .fold(column![].spacing(2), |col, item| {
                col.push(self.view_row(item, catalog))
            });
        let list_header = text(if self.query.trim().is_empty() {
            format!(
                "BROWSING {} OF {} ITEMS · TYPE TO SEARCH",
                self.results.len(),
                catalog.items.len()
            )
        } else {
            format!("{} RESULTS", self.results.len())
        })
        .size(11)
        .color(palette::TEXT_MUTED);

        let list = column![
            list_header,
            scrollable(rows.padding([0, 8])).height(Length::Fill)
        ]
        .spacing(8)
        .padding([12, 8])
        .width(LIST_WIDTH);

        let detail: Element<'_, Message> =
            match self.selected.as_ref().and_then(|id| catalog.item(id)) {
                Some(item) => {
                    let card = item_card(&ItemCard {
                        item,
                        advice: advise(item, |id| catalog.item(id)),
                        icon: self.icons.get(&item.id).map(|icon| &icon.large),
                        recycle_names: recycle_names(item, catalog),
                        size: CardSize::Full,
                    });
                    scrollable(
                        container(card)
                            .padding(24)
                            .max_width(720),
                    )
                    .height(Length::Fill)
                    .into()
                }
                None => centered(
                    column![
                        text("Pick an item").size(22).font(BOLD),
                        text("Search above (Enter opens the top result) or browse the list. The selected item is also shown on the in-game overlay.")
                            .color(palette::TEXT_MUTED),
                    ]
                    .spacing(6)
                    .align_x(Alignment::Center),
                ),
            };

        row![
            list,
            container(Space::new().width(1).height(Length::Fill))
                .style(|_| container::Style::default().background(palette::BORDER)),
            container(detail).width(Length::Fill).height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    fn view_row<'a>(&'a self, item: &'a Item, catalog: &'a Catalog) -> Element<'a, Message> {
        let rarity = palette::rarity(item.rarity);
        let advice = advise(item, |id| catalog.item(id));
        let verdict_color = palette::verdict(advice.verdict);

        let icon: Element<'_, Message> = match self.icons.get(&item.id) {
            Some(icon) => image(icon.thumb.clone()).width(32).height(32).into(),
            None => Space::new().width(32).height(32).into(),
        };
        let tile = container(icon).center(40).style(move |_| container::Style {
            background: Some(with_alpha(rarity, 0.16).into()),
            border: Border {
                color: with_alpha(rarity, 0.55),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..container::Style::default()
        });

        let value = item
            .value
            .map_or_else(String::new, |v| format!("{} ¢", thousands(v)));
        let content = row![
            tile,
            column![
                text(&item.name).size(14),
                text(palette::rarity_label(item.rarity))
                    .size(10)
                    .color(rarity),
            ]
            .spacing(1)
            .width(Length::Fill),
            column![
                text(palette::verdict_label(advice.verdict))
                    .size(11)
                    .font(BOLD)
                    .color(verdict_color),
                text(value).size(12).color(palette::COIN),
            ]
            .spacing(1)
            .align_x(Alignment::End),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let selected = self.selected.as_ref() == Some(&item.id);
        button(content)
            .width(Length::Fill)
            .padding([6, 8])
            .on_press(Message::Select(item.id.clone()))
            .style(move |_, status| {
                let background = match (selected, status) {
                    (true, _) => Some(with_alpha(palette::TEXT, 0.10).into()),
                    (false, button::Status::Hovered | button::Status::Pressed) => {
                        Some(with_alpha(palette::TEXT, 0.05).into())
                    }
                    _ => None,
                };
                button::Style {
                    background,
                    text_color: palette::TEXT,
                    border: Border {
                        radius: 6.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .into()
    }

    fn view_footer(&self) -> Element<'_, Message> {
        let mut parts = Vec::new();
        if let Load::Ready(catalog) = &self.catalog {
            parts.push(format!("Data: {}", catalog.source));
        }
        parts.extend(self.status.iter().cloned());
        container(
            text(parts.join("   ·   "))
                .size(11)
                .color(palette::TEXT_MUTED),
        )
        .padding([6, 16])
        .width(Length::Fill)
        .into()
    }
}

fn recycle_names(item: &Item, catalog: &Catalog) -> Vec<String> {
    item.recycles_into
        .iter()
        .map(|q| {
            catalog
                .item(&q.item)
                .map_or_else(|| q.item.to_string(), |i| i.name.clone())
        })
        .collect()
}

fn pill_button<'a>(label: &'a str, shortcut: &'a str, on_press: Message) -> Element<'a, Message> {
    button(
        row![
            text(label).size(13),
            text(shortcut).size(11).color(palette::TEXT_MUTED),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .on_press(on_press)
    .style(|_, status| {
        let alpha = match status {
            button::Status::Hovered | button::Status::Pressed => 0.12,
            _ => 0.06,
        };
        button::Style {
            background: Some(with_alpha(palette::TEXT, alpha).into()),
            text_color: palette::TEXT,
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

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).padding(24).into()
}
