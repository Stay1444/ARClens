//! `arclens-overlay` — the in-game overlay.
//!
//! A full-screen `wlr-layer-shell` surface on the **OVERLAY** layer (the only
//! layer KWin stacks above fullscreen windows), click-through by default.
//! It is a dumb renderer: all state comes from the companion app over IPC
//! (see `arclens-ipc`). It never touches the game process.

mod ipc;
mod outputs;
mod view;

use arclens_core::{Advice, Item, Marker, Transform};
use arclens_ipc::ToOverlay;
use iced::{Color, Subscription, Task};
use iced_layershell::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
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
    let start_mode = outputs::requested_monitor()
        .and_then(|monitor| {
            let name = outputs::output_for(monitor);
            tracing::info!(?monitor, output = ?name, "target monitor");
            name
        })
        .map_or(StartMode::Active, StartMode::TargetScreen);

    application(Overlay::default, namespace, update, view::view)
        .style(style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
                layer: Layer::Overlay,
                exclusive_zone: -1,
                keyboard_interactivity: KeyboardInteractivity::None,
                events_transparent: true,
                start_mode,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
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

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Event),
}

fn namespace() -> String {
    // Shown by compositors (e.g. KWin window rules) to identify the surface.
    String::from("arclens-overlay")
}

fn subscription(_: &Overlay) -> Subscription<Message> {
    ipc::subscription().map(Message::Ipc)
}

fn update(state: &mut Overlay, message: Message) -> Task<Message> {
    let Message::Ipc(event) = message else {
        return Task::none();
    };
    match event {
        ipc::Event::Connected => {
            tracing::info!("connected to companion app");
            state.connected = true;
        }
        ipc::Event::Disconnected => {
            // Without the companion app there is nothing trustworthy to show.
            *state = Overlay::default();
        }
        ipc::Event::Message(msg) => return apply(state, msg),
    }
    Task::none()
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
            return Task::done(Message::KeyboardInteractivityChange(keyboard));
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
