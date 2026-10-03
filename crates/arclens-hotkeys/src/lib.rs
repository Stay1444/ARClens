//! Global hotkeys on Wayland through the XDG `GlobalShortcuts` portal.
//!
//! Wayland deliberately gives no client a global view of the keyboard, so
//! the only sanctioned way to react to keys while the game is focused is to
//! ask the desktop portal. On KDE Plasma the first bind shows a dialog where
//! the user confirms or changes the keys; afterwards they live in
//! System Settings → Shortcuts. Key presses still reach the game — the
//! portal only notifies us.
//!
//! The portal identifies apps by their `.desktop` file, so ARClens must be
//! installed with one (see `packaging/`) for bindings to persist.

use ashpd::desktop::CreateSessionOptions;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures::StreamExt;

/// Everything a hotkey can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Show / hide the overlay.
    ToggleOverlay,
    /// Make the overlay accept mouse & keyboard (e.g. to type a search).
    ToggleInteractive,
    /// Open the overlay in interactive mode with the search box focused.
    QuickSearch,
}

impl Action {
    pub const ALL: [Self; 3] = [
        Self::ToggleOverlay,
        Self::ToggleInteractive,
        Self::QuickSearch,
    ];

    /// Stable id registered with the portal. Never change these: users'
    /// customised bindings are keyed by them.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "toggle-overlay",
            Self::ToggleInteractive => "toggle-interactive",
            Self::QuickSearch => "quick-search",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "Show or hide the ARClens overlay",
            Self::ToggleInteractive => "Let the ARClens overlay take mouse and keyboard input",
            Self::QuickSearch => "Search for an item in the ARClens overlay",
        }
    }

    /// Suggested default, in XDG shortcuts syntax. The user can override it.
    pub const fn preferred_trigger(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "CTRL+SHIFT+O",
            Self::ToggleInteractive => "CTRL+SHIFT+I",
            Self::QuickSearch => "CTRL+SHIFT+F",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("GlobalShortcuts portal error: {0}")]
    Portal(#[from] ashpd::Error),
}

/// Binds all [`Action`]s and calls `on_action` for each activation, forever.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_round_trip_and_are_unique() {
        let ids: HashSet<_> = Action::ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids.len(), Action::ALL.len());
        for action in Action::ALL {
            assert_eq!(Action::from_id(action.id()), Some(action));
        }
        assert_eq!(Action::from_id("nope"), None);
    }
}
