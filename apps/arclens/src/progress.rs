//! Saving and loading the player's workshop progress.

use arclens_core::Progress;
use std::path::Path;

/// `None` when the player never set any progress (so advice stays purely
/// value-based) or the file is unreadable.
pub fn load(path: &Path) -> Option<Progress> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes)
        .map_err(|error| tracing::warn!(%error, "ignoring unreadable progress file"))
        .ok()
}

pub fn save(path: &Path, progress: Option<&Progress>) {
    let result = match progress {
        Some(progress) => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                let json = serde_json::to_vec_pretty(progress).map_err(std::io::Error::other)?;
                std::fs::write(path, json)
            }),
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
    };
    if let Err(error) = result {
        tracing::warn!(%error, path = %path.display(), "could not save progress");
    }
}
