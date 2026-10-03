//! Bridges the tokio-based IPC client into an iced [`Subscription`].
//!
//! `iced_layershell` drives its own executor, so the socket lives on a
//! dedicated thread with a current-thread tokio runtime and forwards
//! messages over a futures channel.

use arclens_ipc::{Hello, PROTOCOL_VERSION, ToApp, ToOverlay};
use futures::SinkExt;
use futures::channel::mpsc;
use iced::Subscription;
use std::time::Duration;

/// Events surfaced to the overlay's update loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// Connected; send replies through the [`Outbox`].
    Connected(Outbox),
    Message(ToOverlay),
    Disconnected,
}

/// Sends messages to the companion app while connected.
#[derive(Debug, Clone)]
pub struct Outbox(tokio::sync::mpsc::UnboundedSender<ToApp>);

impl Outbox {
    pub fn send(&self, msg: ToApp) {
        // Fails only once the connection is gone, which `Disconnected`
        // reports anyway.
        let _ = self.0.send(msg);
    }
}

pub fn subscription() -> Subscription<Event> {
    Subscription::run(|| {
        iced::stream::channel(32, async |output: mpsc::Sender<Event>| {
            std::thread::Builder::new()
                .name("arclens-ipc".into())
                .spawn(move || run(output))
                .ok();
            // Keep the stream alive; the thread owns the sender.
            futures::future::pending::<()>().await;
        })
    })
}

fn run(mut output: mpsc::Sender<Event>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            tracing::error!(%error, "failed to start IPC runtime");
            return;
        }
    };

    // When the companion app spawned us, we must not outlive it: exit once the
    // connection drops, or if it never comes up.
    let exit_with_app = std::env::args().any(|a| a == EXIT_WITH_APP_FLAG);
    if exit_with_app {
        watch_parent();
    }

    runtime.block_on(async move {
        let mut failed_attempts = 0u32;
        loop {
            let mut connected = false;
            if let Err(error) = connect_once(&mut output, &mut connected).await {
                tracing::debug!(%error, "IPC connection ended");
            }
            if exit_with_app && (connected || failed_attempts >= MAX_ATTEMPTS_WHEN_SPAWNED) {
                tracing::info!("companion app gone; exiting");
                std::process::exit(0);
            }
            failed_attempts = if connected { 0 } else { failed_attempts + 1 };
            if output.send(Event::Disconnected).await.is_err() {
                return; // UI is gone.
            }
            tokio::time::sleep(RETRY_DELAY).await;
        }
    });
}

/// Exits once stdin closes: the app that spawned us holds the other end of
/// that pipe, so this fires however it ends. Without it an orphaned overlay
/// could attach to the next app instance instead of its own.
fn watch_parent() {
    let spawned = std::thread::Builder::new()
        .name("arclens-parent".into())
        .spawn(|| {
            let mut sink = Vec::new();
            let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
            tracing::info!("companion app gone (stdin closed); exiting");
            std::process::exit(0);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "cannot watch the companion app");
    }
}

/// Passed by the companion app when it spawns the overlay.
pub const EXIT_WITH_APP_FLAG: &str = "--exit-with-app";
const RETRY_DELAY: Duration = Duration::from_secs(2);
/// ~10 s of retries before a spawned overlay gives up.
const MAX_ATTEMPTS_WHEN_SPAWNED: u32 = 5;

async fn connect_once(
    output: &mut mpsc::Sender<Event>,
    connected: &mut bool,
) -> Result<(), arclens_ipc::Error> {
    let stream = tokio::net::UnixStream::connect(arclens_ipc::socket_path()).await?;
    let (mut rx, mut tx) = arclens_ipc::split(stream);
    tx.send(&ToApp::Hello(Hello {
        protocol: PROTOCOL_VERSION,
    }))
    .await?;

    let (outbox, mut outgoing) = tokio::sync::mpsc::unbounded_channel();
    let mut outbox = Some(Outbox(outbox));
    loop {
        tokio::select! {
            incoming = rx.recv::<ToOverlay>() => {
                let Some(msg) = incoming? else { break };
                if let ToOverlay::Hello(hello) = &msg {
                    arclens_ipc::check_version(hello)?;
                    *connected = true;
                    if let Some(outbox) = outbox.take() {
                        let _ = output.send(Event::Connected(outbox)).await;
                    }
                    continue;
                }
                if output.send(Event::Message(msg)).await.is_err() {
                    break;
                }
            }
            Some(msg) = outgoing.recv() => tx.send(&msg).await?,
        }
    }
    Ok(())
}
