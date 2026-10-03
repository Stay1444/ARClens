//! `arclens` — the companion app.
//!
//! Owns all state: game data, global hotkeys, and the overlay connection.
//! The overlay (`arclens-overlay`) is a separate, dumb renderer that this
//! app drives over IPC; see `docs/architecture/overview.md`.

pub mod app;
mod data;
mod event_icons;
mod game_process;
mod hotkeys;
mod icons;
mod overlay_link;
mod overlay_process;
mod paths;
mod platform;
mod presets;
mod progress;
mod store;
mod views;
mod vision;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "arclens=info,arclens_data=info".into()),
        )
        .init();

    let paths = paths::Paths::discover()?;
    platform::integrate();
    // Before the event loop: Windows delivers hotkeys through this thread.
    let _hotkeys = arclens_hotkeys::init()
        .map_err(|error| tracing::warn!(%error, "global hotkeys unavailable"))
        .ok();
    iced::application(
        move || app::App::boot(paths.clone()),
        app::App::update,
        app::App::view,
    )
    .title("ARClens")
    .subscription(app::App::subscription)
    .theme(app::App::theme)
    .window(iced::window::Settings {
        size: iced::Size::new(1180.0, 760.0),
        // Wayland: KDE finds the icon through the desktop file named
        // after the app id. Windows and X11 take the icon directly.
        platform_specific: platform::window_platform(),
        icon: platform::window_icon(),
        ..Default::default()
    })
    .run()?;
    Ok(())
}
