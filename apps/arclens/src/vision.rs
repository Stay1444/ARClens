//! Screen analysis: frames in, "the player is hovering item X here" out.
//!
//! Runs on its own thread so detection and OCR never block the UI. Frames
//! come from a [`FrameSource`]: today a replay of recorded frames
//! (`ARCLENS_REPLAY_DIR`), next the XDG `ScreenCast` portal.

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
/// Use this OCR model file instead of the cached download.
pub const MODEL_ENV: &str = "ARCLENS_OCR_MODEL";

#[derive(Debug, Clone)]
pub enum Event {
    Hover(Hover),
    /// No tooltip on screen any more.
    Gone,
    /// Vision isn't running; why.
    Unavailable(String),
}

/// Something that yields frames at its own pace (blocking).
trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<RgbImage>;
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

    let Some(mut source) = frame_source() else {
        send(Event::Unavailable(
            "screen capture is not implemented yet (set ARCLENS_REPLAY_DIR to replay frames)"
                .into(),
        ));
        return;
    };
    let analyzer = Paths::discover()
        .map_err(|e| e.to_string())
        .and_then(|paths| ensure_model(&paths).map_err(|e| format!("{e:#}")))
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

fn frame_source() -> Option<Box<dyn FrameSource>> {
    let dir = std::env::var_os(REPLAY_DIR_ENV)?;
    match Replay::open(Path::new(&dir)) {
        Ok(replay) => Some(Box::new(replay)),
        Err(error) => {
            tracing::warn!(%error, "cannot open replay directory");
            None
        }
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
