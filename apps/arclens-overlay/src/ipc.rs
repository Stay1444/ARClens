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
    Connected,
    Message(ToOverlay),
    Disconnected,
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

    runtime.block_on(async move {
        loop {
            if let Err(error) = connect_once(&mut output).await {
                tracing::debug!(%error, "IPC connection ended");
            }
            if output.send(Event::Disconnected).await.is_err() {
                return; // UI is gone.
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

async fn connect_once(output: &mut mpsc::Sender<Event>) -> Result<(), arclens_ipc::Error> {
    let stream = tokio::net::UnixStream::connect(arclens_ipc::socket_path()).await?;
    let (mut rx, mut tx) = arclens_ipc::split(stream);
    tx.send(&ToApp::Hello(Hello {
        protocol: PROTOCOL_VERSION,
    }))
    .await?;

    while let Some(msg) = rx.recv::<ToOverlay>().await? {
        if let ToOverlay::Hello(hello) = &msg {
            arclens_ipc::check_version(hello)?;
            let _ = output.send(Event::Connected).await;
            continue;
        }
        if output.send(Event::Message(msg)).await.is_err() {
            break;
        }
    }
    Ok(())
}
