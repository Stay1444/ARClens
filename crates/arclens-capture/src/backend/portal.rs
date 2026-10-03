//! Linux: the XDG `ScreenCast` portal and PipeWire.
//!
//! This is the same path OBS and browser screen sharing use: the user picks
//! a monitor in the desktop's own dialog (once — the returned restore token
//! skips it next time), and frames arrive over PipeWire. The game process is
//! never touched. Surplus frames are returned to PipeWire unconverted.

use crate::convert::{Layout, to_rgb};
use crate::slot::Slot;
use crate::{MAX_FPS, Started};
use anyhow::{Context as _, anyhow};
use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{
    CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    StartCastOptions,
};
use pipewire as pw;
use pw::spa;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Instant;

/// The portal session and PipeWire stream; dropping it stops both.
pub struct Session {
    /// Ends the portal session (the desktop's "screen is shared" indicator).
    stop_portal: Option<tokio::sync::oneshot::Sender<()>>,
    /// Quits the PipeWire loop.
    stop_pipewire: Option<pw::channel::Sender<()>>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(stop) = self.stop_pipewire.take() {
            let _ = stop.send(());
        }
        if let Some(stop) = self.stop_portal.take() {
            let _ = stop.send(());
        }
    }
}

/// Asks the portal for a monitor (showing the desktop's picker unless
/// `restore_token` is still valid) and streams it into `slot`.
pub fn start(restore_token: Option<String>, slot: &Arc<Slot>) -> anyhow::Result<Started<Session>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (stop_portal, stop_portal_rx) = tokio::sync::oneshot::channel::<()>();
    // The portal session lives on its own thread/runtime until the session
    // is dropped; closing it ends the cast.
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
    let pw_slot = Arc::clone(slot);
    let (fd, node) = (remote.fd, remote.node_id);
    let (stop_pipewire, stop_pipewire_rx) = pw::channel::channel::<()>();
    std::thread::Builder::new()
        .name("arclens-pipewire".into())
        .spawn(move || {
            if let Err(error) = run_pipewire(fd, node, &pw_slot, stop_pipewire_rx) {
                tracing::error!(%error, "PipeWire capture stopped");
            }
            pw_slot.close();
        })?;

    Ok(Started {
        session: Session {
            stop_portal: Some(stop_portal),
            stop_pipewire: Some(stop_pipewire),
        },
        restore_token: remote.restore_token,
        monitor: remote.monitor,
    })
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
    // The pointer's position as metadata, not drawn into the frames (the
    // tooltip text must not be hidden under it): for marker tooltips.
    let cursor_mode = match proxy.available_cursor_modes().await {
        Ok(modes) if modes.contains(CursorMode::Metadata) => CursorMode::Metadata,
        _ => CursorMode::Hidden,
    };
    let session = proxy
        .create_session(ashpd::desktop::CreateSessionOptions::default())
        .await?;
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(cursor_mode)
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
    slot: &Arc<Slot>,
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

    let slot = Arc::clone(slot);
    let _listener = stream
        .add_local_listener_with_user_data(StreamState::default())
        .param_changed(|stream, state, id, param| {
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
                // Ask for the cursor metadata alongside the frames.
                if let Some(meta) = cursor_meta_pod()
                    && let Some(pod) = spa::pod::Pod::from_bytes(&meta)
                    && let Err(error) = stream.update_params(&mut [pod])
                {
                    tracing::debug!(%error, "cursor metadata not available");
                }
            }
        })
        .process(move |stream, state| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            if let Some(cursor) = buffer.find_meta::<spa::buffer::meta::MetaCursor>()
                && cursor.id() != 0
            {
                let at = cursor.position();
                #[allow(clippy::cast_precision_loss, reason = "screen pixels")]
                slot.set_cursor((at.x as f32, at.y as f32));
            }
            // Skip work nobody asked for.
            if !slot.wants_frame(state.last_convert) {
                return;
            }
            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };
            let stride = usize::try_from(data.chunk().stride()).unwrap_or(0);
            let size = state.format.size();
            let layout = match state.format.format() {
                spa::param::video::VideoFormat::BGRx | spa::param::video::VideoFormat::BGRA => {
                    Layout::Bgr
                }
                spa::param::video::VideoFormat::RGBx | spa::param::video::VideoFormat::RGBA => {
                    Layout::Rgb
                }
                _ => return,
            };
            let Some(bytes) = data.data() else { return };
            let Some(frame) = to_rgb(bytes, size.width, size.height, stride, layout) else {
                return;
            };
            state.last_convert = Some(Instant::now());
            slot.offer(frame);
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

/// The cursor metadata request: position plus room for a 64×64 bitmap
/// (which we don't use, but compositors size the meta for it).
fn cursor_meta_pod() -> Option<Vec<u8>> {
    use spa::pod::{Object, Property, Value};
    let size = std::mem::size_of::<spa::sys::spa_meta_cursor>()
        + std::mem::size_of::<spa::sys::spa_meta_bitmap>()
        + 64 * 64 * 4;
    let object = Object {
        type_: spa::utils::SpaTypes::ObjectParamMeta.as_raw(),
        id: spa::param::ParamType::Meta.as_raw(),
        properties: vec![
            Property::new(
                spa::sys::SPA_PARAM_META_type,
                Value::Id(spa::utils::Id(spa::sys::SPA_META_Cursor)),
            ),
            Property::new(
                spa::sys::SPA_PARAM_META_size,
                Value::Int(i32::try_from(size).ok()?),
            ),
        ],
    };
    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(object),
    )
    .ok()
    .map(|(cursor, _)| cursor.into_inner())
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
