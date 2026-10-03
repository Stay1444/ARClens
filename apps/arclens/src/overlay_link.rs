//! Server side of the overlay IPC, as an iced subscription.
//!
//! One overlay connection at a time; a new connection replaces the old one.

use arclens_ipc::{Hello, PROTOCOL_VERSION, ToApp, ToOverlay};
use futures::SinkExt;
use futures::channel::mpsc;
use iced::Subscription;
use tokio::net::UnixListener;
use tokio::sync::mpsc as tokio_mpsc;

/// Cloneable handle for sending to the connected overlay.
#[derive(Debug, Clone)]
pub struct OverlayHandle(tokio_mpsc::UnboundedSender<ToOverlay>);

impl OverlayHandle {
    /// Sends a message; silently dropped if the overlay just disconnected
    /// (a `Disconnected` event follows).
    pub fn send(&self, msg: ToOverlay) {
        let _ = self.0.send(msg);
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Listening,
    Connected(OverlayHandle),
    Message(ToApp),
    Disconnected,
    Failed(String),
}

pub fn subscription() -> Subscription<Event> {
    Subscription::run(|| {
        iced::stream::channel(32, async |mut output: mpsc::Sender<Event>| {
            if let Err(error) = serve(&mut output).await {
                let _ = output.send(Event::Failed(error.to_string())).await;
            }
            futures::future::pending::<()>().await;
        })
    })
}

async fn serve(output: &mut mpsc::Sender<Event>) -> Result<(), arclens_ipc::Error> {
    let path = arclens_ipc::socket_path();
    // A previous instance may have left the socket behind. If another
    // instance is alive the connect succeeds and we refuse to steal it.
    if tokio::net::UnixStream::connect(&path).await.is_ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            "another ARClens instance is running",
        )
        .into());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    let _ = output.send(Event::Listening).await;
    tracing::info!(path = %path.display(), "waiting for overlay");

    loop {
        let (stream, _) = listener.accept().await?;
        let (mut rx, mut tx) = arclens_ipc::split(stream);
        let (handle_tx, mut handle_rx) = tokio_mpsc::unbounded_channel();

        tx.send(&ToOverlay::Hello(Hello {
            protocol: PROTOCOL_VERSION,
        }))
        .await?;
        let _ = output
            .send(Event::Connected(OverlayHandle(handle_tx)))
            .await;
        tracing::info!("overlay connected");

        loop {
            tokio::select! {
                incoming = rx.recv::<ToApp>() => match incoming {
                    Ok(Some(ToApp::Hello(hello))) => {
                        if let Err(error) = arclens_ipc::check_version(&hello) {
                            tracing::warn!(%error, "dropping incompatible overlay");
                            break;
                        }
                    }
                    Ok(Some(ToApp::Bye) | None) => break,
                    Ok(Some(msg)) => { let _ = output.send(Event::Message(msg)).await; }
                    Err(error) => { tracing::warn!(%error, "overlay connection error"); break; }
                },
                Some(msg) = handle_rx.recv() => {
                    if tx.send(&msg).await.is_err() { break; }
                }
            }
        }
        let _ = output.send(Event::Disconnected).await;
        tracing::info!("overlay disconnected");
    }
}
