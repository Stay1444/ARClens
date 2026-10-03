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
    /// Drawn markers, rebuilt only when they or the surface change.
    marker_cache: iced::widget::canvas::Cache,
    /// Map-screen panel (conditions + marker filter), the clickable part.
    panel: map_panel::PanelState,
    /// Sends to the companion app while connected.
    outbox: Option<ipc::Outbox>,
    /// Surface size in logical pixels, once known.
    screen: Option<iced::Size>,
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
            || self.hover.is_some()
            || self.panel.panel.is_some()
            || ((!self.markers.is_empty() || !self.areas.is_empty()) && self.transform.is_some())
    }

    /// Where the overlay takes pointer input: everywhere while interactive,
    /// the map panel while it is shown, nowhere otherwise (click-through).
    fn input_rect(&self) -> Option<iced::Rectangle> {
        let screen = self.screen?;
        if self.interactive {
            return Some(iced::Rectangle::with_size(screen));
        }
        self.panel.bounds(screen)
    }
}

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message(multi))]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Event),
    Panel(map_panel::PanelMessage),
    /// Surface size changed (logical pixels).
    Resized(iced::Size),
    /// For the platform's shell.
    Shell(shell::ShellMessage),
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
        Message::Resized(size) => {
            if state.screen == Some(size) {
                return Task::none();
            }
            state.screen = Some(size);
            state.marker_cache.clear();
            return shell::input_task(state);
        }
        Message::Shell(msg) => return shell::update(state, msg),
        // Layer-shell requests, which only exist on Linux.
        #[allow(unreachable_patterns, reason = "the layer-shell variants")]
        _ => return Task::none(),
    };
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
                screen: state.screen,
                ..Overlay::default()
            };
            Task::none()
        }
        ipc::Event::Message(msg) => apply(state, msg),
    };
    task.chain(shell::sync_surface(state))
}

fn apply(state: &mut Overlay, msg: ToOverlay) -> Task<Message> {
    match msg {
        ToOverlay::Hello(_) => {}
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
            state.item = Some(ShownItem {
                item,
                advice,
                // Icons are ~256² PNGs; decoding once here is cheap and keeps
                // `view` free of I/O.
                icon: icon
                    .as_deref()
                    .and_then(|p| arclens_ui::decode_icon(p, 128)),
                recycle_names,
            });
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
            let shown = ShownItem {
                item,
                advice,
                icon: icon
                    .as_deref()
                    .and_then(|p| arclens_ui::decode_icon(p, 128)),
                recycle_names,
            };
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
        ToOverlay::HideMapPanel => {
            state.panel.panel = None;
            state.panel.expanded = false;
            state.panel.hovered = false;
            return shell::input_task(state);
        }
    }
    Task::none()
}

fn style(_: &Overlay, theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: theme.palette().text,
    }
}
