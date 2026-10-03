//! Windows: `RegisterHotKey`, through the `global-hotkey` crate.
//!
//! The keys are fixed here (the [`Action`] defaults); Windows has no
//! system place to rebind them. Registration fails when another program
//! holds the same combination.

use crate::{Action, Error};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::OnceLock;

/// Registered hotkey ids and what they trigger.
fn bindings() -> &'static OnceLock<Vec<(u32, Action)>> {
    static BINDINGS: OnceLock<Vec<(u32, Action)>> = OnceLock::new();
    &BINDINGS
}

/// Holds the registration (a hidden message window on this thread).
pub struct Guard(#[allow(dead_code, reason = "held for its Drop")] GlobalHotKeyManager);

impl std::fmt::Debug for Guard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Guard").finish_non_exhaustive()
    }
}

/// The key combination for an action, from its XDG-style trigger.
fn hotkey(action: Action) -> HotKey {
    let key = match action {
        Action::ToggleOverlay => Code::KeyO,
        Action::ToggleInteractive => Code::KeyI,
    };
    HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), key)
}

pub fn init() -> Result<Guard, Error> {
    let manager = GlobalHotKeyManager::new()
        .map_err(|e| Error::Unavailable(format!("global hotkeys: {e}")))?;
    let mut bound = Vec::new();
    for action in Action::ALL {
        let key = hotkey(action);
        match manager.register(key) {
            Ok(()) => {
                tracing::info!(
                    id = action.id(),
                    trigger = action.preferred_trigger(),
                    "hotkey bound"
                );
                bound.push((key.id(), action));
            }
            Err(error) => {
                tracing::warn!(id = action.id(), %error, "hotkey not available");
            }
        }
    }
    if bound.is_empty() {
        return Err(Error::Unavailable(
            "every hotkey is taken by another program".to_owned(),
        ));
    }
    let _ = bindings().set(bound);
    Ok(Guard(manager))
}

pub async fn listen(mut on_action: impl FnMut(Action)) -> Result<(), Error> {
    let bound = bindings()
        .get()
        .ok_or_else(|| Error::Unavailable("hotkeys were not set up".to_owned()))?;
    loop {
        // The receiver blocks; wait on a worker thread.
        let event = tokio::task::spawn_blocking(|| GlobalHotKeyEvent::receiver().recv())
            .await
            .map_err(|e| Error::Unavailable(e.to_string()))?
            .map_err(|e| Error::Unavailable(e.to_string()))?;
        if event.state() != HotKeyState::Pressed {
            continue;
        }
        if let Some(&(_, action)) = bound.iter().find(|(id, _)| *id == event.id()) {
            on_action(action);
        }
    }
}
