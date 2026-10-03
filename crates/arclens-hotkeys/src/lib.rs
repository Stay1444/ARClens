//! Global hotkeys: react to a few key combinations while the game has
//! focus. Key presses still reach the game; we are only notified.
//!
//! One API, one backend per platform:
//! - Linux: the XDG `GlobalShortcuts` portal (`backend/portal.rs`);
//! - Windows: `RegisterHotKey`, through the `global-hotkey` crate
//!   (`backend/windows.rs`).

#[cfg(target_os = "linux")]
#[path = "backend/portal.rs"]
mod backend;
#[cfg(windows)]
#[path = "backend/windows.rs"]
mod backend;

/// Everything a hotkey can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Show / hide the overlay.
    ToggleOverlay,
    /// Make the overlay accept mouse & keyboard (e.g. to type a search).
    ToggleInteractive,
}

impl Action {
    pub const ALL: [Self; 2] = [Self::ToggleOverlay, Self::ToggleInteractive];

    /// Stable id registered with the portal. Never change these: users'
    /// customised bindings are keyed by them.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "toggle-overlay",
            Self::ToggleInteractive => "toggle-interactive",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "Show or hide the ARClens overlay",
            Self::ToggleInteractive => "Let the ARClens overlay take mouse and keyboard input",
        }
    }

    /// Suggested default, in XDG shortcuts syntax. The user can override it
    /// where the platform lets them (KDE's shortcut settings).
    pub const fn preferred_trigger(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "CTRL+SHIFT+O",
            Self::ToggleInteractive => "CTRL+SHIFT+I",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The platform can't deliver global hotkeys (no portal, keys taken by
    /// another app, …).
    #[error("{0}")]
    Unavailable(String),
}

/// Keeps hotkeys registered; drop it to unregister.
#[derive(Debug)]
pub struct Guard(#[allow(dead_code, reason = "held for its Drop")] backend::Guard);

/// Prepares hotkeys. Call it on the main thread before the UI's event loop
/// starts and keep the guard: on Windows the keys are delivered through
/// that thread's message loop. Elsewhere it does nothing.
pub fn init() -> Result<Guard, Error> {
    backend::init().map(Guard)
}

/// Calls `on_action` for each hotkey press, forever. Fails fast when the
/// platform can't deliver hotkeys, so callers can tell the user.
pub async fn listen(on_action: impl FnMut(Action)) -> Result<(), Error> {
    backend::listen(on_action).await
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
