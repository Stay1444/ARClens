//! `arclens` — the companion app.
//!
//! Owns all state: game data, global hotkeys, and the overlay connection.
//! The overlay (`arclens-overlay`) is a separate, dumb renderer that this
//! app drives over IPC; see `docs/architecture/overview.md`.

mod app;
mod data;
mod hotkeys;
mod overlay_link;
mod overlay_process;
mod paths;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "arclens=info,arclens_data=info".into()),
        )
        .init();

    let paths = paths::Paths::discover()?;
    iced::application(
        move || app::App::boot(paths.clone()),
        app::App::update,
        app::App::view,
    )
    .title("ARClens")
    .subscription(app::App::subscription)
    .theme(app::App::theme)
    .window_size((900.0, 640.0))
    .run()?;
    Ok(())
}
