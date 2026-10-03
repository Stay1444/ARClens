//! `arclens-overlay` — the in-game overlay.
//!
//! A full-screen, transparent, click-through layer over the game. It is a
//! dumb renderer: all state comes from the companion app over IPC (see
//! `arclens-ipc`). It never touches the game process.
//!
//! The overlay itself (state, update, view) is the same everywhere; how it
//! gets on screen is the platform's [`shell`]:
//! - Linux: a `wlr-layer-shell` surface on the OVERLAY layer
//!   (`shell/layer.rs`);
//! - Windows: a topmost transparent window (`shell/window.rs`).

mod ipc;
mod map_panel;
mod menu_card;
mod search;
mod tooltip;
mod view;

#[cfg(target_os = "linux")]
#[path = "shell/layer.rs"]
mod shell;
#[cfg(windows)]
#[path = "shell/window.rs"]
mod shell;

use arclens_core::{Advice, Item, Marker, Transform};
use arclens_ipc::{MonitorRect, ToOverlay};
use iced::{Color, Subscription, Task};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "arclens_overlay=info".into()),
        )
        .init();
    shell::run()
}

/// The user's scale, applied on top of the output's.
fn scale_factor(state: &Overlay, _window: iced::window::Id) -> f32 {
    state.scale()
}

/// One surface, one view.
fn view_window(state: &Overlay, _window: iced::window::Id) -> iced::Element<'_, Message> {
    view::view(state)
}

/// The `--monitor` argument (the captured monitor), if given.
pub fn requested_monitor() -> Option<MonitorRect> {
    let mut args = std::env::args()
        .skip_while(|a| a != MonitorRect::FLAG)
        .skip(1);
    args.next().as_deref().and_then(MonitorRect::from_arg)
}

/// The item card currently on screen.
#[derive(Debug)]
pub struct ShownItem {
    item: Box<Item>,
    advice: Advice,
    icon: Option<iced::widget::image::Handle>,
    recycle_names: Vec<String>,
}

impl ShownItem {
    fn new(
        item: Box<Item>,
        advice: Advice,
        icon: Option<&std::path::Path>,
        recycle_names: Vec<String>,
    ) -> Self {
        Self {
            item,
            advice,
            // Icons are ~256² PNGs; decoding once here is cheap and keeps
            // `view` free of I/O.
            icon: icon.and_then(|p| arclens_ui::decode_icon(p, 128)),
            recycle_names,
        }
    }
}

/// Everything the overlay currently displays.
#[derive(Debug, Default)]
pub struct Overlay {
    /// What the platform's shell keeps.
    shell: shell::State,
    /// The surface (window), while one exists.
    surface: Option<iced::window::Id>,
    connected: bool,
    visible: bool,
    interactive: bool,
    item: Option<ShownItem>,
    /// Item detected under the cursor in game, with the game tooltip's
    /// position. Shown whether or not `visible` is set.
    hover: Option<(ShownItem, arclens_ipc::NormRect, arclens_ipc::ItemSide)>,
    markers: Vec<Marker>,
    areas: Vec<arclens_core::MarkerArea>,
    transform: Option<Transform>,
    /// Where markers may be drawn (the game's map viewport).
    clip: Option<arclens_ipc::NormRect>,
    /// The pointer over the map, normalised, for marker tooltips.
    pointer: Option<(f32, f32)>,
    /// Drawn markers, rebuilt only when they or the surface change.
    marker_cache: iced::widget::canvas::Cache,
    /// Map-screen panel (conditions + marker filter), the clickable part.
    panel: map_panel::PanelState,
    /// Quick item search, shown while interactive.
    search: search::SearchState,
    /// The main-menu card, while the game shows its main menu.
    menu_card: Option<arclens_ipc::MenuCard>,
    /// Wall clock (Unix ms) for the card's countdowns.
    now_ms: i64,
    /// Sends to the companion app while connected.
    outbox: Option<ipc::Outbox>,
    /// Surface size in the compositor's logical pixels, once known.
    surface_size: Option<iced::Size>,
    /// The surface in layout pixels (`surface_size / settings.scale`):
    /// what views lay out in.
    screen: Option<iced::Size>,
    /// The user's settings from the app.
    settings: arclens_ipc::OverlaySettings,
}

impl Overlay {
    fn new(shell: shell::State) -> Self {
        Self {
            shell,
            ..Self::default()
        }
    }

    /// Whether anything would be drawn.
    fn has_content(&self) -> bool {
        self.visible
            || self.interactive
            || self.hover.is_some()
            || self.panel.panel.is_some()
            || self.menu_card.is_some()
            || ((!self.markers.is_empty() || !self.areas.is_empty()) && self.transform.is_some())
    }

    /// Where the overlay takes pointer input, in the compositor's logical
    /// pixels: everywhere while interactive, the map panel while it is
    /// shown, nowhere otherwise (click-through).
    fn input_rect(&self) -> Option<iced::Rectangle> {
        if self.interactive {
            return Some(iced::Rectangle::with_size(self.surface_size?));
        }
        self.panel
            .bounds(self.screen?)
            .map(|rect| rect * iced::Transformation::scale(self.scale()))
    }

    /// The user's overlay scale.
    fn scale(&self) -> f32 {
        self.settings.clamped_scale()
    }

    /// Recomputes the layout size after the surface or the scale changed.
    fn update_screen(&mut self) {
        let scale = self.scale();
        self.screen = self
            .surface_size
            .map(|size| iced::Size::new(size.width / scale, size.height / scale));
        self.marker_cache.clear();
    }
}

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message(multi))]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Event),
    Panel(map_panel::PanelMessage),
    Search(search::SearchMessage),
    /// Surface size changed (logical pixels).
    Resized(iced::Size),
    /// For the platform's shell.
    Shell(shell::ShellMessage),
    /// Once a second while the menu card counts down.
    Tick,
}

fn subscription(state: &Overlay) -> Subscription<Message> {
    // Surfaces closed by the compositor (e.g. output unplugged) are forgotten
    // by `sync_surface` on the next change; nothing to listen for here.
    Subscription::batch([
        ipc::subscription().map(Message::Ipc),
        iced::event::listen_with(|event, _, _| match event {
            iced::Event::Window(
                iced::window::Event::Resized(size) | iced::window::Event::Opened { size, .. },
            ) => Some(Message::Resized(size)),
            _ => None,
        }),
        shell::subscription(state),
        if state.menu_card.is_some() {
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        },
    ])
}

fn update(state: &mut Overlay, message: Message) -> Task<Message> {
    let event = match message {
        Message::Ipc(event) => event,
        Message::Panel(msg) => {
            let resize = matches!(
                msg,
                map_panel::PanelMessage::ToggleExpanded | map_panel::PanelMessage::Hover(_)
            );
            if let Some(to_app) = state.panel.update(msg)
                && let Some(outbox) = &state.outbox
            {
                outbox.send(to_app);
            }
            return if resize {
                shell::input_task(state)
            } else {
                Task::none()
            };
        }
        Message::Search(msg) => {
            if let Some(to_app) = state.search.update(msg)
                && let Some(outbox) = &state.outbox
            {
                outbox.send(to_app);
            }
            return Task::none();
        }
        Message::Resized(size) => {
            let size = shell::surface_size(size, state.scale());
            if state.surface_size == Some(size) {
                return Task::none();
            }
            state.surface_size = Some(size);
            state.update_screen();
            let input = shell::input_task(state);
            // The surface may have just opened for interactive mode.
            return if state.interactive {
                input.chain(iced::widget::operation::focus(search::INPUT_ID))
            } else {
                input
            };
        }
        Message::Shell(msg) => return shell::update(state, msg),
        Message::Tick => {
            state.now_ms = now_ms();
            return Task::none();
        }
        // Layer-shell requests, which only exist on Linux.
        #[allow(unreachable_patterns, reason = "the layer-shell variants")]
        _ => return Task::none(),
    };
    // Ready to type as soon as the surface (mapped below if need be) has
    // the keyboard.
    let focus_search = matches!(
        event,
        ipc::Event::Message(ToOverlay::SetInteractive { interactive: true })
    );
    let task = match event {
        ipc::Event::Connected(outbox) => {
            tracing::info!("connected to companion app");
            state.connected = true;
            state.outbox = Some(outbox);
            Task::none()
        }
        ipc::Event::Disconnected => {
            // Without the companion app there is nothing trustworthy to show.
            *state = Overlay {
                shell: std::mem::take(&mut state.shell),
                surface: state.surface.take(),
                surface_size: state.surface_size,
                screen: state.surface_size,
                ..Overlay::default()
            };
            Task::none()
        }
        ipc::Event::Message(msg) => apply(state, msg),
    };
    let task = task.chain(shell::sync_surface(state));
    if focus_search {
        task.chain(iced::widget::operation::focus(search::INPUT_ID))
    } else {
        task
    }
}

fn apply(state: &mut Overlay, msg: ToOverlay) -> Task<Message> {
    match msg {
        ToOverlay::Hello(_) => {}
        ToOverlay::Configure { settings } => {
            tracing::info!(?settings, "overlay settings");
            state.settings = settings;
            state.update_screen();
            return shell::input_task(state);
        }
        ToOverlay::SetVisible { visible } => {
            tracing::info!(visible, "overlay visibility changed");
            state.visible = visible;
        }
        ToOverlay::SetInteractive { interactive } => {
            tracing::info!(interactive, "overlay interactivity changed");
            state.interactive = interactive;
            return shell::input_task(state);
        }
        ToOverlay::ShowItem {
            item,
            advice,
            icon,
            recycle_names,
        } => {
            tracing::info!(item = %item.name, "showing item");
            state.item = Some(ShownItem::new(item, advice, icon.as_deref(), recycle_names));
        }
        ToOverlay::ShowMarkers {
            markers,
            transform,
            areas,
            clip,
            ..
        } => {
            state.markers = markers;
            state.areas = areas;
            state.transform = Some(transform);
            state.clip = clip;
            state.marker_cache.clear();
        }
        ToOverlay::Pointer { at } => state.pointer = at,
        ToOverlay::MoveMarkers { transform, clip } => {
            state.transform = Some(transform);
            state.clip = clip;
            state.marker_cache.clear();
        }
        ToOverlay::ShowHover {
            item,
            advice,
            icon,
            recycle_names,
            anchor,
            item_side,
        } => {
            tracing::debug!(item = %item.name, "hover");
            let shown = ShownItem::new(item, advice, icon.as_deref(), recycle_names);
            state.hover = Some((shown, anchor, item_side));
        }
        ToOverlay::ClearHover => state.hover = None,
        ToOverlay::ClearMarkers => {
            state.markers.clear();
            state.areas.clear();
            state.transform = None;
            state.clip = None;
            state.marker_cache.clear();
        }
        ToOverlay::ShowMapPanel { panel } => {
            let was_shown = state.panel.panel.is_some();
            state.panel.panel = Some(panel);
            if !was_shown {
                return shell::input_task(state);
            }
        }
        ToOverlay::ShowMenuCard { card } => {
            state.now_ms = now_ms();
            state.menu_card = Some(card);
        }
        ToOverlay::HideMenuCard => state.menu_card = None,
        ToOverlay::SearchResults { query, hits } => state.search.results(&query, hits),
        ToOverlay::HideMapPanel => {
            state.pointer = None;
            state.panel.panel = None;
            state.panel.expanded = false;
            state.panel.hovered = false;
            return shell::input_task(state);
        }
    }
    Task::none()
}

/// Wall-clock time in Unix milliseconds.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn style(_: &Overlay, theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    }
}
