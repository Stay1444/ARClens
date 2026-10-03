//! Companion window state, update loop and view.

use crate::icons::{Icon, Icons};
use crate::overlay_link::OverlayHandle;
use crate::paths::Paths;
use crate::{data, game_process, hotkeys, overlay_link, overlay_process, vision};
use arclens_core::{Item, ItemId, Place, Progress, Situation, advise_in, breakdown};
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
/// Results sent to the overlay's quick search.
const SEARCH_HITS: usize = 8;
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
    event_icons: crate::event_icons::EventIcons,
    query: String,
    results: Vec<ItemId>,
    selected: Option<ItemId>,
    overlay: Option<OverlayHandle>,
    overlay_visible: bool,
    overlay_interactive: bool,
    /// When screen capture (item and map detection) runs.
    capture: CaptureMode,
    /// Whether an ARC Raiders process is running.
    game_running: bool,
    /// The player's workshop levels (`None`: never set, value-only advice).
    progress: Option<Progress>,
    progress_path: std::path::PathBuf,
    /// Which page the window shows.
    tab: Tab,
    /// Since when the game shows its main menu (our card is up).
    main_menu: Option<std::time::Instant>,
    /// Progress tab: the section shown.
    progress_section: crate::views::progress::Section,
    /// Map-condition schedule (MetaForge).
    events: Load<Vec<arclens_core::ScheduledEvent>>,
    /// The region the loaded schedule says it is for.
    events_region: Option<String>,
    /// Saved settings (server region).
    settings: Settings,
    events_loaded_at: Option<std::time::Instant>,
    /// Events tab: only this map's conditions (`None`: all maps).
    event_map_filter: Option<String>,
    /// Wall clock in Unix ms, advanced by the 1 s tick while it matters.
    now_ms: i64,
    paths: Paths,
    /// Map tab: selected map (MetaForge id) and markers per map.
    map: String,
    markers: std::collections::HashMap<String, Load<Vec<arclens_core::Marker>>>,
    marker_filter: arclens_core::MarkerFilter,
    /// Map tab and in game: the map's condition (a name from
    /// `metaforge::conditions`), `None` when unknown. Markers of other
    /// conditions are left out.
    map_condition: Option<&'static str>,
    presets: crate::presets::Presets,
    marker_query: String,
    /// The app icon, for the Home tab (parsed once).
    logo: iced::widget::svg::Handle,
    expanded_categories: std::collections::BTreeSet<String>,
    /// Map tab: what the view derives from markers, query and filter, and
    /// the plot's cached marker layer. Rebuilt on change, not per frame.
    map_summary: crate::views::map::MapSummary,
    map_plot: iced::widget::canvas::Cache,
    /// The open in-game map's view, as last read from the screen.
    game_view: GameMapView,
    /// The in-game map last recognised; kept across map close and reopen.
    last_map: Option<&'static str>,
    /// Set while the in-game map is open.
    map_screen: Option<MapScreen>,
    /// Item currently detected under the cursor in game, and where.
    hover: Option<(
        ItemId,
        arclens_ipc::NormRect,
        Situation,
        arclens_ipc::ItemSide,
    )>,
    status: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Where the app opens: status, conditions now, shortcuts.
    Home,
    Items,
    Map,
    Events,
    /// Where the player is in the game (workshop levels, …).
    Progress,
    Settings,
}

/// Settings saved in the config directory.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Settings {
    /// Server region for the event schedule (`None`: not chosen yet).
    #[serde(default)]
    region: Option<String>,
    /// How the overlay looks.
    #[serde(default)]
    overlay: arclens_ipc::OverlaySettings,
}

/// The open in-game map's view: where the last label read put it, and how
/// the map moved since.
#[derive(Debug, Clone, Default)]
struct GameMapView {
    /// Map → normalised screen on the frame of the last label read: the
    /// fit of its place names, else the previous view moved along.
    base: Option<arclens_core::Transform>,
    /// Motion since that frame (frame pixels); `None` when lost.
    motion: Option<arclens_vision::Motion>,
    /// Frame size, pixels.
    frame: (f32, f32),
    /// The quest panel covers the map's left.
    quest_panel_open: bool,
    /// The overlay has the current marker set (only the view changes).
    markers_sent: bool,
    /// What that set is: single markers, and areas' outlines and centres.
    shown: Vec<arclens_core::MapPoint>,
    shown_areas: Vec<(arclens_core::MapPoint, Vec<arclens_core::MapPoint>)>,
}

impl GameMapView {
    /// The view now: the base moved by the motion since.
    fn current(&self) -> Option<arclens_core::Transform> {
        Some(moved(self.base?, self.motion?, self.frame))
    }
}

/// `view` (map → normalised screen) after the map moved by `motion`
/// (frame pixels, on a `frame`-sized screen).
fn moved(
    view: arclens_core::Transform,
    motion: arclens_vision::Motion,
    frame: (f32, f32),
) -> arclens_core::Transform {
    let s = motion.scale;
    arclens_core::Transform {
        a: s * view.a,
        b: s * view.b,
        c: s * view.c,
        d: s * view.d,
        tx: s * view.tx + motion.dx / frame.0.max(1.0),
        ty: s * view.ty + motion.dy / frame.1.max(1.0),
    }
}

/// When to capture the screen for item and map detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// While the game runs (found in the process list).
    Auto,
    Always,
    Off,
}

impl CaptureMode {
    fn next(self) -> Self {
        match self {
            Self::Auto => Self::Always,
            Self::Always => Self::Off,
            Self::Off => Self::Auto,
        }
    }
}

/// The in-game map screen, as last read from the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MapScreen {
    /// MetaForge id of the map, if its title was recognised.
    map: Option<&'static str>,
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
    /// Cycle the capture mode (auto → always → off).
    ToggleVision,
    GameRunning(bool),
    SetTab(Tab),
    /// Typing in the Home search box: look it up on the Items tab.
    HomeSearch(String),
    /// Open the Map tab on this map.
    OpenMap(String),
    EventsLoaded(Result<arclens_data::metaforge::Schedule, String>),
    /// The player picked their server region.
    SetRegion(String),
    SetOverlayScale(f32),
    SetOverlayCorner(arclens_ipc::Corner),
    EventIconLoaded(String, Option<iced::widget::image::Handle>),
    /// Once a second while the Events tab is open (countdowns).
    Tick,
    FilterEventsMap(Option<String>),
    SelectMap(String),
    MarkersLoaded(String, Result<Vec<arclens_core::Marker>, String>),
    MarkerQuery(String),
    ToggleMarkerCategory(String),
    ToggleMarkerSubcategory(String, String),
    SelectCondition(Option<&'static str>),
    ApplyPreset(String),
    PresetDraft(String),
    PresetForMap(bool),
    PresetForCondition(bool),
    SavePresetAs,
    UpdatePreset,
    DeletePreset(String),
    ExpandMarkerCategory(String),
    ShowAllMarkers,
    HideAllMarkers,
    SetStationLevel(String, u32),
    SetQuestDone(String, bool),
    SetProjectPhases(String, u32),
    SetProgressSection(crate::views::progress::Section),
    /// Every 30 s while the main-menu card is up.
    RefreshMenuCard,
    ClearProgress,
    Overlay(overlay_link::Event),
    Hotkey(hotkeys::Event),
    OverlayProcess(overlay_process::Event),
    Vision(vision::Event),
}

impl App {
    pub fn boot(paths: Paths) -> (Self, Task<Message>) {
        let settings: Settings = crate::store::load(&paths.settings()).unwrap_or_default();
        let app = Self {
            catalog: Load::Loading,
            search: ItemSearch::default(),
            icons: Icons::new(&paths),
            event_icons: crate::event_icons::EventIcons::new(&paths),
            query: String::new(),
            results: Vec::new(),
            selected: None,
            overlay: None,
            overlay_visible: false,
            overlay_interactive: false,
            capture: if std::env::var(vision::ENABLE_ENV).is_ok_and(|v| v == "1")
                || std::env::var_os(vision::REPLAY_DIR_ENV).is_some()
            {
                CaptureMode::Always
            } else {
                CaptureMode::Auto
            },
            game_running: false,
            hover: None,
            progress: crate::progress::load(&paths.progress()),
            progress_path: paths.progress(),
            tab: Tab::Home,
            progress_section: crate::views::progress::Section::default(),
            main_menu: None,
            events: Load::Loading,
            events_region: None,
            settings: settings.clone(),
            events_loaded_at: None,
            event_map_filter: None,
            now_ms: now_ms(),
            map: arclens_data::metaforge::MAPS[0].0.to_owned(),
            markers: std::collections::HashMap::new(),
            marker_filter: crate::store::load(&paths.marker_filter()).unwrap_or_default(),
            map_condition: None,
            presets: crate::presets::Presets::load(paths.presets()),
            marker_query: String::new(),
            logo: iced::widget::svg::Handle::from_memory(crate::platform::ICON_SVG),
            expanded_categories: std::collections::BTreeSet::new(),
            map_screen: None,
            game_view: GameMapView::default(),
            last_map: None,
            map_summary: crate::views::map::MapSummary::default(),
            map_plot: iced::widget::canvas::Cache::new(),
            paths: paths.clone(),
            status: Vec::new(),
        };
        let tasks = Task::batch([
            Task::perform(data::load(paths.clone()), Message::CatalogLoaded),
            Task::perform(
                data::load_events(paths, settings.region),
                Message::EventsLoaded,
            ),
            iced::widget::operation::focus(crate::views::home::SEARCH_ID),
        ]);
        (app, tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            // Also while the search box has focus (it captures every key).
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(key) => tab_shortcut(key),
                _ => None,
            }),
            overlay_link::subscription().map(Message::Overlay),
            hotkeys::subscription().map(Message::Hotkey),
        ];
        if self.capture == CaptureMode::Auto {
            subscriptions.push(game_process::subscription().map(Message::GameRunning));
        }
        if self.capturing() {
            subscriptions.push(vision::subscription().map(Message::Vision));
        }
        if overlay_process::enabled() {
            subscriptions.push(overlay_process::subscription().map(Message::OverlayProcess));
        }
        if self.main_menu.is_some() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_secs(30))
                    .map(|_| Message::RefreshMenuCard),
            );
        }
        if matches!(self.tab, Tab::Events | Tab::Home) {
            subscriptions
                .push(iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick));
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
                // The catalogue carries fallback event icons.
                return Task::batch([self.refresh_results(), self.request_event_icons()]);
            }
            Message::CatalogLoaded(Err(error)) => self.catalog = Load::Failed(error),
            Message::QueryChanged(query) => {
                self.query = query;
                return self.refresh_results();
            }
            Message::Select(id) => {
                self.tab = Tab::Items;
                self.selected = Some(id);
                self.push_selected_to_overlay();
            }
            Message::SelectFirst => {
                if let Some(first) = self.results.first().cloned() {
                    return self.update(Message::Select(first));
                }
            }
            Message::IconLoaded(id, icon) => self.on_icon_loaded(id, icon),
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
                let was = self.capturing();
                self.capture = self.capture.next();
                self.capture_changed(was);
            }
            Message::GameRunning(running) => {
                let was = self.capturing();
                self.game_running = running;
                self.capture_changed(was);
            }
            Message::SetTab(tab) => return self.set_tab(tab),
            Message::HomeSearch(query) => return self.home_search(query),
            Message::OpenMap(map) => return self.open_map(map),
            Message::EventsLoaded(result) => return self.on_events_loaded(result),
            Message::EventIconLoaded(url, icon) => self.event_icons.insert(url, icon),
            Message::Tick => {
                self.now_ms = now_ms();
                return self.refresh_events_if_stale();
            }
            Message::FilterEventsMap(map) => self.event_map_filter = map,
            Message::SetRegion(region) => return self.set_region(region),
            Message::SetOverlayScale(_) | Message::SetOverlayCorner(_) => {
                self.set_overlay_setting(&message);
            }
            Message::SetStationLevel(station, level) => {
                self.progress
                    .get_or_insert_with(Progress::default)
                    .stations
                    .insert(station, level);
                self.progress_changed();
            }
            Message::SetQuestDone(..) | Message::SetProjectPhases(..) => {
                self.set_quest_or_project(message);
            }
            Message::SetProgressSection(section) => self.progress_section = section,
            Message::RefreshMenuCard => self.push_menu_card(),
            Message::ClearProgress => {
                self.progress = None;
                self.progress_changed();
            }
            Message::Vision(event) => return self.on_vision_event(event),
            Message::OverlayProcess(overlay_process::Event::Unavailable(error)) => {
                self.status.push(format!("Overlay not started: {error}"));
            }
            // The rest are Map-tab messages.
            message => return self.update_map(message),
        }
        Task::none()
    }

    fn on_icon_loaded(&mut self, id: ItemId, icon: Option<Icon>) {
        let is_selected = self.selected.as_ref() == Some(&id);
        let is_hovered = self.hover.as_ref().is_some_and(|(h, ..)| *h == id);
        let in_search =
            self.overlay_interactive && self.results.iter().take(SEARCH_HITS).any(|r| *r == id);
        self.icons.insert(id, icon);
        // Resend so the overlay picks up the icon path.
        if is_selected {
            self.push_selected_to_overlay();
        }
        if is_hovered {
            self.push_hover_to_overlay();
        }
        if in_search {
            self.push_search_results();
        }
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
                self.send(ToOverlay::Configure {
                    settings: self.settings.overlay,
                });
                self.push_selected_to_overlay();
                self.push_map_panel();
                self.push_map_markers();
                self.push_menu_card();
            }
            overlay_link::Event::Disconnected => self.overlay = None,
            overlay_link::Event::Message(arclens_ipc::ToApp::Search { query }) => {
                self.query = query;
                let icons = self.refresh_results();
                self.push_search_results();
                return icons;
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::PickItem { id }) => {
                self.selected = Some(id);
                self.push_selected_to_overlay();
                // The card is only drawn while the overlay is shown.
                if !self.overlay_visible {
                    self.overlay_visible = true;
                    self.send(ToOverlay::SetVisible { visible: true });
                }
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::ToggleMarkerCategory { category }) => {
                self.edit_marker_filter(Message::ToggleMarkerCategory(category));
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::ToggleMarkerSubcategory {
                category,
                subcategory,
            }) => {
                self.edit_marker_filter(Message::ToggleMarkerSubcategory(category, subcategory));
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::ShowAllMarkers) => {
                self.edit_marker_filter(Message::ShowAllMarkers);
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::HideAllMarkers) => {
                self.edit_marker_filter(Message::HideAllMarkers);
            }
            overlay_link::Event::Message(arclens_ipc::ToApp::ApplyPreset { id }) => {
                return self.update_map(Message::ApplyPreset(id));
            }
            overlay_link::Event::Listening | overlay_link::Event::Message(_) => {}
            overlay_link::Event::Failed(error) => {
                self.status.push(format!("Overlay link failed: {error}"));
            }
            overlay_link::Event::Incompatible(error) => {
                let note = format!(
                    "Overlay is out of date ({error}): rebuild it with \
                     `cargo build --release -p arclens-overlay`"
                );
                if !self.status.contains(&note) {
                    self.status.push(note);
                }
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
                let situation = hover.footer.map_or_else(Situation::default, |f| Situation {
                    place: if f.in_raid {
                        Place::Raid
                    } else {
                        Place::Workshop
                    },
                    sell_value: f.sell_value,
                });
                let side = match hover.item_side {
                    arclens_vision::Side::Left => arclens_ipc::ItemSide::Left,
                    arclens_vision::Side::Right => arclens_ipc::ItemSide::Right,
                };
                self.hover = Some((item.id.clone(), anchor, situation, side));
                self.push_hover_to_overlay();
                // The icon arrives later; `IconLoaded` resends the card.
                if let Some(load) = load {
                    return Task::perform(load, |(id, icon)| Message::IconLoaded(id, icon));
                }
            }
            vision::Event::Monitor(monitor) => {
                // Hover anchors are relative to the captured monitor, so the
                // overlay must live there, not where the app was launched.
                tracing::info!(?monitor, "game monitor");
                overlay_process::set_target(Some(monitor));
            }
            vision::Event::Gone => {
                self.hover = None;
                self.send(ToOverlay::ClearHover);
            }
            vision::Event::MapOpen(header) => {
                // Sticky: a title OCR can't place (mid-pan, partly covered)
                // keeps the map last recognised; only another map's title
                // changes it.
                let read = arclens_data::metaforge::map_for_title(&header.title);
                if read.is_some() {
                    self.last_map = read;
                }
                let map = read.or(self.last_map);
                if self.map_screen != Some(MapScreen { map }) {
                    tracing::info!(title = %header.title, ?map, condition = ?header.condition, "map open");
                }
                self.map_screen = Some(MapScreen { map });
                let mut task = Task::none();
                if let Some(map) = map {
                    let condition = game_condition(map, header.condition.as_deref());
                    if map != self.map {
                        map.clone_into(&mut self.map);
                        self.map_condition = None;
                    }
                    match condition {
                        ConditionLine::Condition(name) => self.map_condition = Some(name),
                        ConditionLine::Other => self.map_condition = None,
                        // Keep what was read before on this map.
                        ConditionLine::Unread => {}
                    }
                    self.sync_preset();
                    self.refresh_map_summary();
                    task = self.load_map_markers();
                }
                self.push_map_panel();
                return task;
            }
            vision::Event::MapClosed => self.on_map_closed(),
            vision::Event::MapLabels(read) => self.on_map_labels(&read),
            vision::Event::MapMotion(motion) => {
                self.game_view.motion = motion;
                self.push_map_view();
            }
            vision::Event::StationLevel(read) => self.on_station_level(&read),
            vision::Event::MainMenu(shown) => self.on_main_menu(shown),
            vision::Event::MapPointer(at) => self.send(ToOverlay::Pointer { at }),
            vision::Event::Unavailable(reason) => {
                tracing::info!(%reason, "item detection off");
                self.status.push(format!("Item detection off: {reason}"));
            }
        }
        Task::none()
    }

    fn on_map_closed(&mut self) {
        tracing::info!("map closed");
        self.map_screen = None;
        self.game_view = GameMapView::default();
        self.send(ToOverlay::HideMapPanel);
        self.send(ToOverlay::ClearMarkers);
        self.send(ToOverlay::Pointer { at: None });
        vision::set_drawn(vision::Drawn::default());
    }

    fn on_main_menu(&mut self, shown: bool) {
        tracing::info!(shown, "main menu");
        self.main_menu = shown.then(std::time::Instant::now);
        if shown {
            self.push_menu_card();
        } else {
            self.send(ToOverlay::HideMenuCard);
        }
    }

    /// Sends the main-menu card: conditions now and next, workshop progress.
    fn push_menu_card(&self) {
        if self.main_menu.is_none() {
            return;
        }
        let now = now_ms();
        let (active, upcoming) = match &self.events {
            Load::Ready(events) => {
                let agenda = arclens_core::agenda(events, now);
                let card = |e: &&arclens_core::ScheduledEvent, at_ms| arclens_ipc::CardEvent {
                    name: e.name.clone(),
                    map: e.map.clone(),
                    at_ms,
                };
                (
                    agenda.active.iter().map(|e| card(e, e.end_ms)).collect(),
                    agenda
                        .upcoming
                        .iter()
                        .take(6)
                        .map(|e| card(e, e.start_ms))
                        .collect(),
                )
            }
            _ => (Vec::new(), Vec::new()),
        };
        let workshop = match (&self.catalog, &self.progress) {
            (Load::Ready(catalog), Some(progress)) => {
                Some(catalog.stations.iter().fold((0, 0), |(b, t), s| {
                    (b + progress.level(&s.id).min(s.max_level), t + s.max_level)
                }))
            }
            _ => None,
        };
        self.send(ToOverlay::ShowMenuCard {
            card: arclens_ipc::MenuCard {
                active,
                upcoming,
                workshop,
            },
        });
    }

    /// A workshop station's page was on screen: take its level.
    fn on_station_level(&mut self, read: &arclens_vision::StationLevel) {
        let Load::Ready(catalog) = &self.catalog else {
            return;
        };
        let wanted = read.station.to_uppercase();
        let Some(station) = catalog
            .stations
            .iter()
            .map(|s| {
                (
                    s,
                    strsim::normalized_levenshtein(&wanted, &s.name.to_uppercase()),
                )
            })
            .filter(|&(_, score)| score >= 0.8)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(s, _)| s)
        else {
            tracing::debug!(station = %read.station, "unknown workshop station");
            return;
        };
        let level = read.level.min(station.max_level);
        let progress = self.progress.get_or_insert_with(Progress::default);
        if progress.level(&station.id) == level {
            return;
        }
        tracing::info!(station = %station.id, level, "workshop level read from the game");
        progress.stations.insert(station.id.clone(), level);
        let note = format!(
            "{} set to level {level} (read from the game).",
            station.name
        );
        self.status.retain(|s| !s.contains("(read from the game)"));
        self.status.push(note);
        self.progress_changed();
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

    fn set_quest_or_project(&mut self, message: Message) {
        let Load::Ready(catalog) = &self.catalog else {
            return;
        };
        let progress = self.progress.get_or_insert_with(Progress::default);
        match message {
            Message::SetQuestDone(quest, done) => {
                progress.set_quest_done(&quest, done, &catalog.quests);
            }
            Message::SetProjectPhases(project, phases) => {
                progress.projects.insert(project, phases);
            }
            _ => return,
        }
        self.progress_changed();
    }

    fn progress_changed(&self) {
        crate::progress::save(&self.progress_path, self.progress.as_ref());
        // Verdicts depend on progress: refresh what the overlay shows.
        self.push_selected_to_overlay();
        self.push_hover_to_overlay();
    }

    fn advice(&self, item: &Item, catalog: &Catalog, situation: Situation) -> arclens_core::Advice {
        advise_in(item, situation, self.progress.as_ref(), |id| {
            catalog.item(id)
        })
    }

    fn push_hover_to_overlay(&self) {
        let (Load::Ready(catalog), Some((id, anchor, situation, side))) =
            (&self.catalog, &self.hover)
        else {
            return;
        };
        if let Some(item) = catalog.item(id) {
            self.send(ToOverlay::ShowHover {
                item: Box::new(item.clone()),
                advice: self.advice(item, catalog, *situation),
                icon: self.icons.get(id).map(|icon| icon.path.clone()),
                recycle_names: recycle_names(item, catalog, situation.place),
                anchor: *anchor,
                item_side: *side,
            });
        }
    }

    /// The overlay's quick-search results: the top of [`Self::results`].
    fn push_search_results(&self) {
        let Load::Ready(catalog) = &self.catalog else {
            return;
        };
        let hits = self
            .results
            .iter()
            .take(SEARCH_HITS)
            .filter_map(|id| catalog.item(id))
            .map(|item| arclens_ipc::SearchHit {
                id: item.id.clone(),
                name: item.name.clone(),
                rarity: item.rarity,
                icon: self.icons.get(&item.id).map(|icon| icon.path.clone()),
            })
            .collect();
        self.send(ToOverlay::SearchResults {
            query: self.query.clone(),
            hits,
        });
    }

    fn push_selected_to_overlay(&self) {
        let (Load::Ready(catalog), Some(id)) = (&self.catalog, &self.selected) else {
            return;
        };
        if let Some(item) = catalog.item(id) {
            self.send(ToOverlay::ShowItem {
                item: Box::new(item.clone()),
                advice: self.advice(item, catalog, Situation::default()),
                icon: self.icons.get(id).map(|icon| icon.path.clone()),
                recycle_names: recycle_names(item, catalog, Place::Workshop),
            });
        }
    }

    /// Whether the screen is being captured now.
    fn capturing(&self) -> bool {
        match self.capture {
            CaptureMode::Auto => self.game_running,
            CaptureMode::Always => true,
            CaptureMode::Off => false,
        }
    }

    /// Clears what capture showed once it stops.
    fn capture_changed(&mut self, was_capturing: bool) {
        if was_capturing && !self.capturing() {
            self.hover = None;
            self.map_screen = None;
            self.game_view = GameMapView::default();
            self.main_menu = None;
            self.send(ToOverlay::HideMenuCard);
            self.send(ToOverlay::ClearHover);
            self.send(ToOverlay::HideMapPanel);
            self.send(ToOverlay::ClearMarkers);
            vision::set_drawn(vision::Drawn::default());
        }
    }

    fn set_tab(&mut self, tab: Tab) -> Task<Message> {
        self.tab = tab;
        self.now_ms = now_ms();
        match tab {
            Tab::Home => Task::batch([
                iced::widget::operation::focus(crate::views::home::SEARCH_ID),
                self.refresh_events_if_stale(),
            ]),
            Tab::Items => iced::widget::operation::focus(SEARCH_ID),
            Tab::Map => Task::batch([
                self.load_map_markers(),
                iced::widget::operation::focus(crate::views::map::SEARCH_ID),
            ]),
            Tab::Events => self.refresh_events_if_stale(),
            Tab::Progress | Tab::Settings => Task::none(),
        }
    }

    /// Map-tab messages.
    fn update_map(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectMap(map) => {
                if map != self.map {
                    self.map = map;
                    self.map_condition = None;
                }
                self.sync_preset();
                self.refresh_map_summary();
                return self.load_map_markers();
            }
            Message::SelectCondition(condition) => {
                self.map_condition = condition;
                self.sync_preset();
                self.refresh_map_summary();
                self.push_map_panel();
                self.push_map_markers();
            }
            Message::ApplyPreset(id) => {
                if let Some(preset) = self.presets.choose(&id, &self.map, self.map_condition) {
                    self.set_filter_from(&preset);
                }
            }
            Message::PresetDraft(name) => self.presets.draft = name,
            Message::PresetForMap(on) => self.presets.for_map = on,
            Message::PresetForCondition(on) => self.presets.for_condition = on,
            Message::SavePresetAs | Message::UpdatePreset => {
                if let Some(Load::Ready(markers)) = self.markers.get(&self.map) {
                    let kinds = arclens_core::marker_counts(markers);
                    if matches!(message, Message::SavePresetAs) {
                        self.presets.save_draft(
                            &self.marker_filter,
                            &kinds,
                            &self.map,
                            self.map_condition,
                        );
                    } else {
                        self.presets.update_active(&self.marker_filter, &kinds);
                    }
                }
                self.refresh_map_summary();
                self.push_map_panel();
            }
            Message::DeletePreset(id) => {
                self.presets.remove(&id);
                // A reverted default applies as shipped.
                if let Some(preset) = self.presets.active().cloned() {
                    self.set_filter_from(&preset);
                } else {
                    self.refresh_map_summary();
                    self.push_map_panel();
                }
            }
            Message::MarkersLoaded(map, result) => {
                if let Err(error) = &result {
                    tracing::warn!(%error, map, "markers unavailable");
                }
                let current = self.tab == Tab::Map && map == self.map;
                let on_screen = self
                    .map_screen
                    .is_some_and(|screen| screen.map == Some(map.as_str()));
                let selected = map == self.map;
                self.markers.insert(
                    map,
                    match result {
                        Ok(markers) => Load::Ready(markers),
                        Err(error) => Load::Failed(error),
                    },
                );
                if selected {
                    self.sync_preset();
                }
                self.refresh_map_summary();
                if on_screen {
                    self.push_map_panel();
                    self.push_map_markers();
                }
                // The search box only exists once markers are in.
                if current {
                    return iced::widget::operation::focus(crate::views::map::SEARCH_ID);
                }
            }
            Message::MarkerQuery(query) => {
                self.marker_query = query;
                self.refresh_map_summary();
            }
            Message::ExpandMarkerCategory(category) => {
                if !self.expanded_categories.remove(&category) {
                    self.expanded_categories.insert(category);
                }
            }
            _ => self.edit_marker_filter(message),
        }
        Task::none()
    }

    /// Starts loading the selected map's markers, unless already loaded.
    fn load_map_markers(&mut self) -> Task<Message> {
        if matches!(
            self.markers.get(&self.map),
            Some(Load::Ready(_) | Load::Loading)
        ) {
            return Task::none();
        }
        self.markers.insert(self.map.clone(), Load::Loading);
        let map = self.map.clone();
        Task::perform(
            data::load_markers(self.paths.clone(), map.clone()),
            move |result| Message::MarkersLoaded(map.clone(), result),
        )
    }

    fn edit_marker_filter(&mut self, message: Message) {
        let filter = &mut self.marker_filter;
        match message {
            Message::ToggleMarkerCategory(category) => filter.toggle_category(&category),
            Message::ToggleMarkerSubcategory(category, sub) => {
                filter.toggle_subcategory(&category, &sub);
            }
            Message::ShowAllMarkers => filter.show_all(),
            Message::HideAllMarkers => {
                if let Some(Load::Ready(markers)) = self.markers.get(&self.map) {
                    filter.hide_all(markers);
                }
            }
            _ => return,
        }
        crate::store::save(&self.paths.marker_filter(), &self.marker_filter);
        self.refresh_map_summary();
        self.push_map_panel();
        self.push_map_markers();
    }

    /// Applies the preset of the current map and condition if they changed
    /// since the filter was set (needs the map's markers).
    fn sync_preset(&mut self) {
        if !matches!(self.markers.get(&self.map), Some(Load::Ready(_))) {
            return;
        }
        if let Some(preset) = self.presets.on_context(&self.map, self.map_condition) {
            tracing::info!(preset = %preset.id, map = %self.map, condition = ?self.map_condition, "preset applied");
            self.set_filter_from(&preset);
        }
    }

    /// Makes `preset` the marker filter.
    fn set_filter_from(&mut self, preset: &arclens_core::Preset) {
        let Some(Load::Ready(markers)) = self.markers.get(&self.map) else {
            return;
        };
        self.marker_filter = preset.filter(&arclens_core::marker_counts(markers));
        crate::store::save(&self.paths.marker_filter(), &self.marker_filter);
        self.refresh_map_summary();
        self.push_map_panel();
        self.push_map_markers();
    }

    /// The bit of the current condition in markers' condition masks.
    fn condition_bit(&self, map: &str) -> Option<u8> {
        let condition = self.map_condition.filter(|_| map == self.map)?;
        arclens_data::metaforge::condition_on_map(map, condition).map(|(_, bit)| bit)
    }

    /// Place names read on the open map: re-anchor the view. A read whose
    /// names don't match carries the previous view forward by the motion
    /// tracked between the two reads.
    fn on_map_labels(&mut self, read: &vision::MapLabels) {
        let fit = self
            .map_screen
            .and_then(|screen| screen.map)
            .and_then(|map| {
                arclens_data::anchors::locate_view(
                    &read.labels,
                    read.frame,
                    &arclens_data::labels::labels_for(map),
                )
            });
        let view = &mut self.game_view;
        view.base = match fit {
            Some((transform, agree)) => {
                tracing::debug!(labels = read.labels.len(), agree, "map view located");
                Some(transform)
            }
            None => view
                .base
                .zip(read.moved)
                .map(|(base, motion)| moved(base, motion, read.frame)),
        };
        view.frame = read.frame;
        view.quest_panel_open = read.quests_open;
        view.motion = Some(arclens_vision::Motion::NONE);
        self.push_map_view();
    }

    /// Moves the overlay's markers to where the map is now, sending the
    /// marker set first if the overlay lacks it. Clears them while the view
    /// is unknown, rather than drawing them in the wrong place.
    fn push_map_view(&mut self) {
        match self.game_view.current() {
            None => {
                if std::mem::take(&mut self.game_view.markers_sent) {
                    self.send(ToOverlay::ClearMarkers);
                    vision::set_drawn(vision::Drawn::default());
                }
            }
            Some(transform) if self.game_view.markers_sent => {
                self.send(ToOverlay::MoveMarkers {
                    transform,
                    clip: Some(self.map_clip()),
                });
                self.report_drawn(transform);
            }
            Some(_) => self.push_map_markers(),
        }
    }

    /// Sends the open map's selected markers to the overlay, placed at the
    /// current view. All of them: the overlay draws those in the viewport,
    /// so a pan only needs a new transform.
    fn push_map_markers(&mut self) {
        self.game_view.markers_sent = false;
        let Some(MapScreen { map: Some(map) }) = self.map_screen else {
            return;
        };
        let Some(Load::Ready(markers)) = self.markers.get(map) else {
            return;
        };
        let Some(transform) = self.game_view.current() else {
            self.send(ToOverlay::ClearMarkers);
            vision::set_drawn(vision::Drawn::default());
            return;
        };
        let bit = self.condition_bit(map);
        let enabled = markers.iter().enumerate().filter(|(_, m)| {
            self.marker_filter.shows(m)
                && m.occurs_in(bit)
                // The game draws place names itself.
                && !(m.label.is_some() && m.category.to_lowercase().contains("label"))
        });
        let layout = arclens_core::layout(markers, enabled.map(|(i, _)| i));
        let shown: Vec<arclens_core::Marker> =
            layout.singles.iter().map(|&i| markers[i].clone()).collect();
        self.game_view.shown = shown.iter().map(|m| m.position).collect();
        self.game_view.shown_areas = layout
            .areas
            .iter()
            .map(|a| (a.center, a.hull.clone()))
            .collect();
        tracing::debug!(
            markers = shown.len(),
            areas = layout.areas.len(),
            ?transform,
            "markers sent"
        );
        self.send(ToOverlay::ShowMarkers {
            map: arclens_core::MapId::new(map),
            markers: shown,
            transform,
            areas: layout.areas,
            clip: Some(self.map_clip()),
        });
        self.game_view.markers_sent = true;
        self.report_drawn(transform);
    }

    /// Tells tracking where the overlay now draws (inside the viewport),
    /// so it ignores our own markers in the capture.
    fn report_drawn(&self, transform: arclens_core::Transform) {
        let clip = self.map_clip();
        let inside = |(x, y): (f32, f32)| {
            (clip.x - 0.02..=clip.x + clip.width + 0.02).contains(&x)
                && (clip.y - 0.02..=clip.y + clip.height + 0.02).contains(&y)
        };
        let at = |p: arclens_core::MapPoint| transform.apply((p.x, p.y));
        let badges = self
            .game_view
            .shown
            .iter()
            .map(|&p| at(p))
            .filter(|&p| inside(p))
            .collect();
        let mut outlines = Vec::new();
        for (center, hull) in &self.game_view.shown_areas {
            let c = at(*center);
            outlines.push(c);
            // Points along the outline, ~0.4 % of the screen apart.
            for (i, &a) in hull.iter().enumerate() {
                let (a, b) = (at(a), at(hull[(i + 1) % hull.len()]));
                let len = (b.0 - a.0).hypot(b.1 - a.1);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "small counts"
                )]
                let steps = ((len / 0.004).ceil() as usize).clamp(1, 400);
                #[allow(clippy::cast_precision_loss, reason = "small counts")]
                for k in 0..steps {
                    let t = k as f32 / steps as f32;
                    outlines.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                }
            }
        }
        outlines.retain(|&p| inside(p));
        vision::set_drawn(vision::Drawn { badges, outlines });
    }

    /// The in-game map's viewport, normalised: markers outside it would sit
    /// on the game's side panels.
    fn map_clip(&self) -> arclens_ipc::NormRect {
        let left = if self.game_view.quest_panel_open {
            MAP_VIEWPORT_LEFT_WITH_QUESTS
        } else {
            MAP_VIEWPORT[0]
        };
        arclens_ipc::NormRect {
            x: left,
            y: MAP_VIEWPORT[1],
            width: MAP_VIEWPORT[2] - left,
            height: MAP_VIEWPORT[3] - MAP_VIEWPORT[1],
        }
    }

    /// Rebuilds the Map tab's derived data after markers, map, query or
    /// filter changed.
    fn refresh_map_summary(&mut self) {
        self.map_summary = match self.markers.get(&self.map) {
            Some(Load::Ready(markers)) => {
                self.presets.edited = self.presets.active().is_some_and(|p| {
                    !p.matches(&self.marker_filter, &arclens_core::marker_counts(markers))
                });
                crate::views::map::MapSummary::new(
                    markers,
                    &self.marker_query,
                    &self.marker_filter,
                    self.condition_bit(&self.map),
                )
            }
            _ => crate::views::map::MapSummary::default(),
        };
        self.map_plot.clear();
    }

    /// Sends the map panel to the overlay while the in-game map is open.
    fn push_map_panel(&self) {
        let Some(MapScreen { map }) = self.map_screen else {
            return;
        };
        let map_name = map
            .and_then(|id| arclens_data::metaforge::MAPS.iter().find(|m| m.0 == id))
            .map_or("Unknown map", |m| m.1);
        let categories = match map.and_then(|m| Some((m, self.markers.get(m)?))) {
            Some((map, Load::Ready(markers))) => {
                panel_categories(markers, &self.marker_filter, self.condition_bit(map))
            }
            _ => Vec::new(),
        };
        let here = map == Some(self.map.as_str());
        let condition = self.map_condition.filter(|_| here);
        let presets = if here {
            self.presets
                .suited(&self.map, condition)
                .into_iter()
                .map(|p| arclens_ipc::PanelPreset {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    for_condition: !p.conditions.is_empty(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let panel = arclens_ipc::MapPanel {
            map_name: map_name.to_owned(),
            categories,
            condition: condition.map(str::to_owned),
            presets,
            active_preset: self.presets.active().map(|p| p.id.clone()),
            edited: self.presets.edited,
        };
        self.send(ToOverlay::ShowMapPanel { panel });
    }

    fn on_events_loaded(
        &mut self,
        result: Result<arclens_data::metaforge::Schedule, String>,
    ) -> Task<Message> {
        self.events_loaded_at = Some(std::time::Instant::now());
        self.events = match result {
            Ok(schedule) => {
                let mut events = schedule.events;
                self.events_region = schedule.region;
                if let (Some(asked), Some(got)) = (&self.settings.region, &self.events_region)
                    && asked != got
                {
                    tracing::warn!(asked, got, "schedule is for another region");
                }
                tracing::info!(events = events.len(), "event schedule loaded");
                // Sorted once here; the view relies on it every tick.
                events.sort_by_key(|e| (e.start_ms, e.end_ms));
                Load::Ready(events)
            }
            Err(error) => {
                tracing::warn!(%error, "event schedule unavailable");
                Load::Failed(error)
            }
        };
        self.request_event_icons()
    }

    /// Saves an overlay setting and applies it.
    fn set_overlay_setting(&mut self, message: &Message) {
        match *message {
            Message::SetOverlayScale(scale) => self.settings.overlay.scale = scale,
            Message::SetOverlayCorner(corner) => self.settings.overlay.corner = corner,
            _ => return,
        }
        crate::store::save(&self.paths.settings(), &self.settings);
        self.send(ToOverlay::Configure {
            settings: self.settings.overlay,
        });
    }

    /// Saves the player's server region and reloads the schedule for it.
    fn set_region(&mut self, region: String) -> Task<Message> {
        self.settings.region = Some(region);
        crate::store::save(&self.paths.settings(), &self.settings);
        self.events = Load::Loading;
        self.events_loaded_at = Some(std::time::Instant::now());
        Task::perform(
            data::load_events(self.paths.clone(), self.settings.region.clone()),
            Message::EventsLoaded,
        )
    }

    /// Fetches icons for scheduled events not seen before.
    fn request_event_icons(&mut self) -> Task<Message> {
        let Load::Ready(events) = &self.events else {
            return Task::none();
        };
        let catalog = match &self.catalog {
            Load::Ready(catalog) => Some(catalog.as_ref()),
            _ => None,
        };
        Task::batch(
            self.event_icons
                .request(events, catalog)
                .into_iter()
                .map(|load| Task::perform(load, |(url, icon)| Message::EventIconLoaded(url, icon))),
        )
    }

    /// Refetches the schedule once it is older than [`data::EVENTS_MAX_AGE`].
    fn refresh_events_if_stale(&mut self) -> Task<Message> {
        match self.events_loaded_at {
            Some(at) if at.elapsed() >= data::EVENTS_MAX_AGE => {
                // Keep showing the old schedule until the new one arrives.
                self.events_loaded_at = Some(std::time::Instant::now());
                Task::perform(
                    data::load_events(self.paths.clone(), self.settings.region.clone()),
                    Message::EventsLoaded,
                )
            }
            _ => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match self.tab {
            Tab::Home => self.view_home(),
            Tab::Events => self.view_events(),
            Tab::Map => self.view_map(),
            Tab::Progress => self.view_progress(),
            Tab::Settings => {
                crate::views::settings::view(self.settings.overlay, self.settings.region.as_deref())
            }
            Tab::Items => self.view_catalog_or_status(),
        };
        column![self.view_top_bar(), body, self.view_footer()].into()
    }

    fn view_map(&self) -> Element<'_, Message> {
        use crate::views::map::{MapView, Markers};
        let markers = match self.markers.get(&self.map) {
            None | Some(Load::Loading) => Markers::Loading,
            Some(Load::Ready(markers)) => Markers::Ready(markers),
            Some(Load::Failed(error)) => Markers::Failed(error),
        };
        crate::views::map::view(&MapView {
            maps: arclens_data::metaforge::MAPS,
            selected: &self.map,
            markers,
            summary: &self.map_summary,
            plot: &self.map_plot,
            query: &self.marker_query,
            expanded: &self.expanded_categories,
            conditions: arclens_data::metaforge::conditions(&self.map),
            condition: self.map_condition,
            presets: self.presets_view(),
        })
    }

    fn presets_view(&self) -> crate::views::map::PresetsView<'_> {
        crate::views::map::PresetsView {
            suited: self.presets.suited(&self.map, self.map_condition),
            presets: &self.presets,
        }
    }

    /// Typing on Home: look it up on the Items tab.
    fn home_search(&mut self, query: String) -> Task<Message> {
        self.query = query;
        self.tab = Tab::Items;
        Task::batch([
            self.refresh_results(),
            iced::widget::operation::focus(SEARCH_ID),
        ])
    }

    fn open_map(&mut self, map: String) -> Task<Message> {
        let load = self.update_map(Message::SelectMap(map));
        Task::batch([load, self.set_tab(Tab::Map)])
    }

    fn view_home(&self) -> Element<'_, Message> {
        use crate::views::home::{HomeView, LastMap, Status};
        let catalog = match &self.catalog {
            Load::Ready(catalog) => Some(catalog),
            _ => None,
        };
        let workshop = self
            .progress
            .as_ref()
            .zip(catalog)
            .map(|(progress, catalog)| {
                catalog
                    .stations
                    .iter()
                    .fold((0, 0), |(built, total), station| {
                        (
                            built + progress.level(&station.id).min(station.max_level),
                            total + station.max_level,
                        )
                    })
            });
        let last_map = self.last_map.and_then(|id| {
            let name = arclens_data::metaforge::MAPS.iter().find(|m| m.0 == id)?.1;
            let here = self.map == id;
            Some(LastMap {
                name,
                condition: self.map_condition.filter(|_| here),
                preset: self
                    .presets
                    .active()
                    .filter(|_| here)
                    .map(|p| p.name.as_str()),
            })
        });
        let status = vec![
            Status {
                label: "GAME",
                value: if self.game_running {
                    "Running".to_owned()
                } else {
                    "Not running".to_owned()
                },
                ok: self.game_running,
            },
            Status {
                label: "GAME CAPTURE",
                value: match self.capture {
                    CaptureMode::Auto if self.game_running => "On (auto)".to_owned(),
                    CaptureMode::Auto => "Waits for game".to_owned(),
                    CaptureMode::Always => "Always on".to_owned(),
                    CaptureMode::Off => "Off".to_owned(),
                },
                ok: self.capturing(),
            },
            Status {
                label: "OVERLAY",
                value: match (&self.overlay, self.overlay_visible) {
                    (None, _) => "Offline".to_owned(),
                    (Some(_), true) => "Shown".to_owned(),
                    (Some(_), false) => "Ready".to_owned(),
                },
                ok: self.overlay.is_some(),
            },
        ];
        crate::views::home::view(&HomeView {
            icon: &self.logo,
            status,
            events: match &self.events {
                Load::Ready(events) => Some(events),
                _ => None,
            },
            icons: &self.event_icons,
            now_ms: self.now_ms,
            maps: arclens_data::metaforge::MAPS,
            last_map,
            workshop,
            item_count: catalog.map(|c| c.items.len()),
        })
    }

    fn view_events(&self) -> Element<'_, Message> {
        match &self.events {
            Load::Loading => centered(text("Loading event schedule…").color(palette::TEXT_MUTED)),
            Load::Failed(error) => centered(
                column![
                    text("Could not load the event schedule")
                        .size(20)
                        .font(BOLD),
                    text(error).color(palette::TEXT_MUTED),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            ),
            Load::Ready(events) => crate::views::events::view(&crate::views::events::EventsView {
                events,
                icons: &self.event_icons,
                now_ms: self.now_ms,
                map_filter: self.event_map_filter.as_deref(),
                region: self.settings.region.as_deref(),
                served_region: self.events_region.as_deref(),
            }),
        }
    }

    fn view_catalog_or_status(&self) -> Element<'_, Message> {
        match &self.catalog {
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
        }
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

        let tabs = [
            ("Home", Tab::Home),
            ("Items", Tab::Items),
            ("Map", Tab::Map),
            ("Events", Tab::Events),
            ("Progress", Tab::Progress),
            ("Settings", Tab::Settings),
        ]
        .into_iter()
        .fold(row![].spacing(4), |r, (label, tab)| {
            r.push(tab_button(label, self.tab == tab, Message::SetTab(tab)))
        });
        let middle: Element<'_, Message> = if self.tab == Tab::Items {
            text_input("Search items…", &self.query)
                .id(SEARCH_ID)
                .on_input(Message::QueryChanged)
                .on_submit(Message::SelectFirst)
                .padding([8, 12])
                .size(15)
                .width(Length::Fill)
                .into()
        } else {
            Space::new().width(Length::Fill).into()
        };
        let bar = row![
            text("ARClens").size(20).font(BOLD),
            tabs,
            middle,
            status,
            pill_button(
                "Game capture",
                match (self.capture, self.game_running) {
                    (CaptureMode::Auto, true) => "auto · game running",
                    (CaptureMode::Auto, false) => "auto · waiting for game",
                    (CaptureMode::Always, _) => "always",
                    (CaptureMode::Off, _) => "off",
                },
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

        let detail: Element<'_, Message> = {
            match self.selected.as_ref().and_then(|id| catalog.item(id)) {
                Some(item) => {
                    let card = item_card(&ItemCard {
                        item,
                        advice: self.advice(item, catalog, Situation::default()),
                        icon: self.icons.get(&item.id).map(|icon| &icon.large),
                        recycle_names: recycle_names(item, catalog, Place::Workshop),
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
            }
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

    fn view_progress(&self) -> Element<'_, Message> {
        let Load::Ready(catalog) = &self.catalog else {
            return self.view_catalog_or_status();
        };
        crate::views::progress::view(&crate::views::progress::ProgressView {
            section: self.progress_section,
            stations: &catalog.stations,
            quests: &catalog.quests,
            projects: &catalog.projects,
            progress: self.progress.as_ref(),
        })
    }

    fn view_row<'a>(&'a self, item: &'a Item, catalog: &'a Catalog) -> Element<'a, Message> {
        let rarity = palette::rarity(item.rarity);
        let advice = self.advice(item, catalog, Situation::default());
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
        // Status messages only; data credits live in the README.
        if self.status.is_empty() {
            return Space::new().into();
        }
        container(
            text(self.status.join("   ·   "))
                .size(11)
                .color(palette::TEXT_MUTED),
        )
        .padding([6, 16])
        .width(Length::Fill)
        .into()
    }
}

fn recycle_names(item: &Item, catalog: &Catalog, place: Place) -> Vec<String> {
    breakdown(item, place)
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

fn tab_button(label: &str, active: bool, on_press: Message) -> Element<'_, Message> {
    button(
        text(label)
            .size(14)
            .font(if active { BOLD } else { Font::DEFAULT }),
    )
    .padding([6, 12])
    .on_press(on_press)
    .style(move |_, status| {
        let background = if active {
            with_alpha(palette::TEXT, 0.14)
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            with_alpha(palette::TEXT, 0.08)
        } else {
            Color::TRANSPARENT
        };
        button::Style {
            background: Some(background.into()),
            text_color: if active {
                palette::TEXT
            } else {
                palette::TEXT_MUTED
            },
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// The in-game map viewport, `[left, top, right, bottom]` as screen
/// fractions: markers outside it would sit on the game's side panels.
const MAP_VIEWPORT: [f32; 4] = [0.02, 0.105, 0.77, 0.90];
/// The viewport's left edge while the quest panel is open.
const MAP_VIEWPORT_LEFT_WITH_QUESTS: f32 = 0.275;

/// The marker filter as the overlay's map panel lists it.
fn panel_categories(
    markers: &[arclens_core::Marker],
    filter: &arclens_core::MarkerFilter,
    condition: Option<u8>,
) -> Vec<arclens_ipc::PanelCategory> {
    use arclens_core::humanize;
    arclens_core::marker_counts(markers.iter().filter(|m| m.occurs_in(condition)))
        .into_iter()
        .map(|(category, subs)| arclens_ipc::PanelCategory {
            id: category.to_owned(),
            label: humanize(category),
            count: subs.values().sum(),
            shown: filter.shows_category(category),
            subcategories: subs
                .into_iter()
                .filter(|(sub, _)| !sub.is_empty())
                .map(|(sub, count)| arclens_ipc::PanelCategory {
                    id: sub.to_owned(),
                    label: humanize(sub),
                    count,
                    shown: filter.shows_subcategory(category, sub),
                    subcategories: Vec::new(),
                })
                .collect(),
        })
        .collect()
}

/// What the map panel's condition line says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionLine {
    /// A condition of the map.
    Condition(&'static str),
    /// Other text: the game shows the player's name there when the raid
    /// has no condition.
    Other,
    /// Nothing read.
    Unread,
}

fn game_condition(map: &str, line: Option<&str>) -> ConditionLine {
    match line.map(str::trim).filter(|l| !l.is_empty()) {
        None => ConditionLine::Unread,
        Some(line) => arclens_data::metaforge::condition_on_map(map, line)
            .map_or(ConditionLine::Other, |(name, _)| {
                ConditionLine::Condition(name)
            }),
    }
}

/// Ctrl+1…5 switch tabs.
fn tab_shortcut(event: iced::keyboard::Event) -> Option<Message> {
    use iced::keyboard::{Event, Key};
    let Event::KeyPressed {
        key: Key::Character(c),
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    if !modifiers.command() {
        return None;
    }
    let tab = match c.as_str() {
        "1" => Tab::Home,
        "2" => Tab::Items,
        "3" => Tab::Map,
        "4" => Tab::Events,
        "5" => Tab::Progress,
        _ => return None,
    };
    Some(Message::SetTab(tab))
}

/// Wall-clock time in Unix milliseconds.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).padding(24).into()
}
