//! Screen analysis: frames in, "the player is hovering item X here" out.
//!
//! Runs on its own thread so detection and OCR never block the UI. Frames
//! come from a [`FrameSource`]: the XDG `ScreenCast` portal (the desktop asks
//! once which monitor to share), or a replay of recorded frames
//! (`ARCLENS_REPLAY_DIR`) for development.

use crate::data;
use crate::paths::Paths;
use arclens_vision::{
    Analyzer, Hover, MapHeader, NameReader, RECOGNITION_MODEL_URL, is_map_screen,
};
use futures::channel::mpsc;
use iced::Subscription;
use image::RgbImage;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Capture pace right after tooltip activity (10 fps).
const FAST_INTERVAL: Duration = Duration::from_millis(100);
/// How long to stay fast after the last tooltip was seen.
const FAST_FOR: Duration = Duration::from_secs(3);
/// How often the map header is re-read while the map is open (its clock
/// ticks every second; the map name and condition rarely change).
const MAP_REREAD: Duration = Duration::from_secs(5);
/// How long the map screen must be gone before it counts as closed: a pan,
/// a zoom animation or one odd frame must not close it.
const MAP_CLOSE_GRACE: Duration = Duration::from_secs(1);

/// Whether the map screen is open, with hysteresis.
#[derive(Debug, Default)]
struct MapWatch {
    /// When the header was last read; `Some` while the map is open.
    read_at: Option<Instant>,
    /// When the map screen was last not seen while open.
    missing_since: Option<Instant>,
}

impl MapWatch {
    fn is_open(&self) -> bool {
        self.read_at.is_some()
    }
}

/// Minimum time between map-label reads while the view keeps changing.
const LABEL_INTERVAL: Duration = Duration::from_millis(250);

/// Tracks the open map's view so labels are only read when it changed.
#[derive(Debug, Default)]
struct MapView {
    fingerprint: Option<u64>,
    read_at: Option<Instant>,
    /// The view changed since the last read.
    dirty: bool,
}

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
    /// The map screen is open (sent again every few seconds while it is).
    MapOpen(MapHeader),
    /// The map screen closed.
    MapClosed,
    /// Place names read on the open map (boxes in frame pixels), the frame
    /// size, and whether the quest panel covers the map's left. Sent when
    /// the view changed.
    MapLabels(Vec<arclens_data::anchors::ScreenLabel>, (f32, f32), bool),
    /// Vision isn't running; why.
    Unavailable(String),
}

/// Something that yields frames at its own pace (blocking).
trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<RgbImage>;

    /// Hint: how soon the next frame is wanted.
    fn set_interval(&mut self, _interval: Duration) {}

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

/// The worker's link to the UI.
struct Outbox(mpsc::Sender<Event>);

impl Outbox {
    /// `false` when the UI is gone or hopelessly behind.
    fn send(&mut self, event: Event) -> bool {
        self.0.try_send(event).is_ok()
    }

    fn closed(&self) -> bool {
        self.0.is_closed()
    }
}

fn run(output: mpsc::Sender<Event>) {
    let mut out = Outbox(output);

    let paths = match Paths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            out.send(Event::Unavailable(error.to_string()));
            return;
        }
    };
    let analyzer = ensure_model(&paths)
        .map_err(|e| format!("{e:#}"))
        .and_then(|model| NameReader::from_model_file(&model).map_err(|e| format!("{e:#}")));
    let mut analyzer = match analyzer {
        Ok(reader) => Analyzer::new(reader),
        Err(error) => {
            out.send(Event::Unavailable(format!(
                "OCR model unavailable: {error}"
            )));
            return;
        }
    };

    let mut source = match frame_source(&paths) {
        Ok(source) => source,
        Err(error) => {
            out.send(Event::Unavailable(format!(
                "screen capture unavailable: {error:#}"
            )));
            return;
        }
    };

    if let Some(monitor) = source.monitor() {
        out.send(Event::Monitor(monitor));
    }

    let mut last: Option<Hover> = None;
    let mut fast_until = Instant::now();
    // When the map header was last read, while the map is open.
    let mut map_watch = MapWatch::default();
    let mut map_view = MapView::default();
    while let Some(frame) = source.next_frame() {
        // Capture turned off (the UI dropped the subscription): stop, which
        // drops the source and ends the screencast session.
        if out.closed() {
            tracing::info!("item detection stopped");
            return;
        }
        if !watch_map(&analyzer, &frame, &mut map_watch, &mut |e| out.send(e)) {
            return;
        }
        if map_watch.is_open() {
            // Labels only from frames that do show the map.
            if map_watch.missing_since.is_none()
                && !watch_labels(&mut analyzer, &frame, &mut map_view, &mut |e| out.send(e))
            {
                return;
            }
            // Keep sampling quickly while the map is open: panning.
            source.set_interval(FAST_INTERVAL);
            continue;
        }
        map_view = MapView::default();
        let hover = match analyzer.analyze(&frame) {
            Ok(hover) => hover,
            Err(error) => {
                tracing::warn!(error = format!("{error:#}"), "frame analysis failed");
                None
            }
        };
        // Sample faster for a moment after something changed on screen, so
        // the next hover is picked up sooner; idle otherwise.
        if hover.is_some() || last.is_some() {
            fast_until = Instant::now() + FAST_FOR;
        }
        source.set_interval(if Instant::now() < fast_until {
            FAST_INTERVAL
        } else {
            arclens_capture::IDLE_INTERVAL
        });

        // Only report changes.
        let event = match (&last, &hover) {
            (Some(a), Some(b)) if a.same_as(b) => continue,
            (None, None) => continue,
            (_, Some(h)) => Event::Hover(h.clone()),
            (Some(_), None) => Event::Gone,
        };
        last = hover;
        if !out.send(event) {
            return; // UI gone or hopelessly behind.
        }
    }
}

/// Reports the map screen opening, staying open (header re-read every
/// [`MAP_REREAD`]) and closing. `false` when the UI is gone.
fn watch_map(
    analyzer: &Analyzer,
    frame: &RgbImage,
    watch: &mut MapWatch,
    send: &mut impl FnMut(Event) -> bool,
) -> bool {
    if !is_map_screen(frame) {
        if !watch.is_open() {
            return true;
        }
        let since = *watch.missing_since.get_or_insert_with(Instant::now);
        if since.elapsed() < MAP_CLOSE_GRACE {
            return true;
        }
        *watch = MapWatch::default();
        return send(Event::MapClosed);
    }
    watch.missing_since = None;
    if watch.read_at.is_some_and(|at| at.elapsed() < MAP_REREAD) {
        return true;
    }
    let opening = !watch.is_open();
    watch.read_at = Some(Instant::now());
    let header = match analyzer.read_map_header(frame) {
        Ok(header) => header,
        Err(error) => {
            tracing::warn!(error = format!("{error:#}"), "map header unreadable");
            None
        }
    };
    match header {
        Some(header) => send(Event::MapOpen(header)),
        // Open anyway: the app keeps the map it last recognised.
        None if opening => send(Event::MapOpen(arclens_vision::MapHeader {
            title: String::new(),
            condition: None,
        })),
        None => true,
    }
}

/// Reads the map's labels when the view changed. `false` when the UI is
/// gone.
fn watch_labels(
    analyzer: &mut Analyzer,
    frame: &RgbImage,
    view: &mut MapView,
    send: &mut impl FnMut(Event) -> bool,
) -> bool {
    let fingerprint = view_fingerprint(frame);
    if view.fingerprint != Some(fingerprint) {
        view.fingerprint = Some(fingerprint);
        view.dirty = true;
    }
    if !view.dirty || view.read_at.is_some_and(|at| at.elapsed() < LABEL_INTERVAL) {
        return true;
    }
    view.dirty = false;
    view.read_at = Some(Instant::now());
    let labels = match analyzer.read_map_labels(frame) {
        Ok(labels) => labels,
        Err(error) => {
            tracing::warn!(error = format!("{error:#}"), "map labels unreadable");
            return true;
        }
    };
    #[allow(clippy::cast_precision_loss, reason = "pixel coordinates")]
    let size = (frame.width() as f32, frame.height() as f32);
    #[allow(clippy::cast_precision_loss, reason = "pixel coordinates")]
    let labels = labels
        .into_iter()
        .map(|l| arclens_data::anchors::ScreenLabel {
            text: l.text,
            rect: (
                l.rect.x as f32,
                l.rect.y as f32,
                l.rect.width as f32,
                l.rect.height as f32,
            ),
        })
        .collect();
    send(Event::MapLabels(
        labels,
        size,
        arclens_vision::quest_panel_open(frame),
    ))
}

/// Coarse fingerprint of the map viewport: changes when the view pans or
/// zooms, not with compression noise.
fn view_fingerprint(frame: &RgbImage) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let (width, height) = (frame.width(), frame.height());
    for gy in 1..24 {
        for gx in 1..32 {
            // Viewport only (left of the legend, below the tab bar).
            let col = width * 76 / 100 * gx / 32 + width * 2 / 100;
            let row = height * 82 / 100 * gy / 24 + height * 8 / 100;
            let [red, green, blue] = frame.get_pixel(col.min(width - 1), row.min(height - 1)).0;
            ((u16::from(red) + u16::from(green) + u16::from(blue)) / 48).hash(&mut hasher);
        }
    }
    hasher.finish()
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

    fn set_interval(&mut self, interval: Duration) {
        self.0.set_interval(interval);
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
