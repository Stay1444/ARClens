//! Screen capture through the XDG `ScreenCast` portal and PipeWire.
//!
//! This is the same path OBS and browser screen sharing use: the user picks
//! a monitor in the desktop's own dialog (once — the returned restore token
//! skips it next time), and frames arrive over PipeWire. The game process is
//! never touched.
//!
//! Cost control: we ask the compositor for at most [`MAX_FPS`] frames per
//! second, and only convert a frame to RGB when the consumer asks for one —
//! surplus frames are returned to PipeWire untouched.

use anyhow::{Context as _, anyhow};
use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{
    CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    StartCastOptions,
};
use image::RgbImage;
use pipewire as pw;
use pw::spa;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Upper bound on frames requested from the compositor.
pub const MAX_FPS: u32 = 10;
/// Default pace while nothing interesting is on screen.
pub const IDLE_INTERVAL: Duration = Duration::from_millis(250);

/// A running capture of one monitor. Dropping it stops the capture.
pub struct Capture {
    shared: Arc<Shared>,
    last_seq: u64,
    /// Token to pass to [`Capture::start`] next time to skip the dialog.
    pub restore_token: Option<String>,
    /// Where the captured monitor sits in the compositor's logical space,
    /// as `(x, y, width, height)`, if the portal reports it.
    pub monitor: Option<(i32, i32, i32, i32)>,
    /// Ends the portal session (the desktop's "screen is shared" indicator).
    stop_portal: Option<tokio::sync::oneshot::Sender<()>>,
    /// Quits the PipeWire loop.
    stop_pipewire: Option<pw::channel::Sender<()>>,
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
    /// Stops streaming and closes the portal session, so turning capture
    /// off leaves nothing running.
    fn drop(&mut self) {
        if let Some(stop) = self.stop_pipewire.take() {
            let _ = stop.send(());
        }
        if let Some(stop) = self.stop_portal.take() {
            let _ = stop.send(());
        }
        // Wake a consumer blocked in `next_frame`.
        if let Ok(mut state) = self.shared.state.lock() {
            state.closed = true;
            self.shared.ready.notify_all();
        }
    }
}

#[derive(Debug, Default)]
struct Shared {
    state: Mutex<SharedState>,
    ready: Condvar,
    /// Minimum time between converted frames, in ms (0 = default).
    interval_ms: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Default)]
struct SharedState {
    /// The consumer wants a frame converted.
    wanted: bool,
    frame: Option<RgbImage>,
    seq: u64,
    /// The PipeWire side died; no more frames will come.
    closed: bool,
}

impl Capture {
    /// Asks the portal for a monitor (showing the desktop's picker unless
    /// `restore_token` is still valid) and starts streaming.
    pub fn start(restore_token: Option<String>) -> anyhow::Result<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        let (stop_portal, stop_portal_rx) = tokio::sync::oneshot::channel::<()>();
        // The portal session lives on its own thread/runtime until the
        // `Capture` is dropped; closing it ends the cast.
        std::thread::Builder::new()
            .name("arclens-portal".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx.send(Err(anyhow!(e)));
                        return;
                    }
                };
                runtime.block_on(async move {
                    match open_portal(restore_token.as_deref()).await {
                        Ok(((_proxy, session), remote)) => {
                            let _ = tx.send(Ok(remote));
                            // Until the Capture is dropped (or the sender
                            // vanishes with it).
                            let _ = stop_portal_rx.await;
                            if let Err(error) = session.close().await {
                                tracing::debug!(%error, "closing screencast session");
                            }
                            tracing::info!("screen capture stopped");
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                        }
                    }
                });
            })?;

        let remote = rx
            .recv()
            .context("portal thread exited")?
            .context("screen capture portal")?;
        let shared = Arc::new(Shared::default());
        let pw_shared = Arc::clone(&shared);
        let (fd, node) = (remote.fd, remote.node_id);
        let (stop_pipewire, stop_pipewire_rx) = pw::channel::channel::<()>();
        std::thread::Builder::new()
            .name("arclens-pipewire".into())
            .spawn(move || {
                if let Err(error) = run_pipewire(fd, node, &pw_shared, stop_pipewire_rx) {
                    tracing::error!(%error, "PipeWire capture stopped");
                }
                let mut state = pw_shared
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.closed = true;
                pw_shared.ready.notify_all();
            })?;

        Ok(Self {
            shared,
            last_seq: 0,
            restore_token: remote.restore_token,
            monitor: remote.monitor,
            stop_portal: Some(stop_portal),
            stop_pipewire: Some(stop_pipewire),
        })
    }

    /// Sets the pace of converted frames: e.g. faster while a tooltip is on
    /// screen, [`IDLE_INTERVAL`] otherwise. Capped by [`MAX_FPS`].
    pub fn set_interval(&self, interval: Duration) {
        let ms = u64::try_from(interval.as_millis()).unwrap_or(u64::MAX);
        self.shared
            .interval_ms
            .store(ms, std::sync::atomic::Ordering::Relaxed);
    }

    /// Blocks until a frame newer than the previous call is available.
    /// `None` once the capture has ended (monitor unplugged, cast revoked).
    pub fn next_frame(&mut self) -> Option<RgbImage> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.wanted = true;
        loop {
            if state.seq > self.last_seq
                && let Some(frame) = state.frame.take()
            {
                self.last_seq = state.seq;
                return Some(frame);
            }
            if state.closed {
                return None;
            }
            state = self
                .shared
                .ready
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

struct Remote {
    fd: OwnedFd,
    node_id: u32,
    restore_token: Option<String>,
    monitor: Option<(i32, i32, i32, i32)>,
}

/// The portal proxy and session (kept alive while capturing), and the
/// PipeWire remote to read from.
type Portal = (Screencast, ashpd::desktop::Session<Screencast>);

async fn open_portal(restore_token: Option<&str>) -> anyhow::Result<(Portal, Remote)> {
    let proxy = Screencast::new().await?;
    let session = proxy
        .create_session(ashpd::desktop::CreateSessionOptions::default())
        .await?;
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                // The tooltip text must not be hidden under the pointer.
                .set_cursor_mode(CursorMode::Hidden)
                .set_sources(ashpd::enumflags2::BitFlags::from(SourceType::Monitor))
                .set_multiple(false)
                .set_persist_mode(PersistMode::ExplicitlyRevoked)
                .set_restore_token(restore_token),
        )
        .await?
        .response()?;
    let streams = proxy
        .start(&session, None, StartCastOptions::default())
        .await?
        .response()?;
    let stream = streams
        .streams()
        .first()
        .ok_or_else(|| anyhow!("portal returned no streams"))?;
    let node_id = stream.pipe_wire_node_id();
    let monitor = stream
        .position()
        .zip(stream.size())
        .map(|((x, y), (w, h))| (x, y, w, h));
    tracing::info!(?monitor, "capturing monitor");
    let restore_token = streams.restore_token().map(str::to_owned);
    let fd = proxy
        .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
        .await?;
    Ok((
        (proxy, session),
        Remote {
            fd,
            node_id,
            restore_token,
            monitor,
        },
    ))
}

#[derive(Default)]
struct StreamState {
    format: spa::param::video::VideoInfoRaw,
    last_convert: Option<Instant>,
}

fn run_pipewire(
    fd: OwnedFd,
    node_id: u32,
    shared: &Arc<Shared>,
    stop: pw::channel::Receiver<()>,
) -> anyhow::Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let _stop = stop.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |()| mainloop.quit()
    });
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;
    let stream = pw::stream::StreamBox::new(
        &core,
        "arclens-capture",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let shared = Arc::clone(shared);
    let _listener = stream
        .add_local_listener_with_user_data(StreamState::default())
        .param_changed(|_, state, id, param| {
            let Some(param) = param else { return };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            if state.format.parse(param).is_ok() {
                let size = state.format.size();
                tracing::info!(
                    width = size.width,
                    height = size.height,
                    format = ?state.format.format(),
                    "capture format"
                );
            }
        })
        .process(move |stream, state| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            // Throttle and skip work nobody asked for.
            let interval = match shared
                .interval_ms
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                0 => IDLE_INTERVAL,
                ms => Duration::from_millis(ms),
            };
            let due = state.last_convert.is_none_or(|t| t.elapsed() >= interval);
            let wanted = shared.state.lock().is_ok_and(|s| s.wanted);
            if !due || !wanted {
                return;
            }
            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };
            let stride = data.chunk().stride();
            let size = state.format.size();
            let format = state.format.format();
            let Some(bytes) = data.data() else { return };
            let Some(frame) = to_rgb(bytes, size.width, size.height, stride, format) else {
                return;
            };
            state.last_convert = Some(Instant::now());
            if let Ok(mut s) = shared.state.lock() {
                s.frame = Some(frame);
                s.seq += 1;
                s.wanted = false;
                shared.ready.notify_all();
            }
        })
        .register()?;

    let format = format_pod()?;
    let mut params = [spa::pod::Pod::from_bytes(&format).ok_or_else(|| anyhow!("bad format pod"))?];
    stream.connect(
        spa::utils::Direction::Input,
        Some(node_id),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;
    mainloop.run();
    Ok(())
}

/// Raw video in a 4-byte RGB layout, any size, at most [`MAX_FPS`].
fn format_pod() -> anyhow::Result<Vec<u8>> {
    use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    use spa::param::video::VideoFormat;
    let object = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
        spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::BGRA,
            VideoFormat::RGBx,
            VideoFormat::RGBA
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle {
                width: 1920,
                height: 1080
            },
            spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            spa::utils::Rectangle {
                width: 8192,
                height: 8192
            }
        ),
        spa::pod::property!(
            FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            spa::utils::Fraction {
                num: MAX_FPS,
                denom: 1
            },
            spa::utils::Fraction { num: 0, denom: 1 },
            spa::utils::Fraction {
                num: MAX_FPS,
                denom: 1
            }
        ),
    );
    Ok(spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .map_err(|e| anyhow!("serialising format: {e:?}"))?
    .0
    .into_inner())
}

/// Converts one 4-byte-per-pixel frame to RGB. `None` for unsupported
/// formats or inconsistent sizes.
fn to_rgb(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: i32,
    format: spa::param::video::VideoFormat,
) -> Option<RgbImage> {
    use spa::param::video::VideoFormat;
    let bgr = match format {
        VideoFormat::BGRx | VideoFormat::BGRA => true,
        VideoFormat::RGBx | VideoFormat::RGBA => false,
        _ => return None,
    };
    let stride = usize::try_from(stride)
        .ok()
        .filter(|&s| s >= width as usize * 4)?;
    if bytes.len() < stride * height as usize {
        return None;
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for row in bytes.chunks_exact(stride).take(height as usize) {
        for px in row[..width as usize * 4].as_chunks::<4>().0 {
            if bgr {
                rgb.extend_from_slice(&[px[2], px[1], px[0]]);
            } else {
                rgb.extend_from_slice(&px[..3]);
            }
        }
    }
    RgbImage::from_raw(width, height, rgb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spa::param::video::VideoFormat;

    #[test]
    fn converts_bgrx_with_row_padding() {
        // 2×1 image, stride 12 (4 bytes padding).
        let bytes = [3, 2, 1, 0, 6, 5, 4, 0, 9, 9, 9, 9];
        let img = to_rgb(&bytes, 2, 1, 12, VideoFormat::BGRx).unwrap();
        assert_eq!(img.into_raw(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rejects_short_buffers_and_unknown_formats() {
        assert!(to_rgb(&[0; 4], 2, 1, 8, VideoFormat::BGRx).is_none());
        assert!(to_rgb(&[0; 8], 2, 1, 8, VideoFormat::I420).is_none());
    }
}
