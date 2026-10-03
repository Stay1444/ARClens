//! Where the app listens and the overlay connects, per platform:
//!
//! - Unix: a socket at `$XDG_RUNTIME_DIR/arclens.sock` (see
//!   [`Endpoint::for_user`]);
//! - Windows: the named pipe `\\.\pipe\arclens-<user>`.
//!
//! Both carry the same byte stream; the framing lives in the crate root.

use crate::{Receiver, Sender, split};

#[cfg(unix)]
#[path = "unix.rs"]
mod backend;
#[cfg(windows)]
#[path = "windows.rs"]
mod backend;

/// An address the app can listen on and the overlay connect to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint(backend::Address);

impl Endpoint {
    /// The current user's endpoint.
    pub fn for_user() -> Self {
        Self(backend::Address::for_user())
    }

    /// A specific address: a socket path on Unix, a pipe name on Windows.
    pub fn at(address: impl Into<std::path::PathBuf>) -> Self {
        Self(backend::Address::at(address.into()))
    }

    /// For logs.
    pub fn describe(&self) -> String {
        self.0.describe()
    }

    /// Connects to a listening app.
    pub async fn connect(&self) -> std::io::Result<(Receiver, Sender)> {
        let (read, write) = self.0.connect().await?;
        Ok(split(read, write))
    }

    /// Starts listening. Fails with `AddrInUse` while another instance is
    /// listening; takes over an address a dead instance left behind.
    pub async fn bind(&self) -> std::io::Result<Listener> {
        Ok(Listener(self.0.bind().await?))
    }
}

/// Accepts overlay connections.
#[derive(Debug)]
pub struct Listener(backend::Listener);

impl Listener {
    /// Waits for the next connection.
    pub async fn accept(&mut self) -> std::io::Result<(Receiver, Sender)> {
        let (read, write) = self.0.accept().await?;
        Ok(split(read, write))
    }
}
