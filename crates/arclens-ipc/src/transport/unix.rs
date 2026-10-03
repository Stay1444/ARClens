//! Unix domain socket transport.

use std::path::PathBuf;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address(PathBuf);

impl Address {
    /// `$XDG_RUNTIME_DIR/arclens.sock`, else the temp dir (non-systemd
    /// systems).
    pub fn for_user() -> Self {
        Self(
            std::env::var_os("XDG_RUNTIME_DIR")
                .map_or_else(std::env::temp_dir, PathBuf::from)
                .join("arclens.sock"),
        )
    }

    pub fn at(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn describe(&self) -> String {
        self.0.display().to_string()
    }

    pub async fn connect(&self) -> std::io::Result<(OwnedReadHalf, OwnedWriteHalf)> {
        Ok(UnixStream::connect(&self.0).await?.into_split())
    }

    pub async fn bind(&self) -> std::io::Result<Listener> {
        // A live instance answers; a dead one left only the file behind.
        if UnixStream::connect(&self.0).await.is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "another ARClens instance is running",
            ));
        }
        let _ = std::fs::remove_file(&self.0);
        Ok(Listener(UnixListener::bind(&self.0)?))
    }
}

#[derive(Debug)]
pub struct Listener(UnixListener);

impl Listener {
    pub async fn accept(&mut self) -> std::io::Result<(OwnedReadHalf, OwnedWriteHalf)> {
        let (stream, _) = self.0.accept().await?;
        Ok(stream.into_split())
    }
}
