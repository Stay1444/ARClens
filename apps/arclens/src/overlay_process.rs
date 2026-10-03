//! Launches `arclens-overlay` and keeps it running.
//!
//! The overlay is started with `--exit-with-app`, so it exits by itself when
//! this app goes away (even on a crash or `kill -9`). Here we only restart
//! it if *it* exits, with exponential backoff.

use arclens_ipc::MonitorRect;
use futures::SinkExt;
use futures::channel::mpsc;
use iced::Subscription;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// Set to `0` to not launch the overlay (e.g. when running it by hand from
/// `cargo run -p arclens-overlay` while developing it).
pub const AUTOSTART_ENV: &str = "ARCLENS_OVERLAY_AUTOSTART";
/// Explicit path to the overlay binary.
pub const BIN_ENV: &str = "ARCLENS_OVERLAY_BIN";

const OVERLAY_BIN: &str = "arclens-overlay";
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// A run longer than this counts as healthy and resets the backoff.
const HEALTHY_RUN: Duration = Duration::from_secs(30);

/// Which monitor the overlay should open on (`None`: the active one).
/// Changing it restarts the overlay there.
fn target() -> &'static watch::Sender<Option<MonitorRect>> {
    static TARGET: OnceLock<watch::Sender<Option<MonitorRect>>> = OnceLock::new();
    TARGET.get_or_init(|| watch::channel(None).0)
}

/// Moves the overlay to `monitor` (restarting it) if it isn't there already.
pub fn set_target(monitor: Option<MonitorRect>) {
    target().send_if_modified(|current| {
        let changed = *current != monitor;
        *current = monitor;
        changed
    });
}

#[derive(Debug, Clone)]
pub enum Event {
    /// The overlay could not be started at all; we stop trying.
    Unavailable(String),
}

pub fn enabled() -> bool {
    std::env::var(AUTOSTART_ENV).map_or(true, |v| v != "0")
}

pub fn subscription() -> Subscription<Event> {
    Subscription::run(|| {
        iced::stream::channel(4, async |mut output: mpsc::Sender<Event>| {
            if let Err(error) = supervise().await {
                tracing::warn!(%error, "overlay unavailable");
                let _ = output.send(Event::Unavailable(error)).await;
            }
            futures::future::pending::<()>().await;
        })
    })
}

async fn supervise() -> Result<(), String> {
    let bin = locate();
    let mut target_rx = target().subscribe();
    let mut backoff = MIN_BACKOFF;
    loop {
        let monitor = *target_rx.borrow_and_update();
        let mut command = tokio::process::Command::new(&bin);
        command.arg("--exit-with-app").kill_on_drop(true);
        if let Some(monitor) = monitor {
            command.arg(MonitorRect::FLAG).arg(monitor.to_arg());
        }
        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", bin.display()))?;
        tracing::info!(bin = %bin.display(), pid = child.id(), ?monitor, "overlay started");

        tokio::select! {
            status = child.wait() => {
                let status = status.map_err(|e| e.to_string())?;
                tracing::warn!(%status, "overlay exited");
                if started.elapsed() > HEALTHY_RUN {
                    backoff = MIN_BACKOFF;
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            changed = target_rx.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
                tracing::info!("moving overlay to another monitor");
                let _ = child.kill().await;
            }
        }
    }
}

/// `$ARCLENS_OVERLAY_BIN`, else next to our own executable (cargo's
/// `target/<profile>/` or an install prefix's `bin/`), else `$PATH`.
fn locate() -> PathBuf {
    if let Some(path) = std::env::var_os(BIN_ENV) {
        return path.into();
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(OVERLAY_BIN)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| OVERLAY_BIN.into())
}
