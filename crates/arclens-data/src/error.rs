use std::path::PathBuf;

/// Errors produced while fetching, parsing or caching game data.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}
