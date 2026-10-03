//! Screen capture: frames of the monitor the game is on, at a pace the
//! consumer sets. Only the screen is read, as screen recorders do; the game
//! process is never touched.
//!
//! One API, one backend per platform:
//! - Linux: the XDG `ScreenCast` portal and PipeWire (`backend/portal.rs`);
//! - Windows: Windows Graphics Capture (`backend/wgc.rs`).
//!
//! Cost control: frames are converted to RGB only when the consumer asks
//! for one and the pace allows ([`Capture::set_interval`]).

mod convert;
mod slot;

#[cfg(target_os = "linux")]
#[path = "backend/portal.rs"]
mod backend;
#[cfg(windows)]
#[path = "backend/wgc.rs"]
mod backend;

use image::RgbImage;
use slot::Slot;
use std::sync::Arc;
use std::time::Duration;

/// Upper bound on frames taken from the system.
pub const MAX_FPS: u32 = 30;
/// Default pace while nothing interesting is on screen.
pub const IDLE_INTERVAL: Duration = Duration::from_millis(250);

/// What a backend hands back once streaming.
struct Started<S> {
    /// Stops the capture when dropped.
    session: S,
    restore_token: Option<String>,
    monitor: Option<(i32, i32, i32, i32)>,
}

/// A running capture of one monitor. Dropping it stops the capture.
pub struct Capture {
    slot: Arc<Slot>,
    last_seq: u64,
    /// Declared before nothing that outlives it: dropped first, stopping
    /// the backend.
    session: Option<backend::Session>,
    /// Token to pass to [`Capture::start`] next time to skip the system's
    /// picker (Linux portal; `None` elsewhere).
    pub restore_token: Option<String>,
    /// Where the captured monitor sits in the desktop's logical space, as
    /// `(x, y, width, height)`, if the system reports it.
    pub monitor: Option<(i32, i32, i32, i32)>,
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("last_seq", &self.last_seq)
            .field("monitor", &self.monitor)
            .finish_non_exhaustive()
    }
}

impl Drop for Capture {
    /// Stops streaming, so turning capture off leaves nothing running, and
    /// wakes a consumer blocked in [`Capture::next_frame`].
    fn drop(&mut self) {
        drop(self.session.take());
        self.slot.close();
    }
}

impl Capture {
    /// Starts capturing the monitor to watch (on Linux the desktop asks
    /// which, unless `restore_token` is still valid).
    pub fn start(restore_token: Option<String>) -> anyhow::Result<Self> {
        let slot = Arc::new(Slot::default());
        let started = backend::start(restore_token, &slot)?;
        Ok(Self {
            slot,
            last_seq: 0,
            session: Some(started.session),
            restore_token: started.restore_token,
            monitor: started.monitor,
        })
    }

    /// Sets the pace of converted frames: e.g. faster while a tooltip is on
    /// screen, [`IDLE_INTERVAL`] otherwise. Capped by [`MAX_FPS`].
    pub fn set_interval(&self, interval: Duration) {
        self.slot.set_interval(interval);
    }

    /// Blocks until a frame newer than the previous call is available.
    /// `None` once the capture has ended (monitor unplugged, cast revoked).
    pub fn next_frame(&mut self) -> Option<RgbImage> {
        self.slot.next_frame(&mut self.last_seq)
    }
}
