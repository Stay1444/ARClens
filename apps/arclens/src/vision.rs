//! Screen analysis: frames in, "the player is hovering item X here" out.
//!
//! Runs on its own thread so detection and OCR never block the UI. Frames
//! come from a [`FrameSource`]: the XDG `ScreenCast` portal (the desktop asks
//! once which monitor to share), or a replay of recorded frames
//! (`ARCLENS_REPLAY_DIR`) for development.

use crate::data;
use crate::paths::Paths;
use arclens_vision::{Analyzer, Hover, NameReader, RECOGNITION_MODEL_URL};
use futures::channel::mpsc;
use iced::Subscription;
use image::RgbImage;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Directory of captured frames (PNG/JPEG) to replay instead of capturing.
pub const REPLAY_DIR_ENV: &str = "ARCLENS_REPLAY_DIR";
/// `1` to start with item detection enabled.
pub const ENABLE_ENV: &str = "ARCLENS_VISION";
/// Use this OCR model file instead of the cached download.
pub const MODEL_ENV: &str = "ARCLENS_OCR_MODEL";

#[derive(Debug, Clone)]
pub enum Event {
    /// Capture started on this monitor (logical coordinates).
    Monitor(arclens_ipc::MonitorRect),
    Hover(Hover),
    /// No tooltip on screen any more.
    Gone,
    /// Vision isn't running; why.
    Unavailable(String),
}

/// Something that yields frames at its own pace (blocking).
trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<RgbImage>;

    /// The monitor being captured, if known.
    fn monitor(&self) -> Option<arclens_ipc::MonitorRect> {
        None
    }
}

pub fn subscription() -> Subscription<Event> {
    Subscription::run(|| {
        iced::stream::channel(16, async |output: mpsc::Sender<Event>| {
            std::thread::Builder::new()
                .name("arclens-vision".into())
                .spawn(move || run(output))
                .ok();
            futures::future::pending::<()>().await;
        })
    })
}

fn run(mut output: mpsc::Sender<Event>) {
    let mut send = |event: Event| output.try_send(event).is_ok();

    let paths = match Paths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            send(Event::Unavailable(error.to_string()));
            return;
        }
    };
    let analyzer = ensure_model(&paths)
        .map_err(|e| format!("{e:#}"))
        .and_then(|model| NameReader::from_model_file(&model).map_err(|e| format!("{e:#}")));
    let mut analyzer = match analyzer {
        Ok(reader) => Analyzer::new(reader),
        Err(error) => {
            send(Event::Unavailable(format!(
                "OCR model unavailable: {error}"
            )));
            return;
        }
    };

    let mut source = match frame_source(&paths) {
        Ok(source) => source,
        Err(error) => {
            send(Event::Unavailable(format!(
                "screen capture unavailable: {error:#}"
            )));
            return;
        }
    };

    if let Some(monitor) = source.monitor() {
        send(Event::Monitor(monitor));
    }

    let mut last: Option<Hover> = None;
    while let Some(frame) = source.next_frame() {
        let hover = match analyzer.analyze(&frame) {
            Ok(hover) => hover,
            Err(error) => {
                tracing::warn!(error = format!("{error:#}"), "frame analysis failed");
                None
            }
        };
        // Only report changes.
        let event = match (&last, &hover) {
            (Some(a), Some(b)) if a == b => continue,
            (None, None) => continue,
            (_, Some(h)) => Event::Hover(h.clone()),
            (Some(_), None) => Event::Gone,
        };
        last = hover;
        if !send(event) {
            return; // UI gone or hopelessly behind.
        }
    }
}

fn frame_source(paths: &Paths) -> anyhow::Result<Box<dyn FrameSource>> {
    if let Some(dir) = std::env::var_os(REPLAY_DIR_ENV) {
        return Ok(Box::new(Replay::open(Path::new(&dir))?));
    }
    let token_path = paths.capture_token();
    let token = std::fs::read_to_string(&token_path)
        .ok()
        .map(|t| t.trim().to_owned());
    let capture = arclens_capture::Capture::start(token)?;
    // Persist the new token so the monitor picker isn't shown next time.
    if let Some(token) = &capture.restore_token {
        if let Some(dir) = token_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&token_path, token)?;
    }
    Ok(Box::new(Portal(capture)))
}

/// Live frames from the screen-cast portal.
struct Portal(arclens_capture::Capture);

impl FrameSource for Portal {
    fn next_frame(&mut self) -> Option<RgbImage> {
        self.0.next_frame()
    }

    fn monitor(&self) -> Option<arclens_ipc::MonitorRect> {
        self.0
            .monitor
            .map(|(x, y, width, height)| arclens_ipc::MonitorRect {
                x,
                y,
                width,
                height,
            })
    }
}

/// The OCR model path, downloading it into the cache on first use.
fn ensure_model(paths: &Paths) -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os(MODEL_ENV) {
        return Ok(path.into());
    }
    let path = paths.cache.join("models").join("text-recognition.rten");
    if path.is_file() {
        return Ok(path);
    }
    tracing::info!(
        url = RECOGNITION_MODEL_URL,
        "downloading OCR model (~10 MB)"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let bytes = runtime.block_on(async {
        data::http_client()
            .get(RECOGNITION_MODEL_URL)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await
    })?;
    std::fs::create_dir_all(path.parent().unwrap_or(&paths.cache))?;
    let tmp = path.with_extension("rten.part");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Replays image files from a directory, one per interval, looping.
struct Replay {
    files: Vec<PathBuf>,
    next: usize,
    interval: Duration,
}

impl Replay {
    fn open(dir: &Path) -> std::io::Result<Self> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| matches!(e, "png" | "jpg" | "jpeg"))
            })
            .collect();
        files.sort();
        if files.is_empty() {
            return Err(std::io::Error::other(format!(
                "no images in {}",
                dir.display()
            )));
        }
        let interval = std::env::var("ARCLENS_REPLAY_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map_or(Duration::from_millis(1500), Duration::from_millis);
        Ok(Self {
            files,
            next: 0,
            interval,
        })
    }
}

impl FrameSource for Replay {
    fn next_frame(&mut self) -> Option<RgbImage> {
        std::thread::sleep(self.interval);
        let path = &self.files[self.next % self.files.len()];
        self.next += 1;
        tracing::debug!(frame = %path.display(), "replay");
        image::open(path).ok().map(image::DynamicImage::into_rgb8)
    }
}
