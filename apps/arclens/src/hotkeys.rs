//! Global hotkeys as an iced subscription.

use arclens_hotkeys::Action;
use futures::SinkExt;
use futures::channel::mpsc;
use iced::Subscription;

#[derive(Debug, Clone)]
pub enum Event {
    Pressed(Action),
    /// The portal is unavailable; hotkeys are disabled for this session.
    Unavailable(String),
}

pub fn subscription() -> Subscription<Event> {
    Subscription::run(|| {
        iced::stream::channel(8, async |mut output: mpsc::Sender<Event>| {
            let mut forward = output.clone();
            let result = arclens_hotkeys::listen(move |action| {
                let _ = forward.try_send(Event::Pressed(action));
            })
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, "global hotkeys unavailable");
                let _ = output.send(Event::Unavailable(error.to_string())).await;
            }
            futures::future::pending::<()>().await;
        })
    })
}
