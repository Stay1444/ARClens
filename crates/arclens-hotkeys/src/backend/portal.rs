//! Linux: the XDG `GlobalShortcuts` portal.
//!
//! Wayland deliberately gives no client a global view of the keyboard, so
//! the only sanctioned way to react to keys while the game is focused is to
//! ask the desktop portal. On KDE Plasma the first bind shows a dialog where
//! the user confirms or changes the keys; afterwards they live in
//! System Settings → Shortcuts. Key presses still reach the game — the
//! portal only notifies us.
//!
//! The portal identifies apps by their `.desktop` file, so ARClens must be
//! installed with one for bindings to persist.

use crate::{Action, Error};
use ashpd::desktop::CreateSessionOptions;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures::StreamExt;

impl From<ashpd::Error> for Error {
    fn from(error: ashpd::Error) -> Self {
        Self::Unavailable(format!("GlobalShortcuts portal error: {error}"))
    }
}

/// Nothing to set up ahead: the portal session starts in [`listen`].
#[derive(Debug)]
pub struct Guard;

#[allow(clippy::unnecessary_wraps, reason = "same signature as other backends")]
pub fn init() -> Result<Guard, Error> {
    Ok(Guard)
}

/// Binds all [`Action`]s through the portal and calls `on_action` for each
/// activation, forever.
///
/// The portal session lives as long as this future; drop it to unbind.
/// Fails fast if the portal is missing (e.g. GNOME < 48 or no
/// xdg-desktop-portal running) so callers can fall back / warn the user.
pub async fn listen(mut on_action: impl FnMut(Action)) -> Result<(), Error> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await?;

    let shortcuts: Vec<NewShortcut> = Action::ALL
        .iter()
        .map(|a| NewShortcut::new(a.id(), a.description()).preferred_trigger(a.preferred_trigger()))
        .collect();
    let bound = portal
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await?
        .response()?;
    for shortcut in bound.shortcuts() {
        tracing::info!(
            id = shortcut.id(),
            trigger = shortcut.trigger_description(),
            "global shortcut bound"
        );
    }

    let mut activated = portal.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if let Some(action) = Action::from_id(event.shortcut_id()) {
            on_action(action);
        } else {
            tracing::warn!(id = event.shortcut_id(), "unknown shortcut activated");
        }
    }
    Ok(())
}
