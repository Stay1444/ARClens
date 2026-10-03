//! Named pipe transport (Windows).

use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

/// `ERROR_PIPE_BUSY`: every instance is taken; wait and retry.
const PIPE_BUSY: i32 = 231;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address(String);

impl Address {
    /// `\\.\pipe\arclens-<user>`: one per user, like the Unix socket in
    /// the user's runtime directory.
    pub fn for_user() -> Self {
        let user: String = std::env::var("USERNAME")
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        Self(format!(r"\\.\pipe\arclens-{user}"))
    }

    pub fn at(name: PathBuf) -> Self {
        Self(name.into_os_string().to_string_lossy().into_owned())
    }

    pub fn describe(&self) -> String {
        self.0.clone()
    }

    pub async fn connect(
        &self,
    ) -> std::io::Result<(ReadHalf<NamedPipeClient>, WriteHalf<NamedPipeClient>)> {
        loop {
            match ClientOptions::new().open(&self.0) {
                Ok(client) => return Ok(tokio::io::split(client)),
                Err(e) if e.raw_os_error() == Some(PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    #[allow(clippy::unused_async, reason = "same signature as the Unix backend")]
    pub async fn bind(&self) -> std::io::Result<Listener> {
        // `first_pipe_instance` fails while another process owns the name,
        // i.e. while another instance runs. Pipes vanish with their owner,
        // so nothing is left behind to clean up.
        let next = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&self.0)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "another ARClens instance is running",
                    )
                } else {
                    e
                }
            })?;
        Ok(Listener {
            name: self.0.clone(),
            next,
        })
    }
}

#[derive(Debug)]
pub struct Listener {
    name: String,
    /// The instance the next client connects to.
    next: NamedPipeServer,
}

impl Listener {
    pub async fn accept(
        &mut self,
    ) -> std::io::Result<(ReadHalf<NamedPipeServer>, WriteHalf<NamedPipeServer>)> {
        self.next.connect().await?;
        // A fresh instance for the next client before handing this one out.
        let fresh = ServerOptions::new().create(&self.name)?;
        let connected = std::mem::replace(&mut self.next, fresh);
        Ok(tokio::io::split(connected))
    }
}
