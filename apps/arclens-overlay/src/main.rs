//! `arclens-overlay` — the in-game overlay.
//!
//! A full-screen `wlr-layer-shell` surface on the **OVERLAY** layer (the only
//! layer KWin stacks above fullscreen windows), click-through by default.
//! It is a dumb renderer: all state comes from the companion app over IPC
//! (see `arclens-ipc`). It never touches the game process.
//!
//! The surface only exists while there is something to draw. A mapped
//! surface — even a fully transparent one — over a fullscreen game stops the
//! compositor from scanning the game out directly, which costs frames.

mod ipc;
mod map_panel;
mod outputs;
mod view;

use arclens_core::{Advice, Item, Marker, Transform};
use arclens_ipc::ToOverlay;
use iced::{Color, Subscription, Task};
use iced_layershell::actions::ActionCallback;
use iced_layershell::daemon;
use iced_layershell::reexport::{
    Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;

fn main() -> Result<(), iced_layershell::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "arclens_overlay=info".into()),
        )
        .init();

    // Open on the monitor the game is captured from, if the app told us.
    let output = outputs::requested_monitor().and_then(|monitor| {
        let name = outputs::output_for(monitor);
        tracing::info!(?monitor, output = ?name, "target monitor");
        name
    });

    daemon(
        move || Overlay::new(output.clone()),
        namespace,
        update,
        view_window,
    )
    .style(style)
    .subscription(subscription)
    .settings(Settings {
        layer_settings: LayerShellSettings {
            // No surface until there is something to show.
            start_mode: StartMode::Background,
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
}

/// One surface, one view.
fn view_window(state: &Overlay, _window: IcedId) -> iced::Element<'_, Message> {
    view::view(state)
}

/// The full-screen, click-through surface on the OVERLAY layer.
fn surface_settings(output: Option<&String>) -> NewLayerShellSettings {
    NewLayerShellSettings {
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        exclusive_zone: Some(-1),
        keyboard_interactivity: KeyboardInteractivity::None,
        events_transparent: true,
        output_option: output.map_or(OutputOption::Active, |name| {
            OutputOption::OutputName(name.clone())
        }),
        namespace: Some(namespace()),
        ..NewLayerShellSettings::default()
    }
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
    /// Output (monitor) name to open on; `None` = the active one.
    output: Option<String>,
    /// The layer surface, while one exists.
    surface: Option<IcedId>,
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
    fn new(output: Option<String>) -> (Self, Task<Message>) {
        (
            Self {
                output,
                ..Self::default()
            },
            Task::none(),
        )
    }

    /// Whether anything would be drawn.
    fn has_content(&self) -> bool {
        self.visible
            || self.hover.is_some()
            || self.panel.panel.is_some()
            || ((!self.markers.is_empty() || !self.areas.is_empty()) && self.transform.is_some())
    }

    /// Creates or removes the surface to match [`Self::has_content`].
    fn sync_surface(&mut self) -> Task<Message> {
        match (self.has_content(), self.surface) {
            (true, None) => {
                let (id, task) = Message::layershell_open(surface_settings(self.output.as_ref()));
                tracing::debug!("mapping overlay surface");
                self.surface = Some(id);
                task.chain(self.input_task())
            }
            (false, Some(id)) => {
                tracing::debug!("unmapping overlay surface");
                self.surface = None;
                Task::done(Message::RemoveWindow(id))
            }
            _ => Task::none(),
        }
    }

    /// Where the surface takes pointer input: everywhere while interactive,
    /// the map panel while it is shown, nowhere otherwise (click-through).
    fn input_rect(&self) -> Option<iced::Rectangle> {
        let screen = self.screen?;
        if self.interactive {
            return Some(iced::Rectangle::with_size(screen));
        }
        self.panel.bounds(screen)
    }

    /// Applies [`Self::input_rect`] and the matching keyboard mode.
    fn input_task(&self) -> Task<Message> {
        let Some(id) = self.surface else {
            return Task::none();
        };
        #[allow(clippy::cast_possible_truncation, reason = "screen pixels fit i32")]
        let rect = self.input_rect().map(|r| {
            (
                r.x as i32,
                r.y as i32,
                r.width.ceil() as i32,
                r.height.ceil() as i32,
            )
        });
        // Keyboard only while interactive or the pointer is on the panel:
        // otherwise the compositor may hand us focus and the game loses it.
        let keyboard = if self.interactive || (rect.is_some() && self.panel.hovered) {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::None
        };
        Task::done(Message::SetInputRegion {
            id,
            callback: ActionCallback::new(move |region| {
                if let Some((x, y, width, height)) = rect {
                    region.add(x, y, width, height);
                }
            }),
        })
        .chain(Task::done(Message::KeyboardInteractivityChange {
            id,
            keyboard_interactivity: keyboard,
        }))
    }
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Event),
    Panel(map_panel::PanelMessage),
    /// Surface size changed (logical pixels).
    Resized(iced::Size),
}

fn namespace() -> String {
    // Shown by compositors (e.g. KWin window rules) to identify the surface.
    String::from("arclens-overlay")
}

fn subscription(_: &Overlay) -> Subscription<Message> {
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
                state.input_task()
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
            return state.input_task();
        }
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
            let (output, surface, screen) =
                (state.output.take(), state.surface.take(), state.screen);
            *state = Overlay {
                output,
                surface,
                screen,
                ..Overlay::default()
            };
            Task::none()
        }
        ipc::Event::Message(msg) => apply(state, msg),
    };
    task.chain(state.sync_surface())
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
            return state.input_task();
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
            ..
        } => {
            state.markers = markers;
            state.areas = areas;
            state.transform = Some(transform);
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
            state.marker_cache.clear();
        }
        ToOverlay::ShowMapPanel { panel } => {
            let was_shown = state.panel.panel.is_some();
            state.panel.panel = Some(panel);
            if !was_shown {
                return state.input_task();
            }
        }
        ToOverlay::HideMapPanel => {
            state.panel.panel = None;
            state.panel.expanded = false;
            state.panel.hovered = false;
            return state.input_task();
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
