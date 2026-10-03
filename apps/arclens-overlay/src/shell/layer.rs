//! Linux: a `wlr-layer-shell` surface on the **OVERLAY** layer (the only
//! layer KWin stacks above fullscreen windows), click-through except for
//! an input region.
//!
//! The surface only exists while there is something to draw. A mapped
//! surface, even a fully transparent one, over a fullscreen game stops the
//! compositor from scanning the game out directly, which costs frames.

#[path = "outputs.rs"]
mod outputs;

use crate::{Message, Overlay};
use iced::Task;
use iced_layershell::actions::ActionCallback;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

/// What the layer shell needs to remember.
#[derive(Debug, Default)]
pub struct State {
    /// Output (monitor) name to open on; `None` = the active one.
    output: Option<String>,
}

/// Messages only this shell sends itself (none).
#[derive(Debug, Clone, Copy)]
pub enum ShellMessage {}

pub fn run() -> anyhow::Result<()> {
    // Open on the monitor the game is captured from, if the app told us.
    let output = crate::requested_monitor().and_then(|monitor| {
        let name = outputs::output_for(monitor);
        tracing::info!(?monitor, output = ?name, "target monitor");
        name
    });
    iced_layershell::daemon(
        move || {
            let overlay = Overlay::new(State {
                output: output.clone(),
            });
            (overlay, Task::none())
        },
        namespace,
        crate::update,
        crate::view_window,
    )
    .style(crate::style)
    .scale_factor(crate::scale_factor)
    .subscription(crate::subscription)
    .settings(Settings {
        fonts: arclens_ui::theme::FONTS.iter().map(|&f| f.into()).collect(),
        default_font: arclens_ui::theme::BODY,
        layer_settings: LayerShellSettings {
            // No surface until there is something to show.
            start_mode: StartMode::Background,
            ..Default::default()
        },
        ..Default::default()
    })
    .run()?;
    Ok(())
}

fn namespace() -> String {
    // Shown by compositors (e.g. KWin window rules) to identify the surface.
    String::from("arclens-overlay")
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

/// Maps the surface while there is content, unmaps it otherwise.
pub fn sync_surface(state: &mut Overlay) -> Task<Message> {
    match (state.has_content(), state.surface) {
        (true, None) => {
            let (id, task) =
                Message::layershell_open(surface_settings(state.shell.output.as_ref()));
            tracing::debug!("mapping overlay surface");
            state.surface = Some(id);
            task.chain(input_task(state))
        }
        (false, Some(id)) => {
            tracing::debug!("unmapping overlay surface");
            state.surface = None;
            Task::done(Message::RemoveWindow(id))
        }
        _ => Task::none(),
    }
}

/// The surface's size in the compositor's logical pixels, from a resize
/// event: `iced_layershell` reports it before the app's scale.
pub fn surface_size(reported: iced::Size, _scale: f32) -> iced::Size {
    reported
}

/// Applies [`Overlay::input_rect`] as the surface's input region, and the
/// matching keyboard mode.
pub fn input_task(state: &Overlay) -> Task<Message> {
    let Some(id) = state.surface else {
        return Task::none();
    };
    #[allow(clippy::cast_possible_truncation, reason = "screen pixels fit i32")]
    let rect = state.input_rect().map(|r| {
        (
            r.x as i32,
            r.y as i32,
            r.width.ceil() as i32,
            r.height.ceil() as i32,
        )
    });
    // Interactive mode is the user asking for mouse and keyboard (to type
    // a quick search): take the keyboard until it is toggled off.
    // Otherwise keyboard only while the pointer is on the panel: else the
    // compositor may hand us focus and the game loses it.
    let keyboard = if state.interactive {
        KeyboardInteractivity::Exclusive
    } else if rect.is_some() && state.panel.hovered {
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "same signature as other shells"
)]
pub fn update(_: &mut Overlay, message: ShellMessage) -> Task<Message> {
    match message {}
}

/// Nothing to watch beyond the core's subscriptions.
pub fn subscription(_: &Overlay) -> iced::Subscription<Message> {
    iced::Subscription::none()
}
