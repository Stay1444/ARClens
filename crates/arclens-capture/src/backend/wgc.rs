//! Windows: Windows Graphics Capture, through the `windows-capture` crate.
//!
//! Captures the primary monitor (where a fullscreen game runs unless moved),
//! without the cursor and, where Windows allows it, without the yellow
//! capture border.

use crate::convert::{Layout, to_rgb};
use crate::slot::Slot;
use crate::{MAX_FPS, Started};
use anyhow::anyhow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Receives frames on the capture thread.
struct Handler {
    slot: Arc<Slot>,
    last_convert: Option<Instant>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Arc<Slot>;
    type Error = Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            slot: ctx.flags,
            last_convert: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame<'_>,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // The pointer, in frame pixels: the primary monitor sits at the
        // desktop origin, in physical pixels like the frames.
        if let mouse_position::mouse_position::Mouse::Position { x, y } =
            mouse_position::mouse_position::Mouse::get_mouse_position()
        {
            #[allow(clippy::cast_precision_loss, reason = "screen pixels")]
            self.slot.set_cursor((x as f32, y as f32));
        }
        // Skip work nobody asked for.
        if !self.slot.wants_frame(self.last_convert) {
            return Ok(());
        }
        let mut buffer = frame.buffer()?;
        let (width, height) = (buffer.width(), buffer.height());
        let stride = buffer.row_pitch() as usize;
        let bytes = buffer.as_raw_buffer();
        if let Some(rgb) = to_rgb(bytes, width, height, stride, Layout::Bgr) {
            self.last_convert = Some(Instant::now());
            self.slot.offer(rgb);
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.slot.close();
        Ok(())
    }
}

/// The running capture; dropping it stops it.
pub struct Session(Option<CaptureControl<Handler, Error>>);

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(control) = self.0.take()
            && let Err(error) = control.stop()
        {
            tracing::debug!(error = %error, "stopping screen capture");
        }
        tracing::info!("screen capture stopped");
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "same signature as other backends"
)]
pub fn start(_restore_token: Option<String>, slot: &Arc<Slot>) -> anyhow::Result<Started<Session>> {
    let monitor = Monitor::primary().map_err(|e| anyhow!("no primary monitor: {e}"))?;
    let size = monitor
        .width()
        .ok()
        .zip(monitor.height().ok())
        .map(|(w, h)| {
            (
                0,
                0,
                i32::try_from(w).unwrap_or(0),
                i32::try_from(h).unwrap_or(0),
            )
        });
    let settings = Settings::new(
        monitor,
        // The tooltip text must not be hidden under the pointer.
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(Duration::from_secs(1) / MAX_FPS),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        Arc::clone(slot),
    );
    let control = Handler::start_free_threaded(settings)
        .map_err(|e| anyhow!("Windows Graphics Capture: {e}"))?;
    tracing::info!(?size, "capturing the primary monitor");
    Ok(Started {
        session: Session(Some(control)),
        restore_token: None,
        // The primary monitor's top-left is the desktop origin. Its size is
        // in physical pixels; the overlay places itself on the primary
        // monitor anyway.
        monitor: size,
    })
}
