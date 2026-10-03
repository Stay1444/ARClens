//! Small JSON settings files under the config directory.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;

/// `None` when the file is missing or unreadable (logged).
pub fn load<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes)
        .map_err(|error| tracing::warn!(%error, path = %path.display(), "ignoring unreadable file"))
        .ok()
}

pub fn save<T: Serialize>(path: &Path, value: &T) {
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            let json = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
            std::fs::write(path, json)
        });
    if let Err(error) = result {
        tracing::warn!(%error, path = %path.display(), "could not save");
    }
}
