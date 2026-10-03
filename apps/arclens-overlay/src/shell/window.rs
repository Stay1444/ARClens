//! Windows: a transparent, undecorated, always-on-top window over the
//! primary monitor (the one Graphics Capture records), click-through.
//!
//! - The window is opened once and kept: showing a window activates it,
//!   which would take focus from the game whenever the map opens. Empty, it
//!   draws nothing. Exclusive-fullscreen games cover it; borderless
//!   windowed works.
//! - Windows has no per-region click-through, so the map panel's "input
//!   region" is emulated: while the panel is shown the cursor position is
//!   polled, and click-through turns off only while it is over the panel.

use crate::{Message, Overlay};
use iced::window::{self, Level, Position};
use iced::{Point, Size, Subscription, Task};
use std::time::Duration;

/// How often the cursor is checked while the map panel is shown.
const CURSOR_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, Default)]
pub struct State {
    /// Physical pixels per logical pixel, once known.
    scale: Option<f32>,
    /// Clicks currently reach the overlay (not passed through).
    takes_input: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ShellMessage {
    /// The window opened; its id, then the monitor's size and its scale.
    Opened(window::Id),
    Placed(window::Id, Option<Size>, f32),
    /// Global cursor position, physical pixels.
    Cursor(Option<Point>),
}

pub fn run() -> anyhow::Result<()> {
    iced::daemon(
        || (Overlay::new(State::default()), Task::none()),
        crate::update,
        crate::view_window,
    )
    .title(|_: &Overlay, _| String::from("ARClens overlay"))
    .style(crate::style)
    .subscription(crate::subscription)
    .run()?;
    Ok(())
}

/// Opens the window on first content; never closes it (see the module docs).
pub fn sync_surface(state: &mut Overlay) -> Task<Message> {
    if state.surface.is_some() || !state.has_content() {
        return Task::none();
    }
    let (id, opened) = window::open(window::Settings {
        position: Position::Specific(Point::ORIGIN),
        size: Size::new(1.0, 1.0),
        decorations: false,
        transparent: true,
        resizable: false,
        level: Level::AlwaysOnTop,
        exit_on_close_request: false,
        platform_specific: window::settings::PlatformSpecific {
            skip_taskbar: true,
            ..Default::default()
        },
        ..window::Settings::default()
    });
    state.surface = Some(id);
    tracing::debug!("opening overlay window");
    opened.map(|id| Message::Shell(ShellMessage::Opened(id)))
}

/// Click-through everywhere, except over the panel (see `Cursor`) or
/// while interactive.
pub fn input_task(state: &Overlay) -> Task<Message> {
    let Some(id) = state.surface else {
        return Task::none();
    };
    if state.interactive || state.shell.takes_input {
        window::disable_mouse_passthrough(id)
    } else {
        window::enable_mouse_passthrough(id)
    }
}

pub fn update(state: &mut Overlay, message: ShellMessage) -> Task<Message> {
    match message {
        ShellMessage::Opened(id) => window::monitor_size(id).then(move |size| {
            window::scale_factor(id)
                .map(move |scale| Message::Shell(ShellMessage::Placed(id, size, scale)))
        }),
        ShellMessage::Placed(id, size, scale) => {
            state.shell.scale = Some(scale);
            let size = size.unwrap_or(Size::new(1920.0, 1080.0));
            tracing::info!(?size, scale, "overlay window placed");
            window::move_to::<Message>(id, Point::ORIGIN)
                .chain(window::resize(id, size))
                .chain(input_task(state))
        }
        ShellMessage::Cursor(position) => {
            let over_panel = position
                .zip(state.shell.scale)
                .and_then(|(cursor, scale)| {
                    let region = state.input_rect()?;
                    // The window sits at the desktop origin.
                    Some(region.contains(Point::new(cursor.x / scale, cursor.y / scale)))
                })
                .unwrap_or(false);
            let mut tasks = Vec::new();
            if over_panel != state.panel.hovered {
                state.panel.hovered = over_panel;
            }
            if over_panel != state.shell.takes_input {
                state.shell.takes_input = over_panel;
                tasks.push(input_task(state));
            }
            Task::batch(tasks)
        }
    }
}

/// Polls the cursor while the map panel is up and the overlay is not
/// interactive anyway.
pub fn subscription(state: &Overlay) -> Subscription<Message> {
    if state.panel.panel.is_none() || state.interactive {
        return Subscription::none();
    }
    iced::time::every(CURSOR_POLL).map(|_| {
        let position = match mouse_position::mouse_position::Mouse::get_mouse_position() {
            mouse_position::mouse_position::Mouse::Position { x, y } =>
            {
                #[allow(clippy::cast_precision_loss, reason = "screen pixels")]
                Some(Point::new(x as f32, y as f32))
            }
            mouse_position::mouse_position::Mouse::Error => None,
        };
        Message::Shell(ShellMessage::Cursor(position))
    })
}
