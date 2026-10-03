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
mod outputs;
mod view;

use arclens_core::{Advice, Item, Marker, Transform};
use arclens_ipc::ToOverlay;
use iced::{Color, Subscription, Task};
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
    hover: Option<(ShownItem, arclens_ipc::NormRect)>,
    markers: Vec<Marker>,
    transform: Option<Transform>,
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
            || (!self.markers.is_empty() && self.transform.is_some())
    }

    /// Creates or removes the surface to match [`Self::has_content`].
    fn sync_surface(&mut self) -> Task<Message> {
        match (self.has_content(), self.surface) {
            (true, None) => {
                let (id, task) = Message::layershell_open(surface_settings(self.output.as_ref()));
                tracing::debug!("mapping overlay surface");
                self.surface = Some(id);
                task
            }
            (false, Some(id)) => {
                tracing::debug!("unmapping overlay surface");
                self.surface = None;
                Task::done(Message::RemoveWindow(id))
            }
            _ => Task::none(),
        }
    }
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Event),
}

fn namespace() -> String {
    // Shown by compositors (e.g. KWin window rules) to identify the surface.
    String::from("arclens-overlay")
}

fn subscription(_: &Overlay) -> Subscription<Message> {
    // Surfaces closed by the compositor (e.g. output unplugged) are forgotten
    // by `sync_surface` on the next change; nothing to listen for here.
    ipc::subscription().map(Message::Ipc)
}

fn update(state: &mut Overlay, message: Message) -> Task<Message> {
    let Message::Ipc(event) = message else {
        return Task::none();
    };
    let task = match event {
        ipc::Event::Connected => {
            tracing::info!("connected to companion app");
            state.connected = true;
            Task::none()
        }
        ipc::Event::Disconnected => {
            // Without the companion app there is nothing trustworthy to show.
            let (output, surface) = (state.output.take(), state.surface.take());
            *state = Overlay {
                output,
                surface,
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
            let keyboard = if interactive {
                KeyboardInteractivity::OnDemand
            } else {
                KeyboardInteractivity::None
            };
            // TODO(input-region): also swap the input region so clicks reach
            // the overlay while interactive; see docs/ROADMAP.md (M1).
            if let Some(id) = state.surface {
                return Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: keyboard,
                });
            }
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
            markers, transform, ..
        } => {
            state.markers = markers;
            state.transform = Some(transform);
        }
        ToOverlay::ShowHover {
            item,
            advice,
            icon,
            recycle_names,
            anchor,
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
            state.hover = Some((shown, anchor));
        }
        ToOverlay::ClearHover => state.hover = None,
        ToOverlay::ClearMarkers => {
            state.markers.clear();
            state.transform = None;
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
