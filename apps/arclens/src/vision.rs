//! Screen analysis: frames in, "the player is hovering item X here" out.
//!
//! Runs on its own thread so detection and OCR never block the UI. Frames
//! come from a [`FrameSource`]: live screen capture (`arclens-capture`; on
//! Linux the desktop asks once which monitor to share), or a replay of
//! recorded frames (`ARCLENS_REPLAY_DIR`) for development.

use crate::data;
use crate::paths::Paths;
use arclens_vision::{
    Analyzer, Hover, MapHeader, Motion, MotionTracker, NameReader, RECOGNITION_MODEL_URL,
    is_map_screen,
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

/// How often the workshop station header is looked for.
const STATION_INTERVAL: Duration = Duration::from_secs(2);
/// Minimum time between map-label reads while the view keeps changing.
const LABEL_INTERVAL: Duration = Duration::from_millis(250);
/// Capture pace while the map is open: the tracker follows pans and zooms
/// from frame to frame (20 fps).
const MAP_INTERVAL: Duration = Duration::from_millis(50);
/// Tracker confidence below which the motion counts as unknown.
const MIN_CONFIDENCE: f32 = 0.06;
/// Motion updates smaller than this (pixels anywhere on screen) aren't sent.
const MOTION_EPSILON: f32 = 0.5;

/// Follows the open map's view: motion from frame to frame (cheap, every
/// frame) and place names (OCR, on a worker, a few times a second). The
/// app places markers from the last names read, moved by the motion since.
#[derive(Debug, Default)]
struct MapView {
    fingerprint: Option<u64>,
    read_at: Option<Instant>,
    /// The view changed since the last read started.
    dirty: bool,
    tracker: MotionTracker,
    /// Motion since the frame of the labels the app has; `None` when lost.
    since_labels: Option<Motion>,
    /// While a read runs: motion from the previous labels' frame to the
    /// frame being read, and since the frame being read.
    reading: Option<(Option<Motion>, Option<Motion>)>,
    /// A read started before the map closed: drop its result.
    stale_read: bool,
    /// The motion last sent.
    sent: Sent,
    /// The pointer last sent, normalised.
    pointer: Option<(f32, f32)>,
}

/// What the app was last told about the map's motion.
#[derive(Debug, Clone, Copy, Default)]
enum Sent {
    /// Nothing since its labels.
    #[default]
    Nothing,
    /// This motion (`None`: lost).
    Motion(Option<Motion>),
}

/// What the overlay draws on the map, normalised to the screen: markers'
/// centres and the outlines of shaded areas. Tracking ignores these pixels
/// (they are in the capture but lag the map; see
/// [`arclens_vision::Footprint`]).
#[derive(Debug, Clone, Default)]
pub struct Drawn {
    pub badges: Vec<(f32, f32)>,
    /// Points along area outlines and their count badges.
    pub outlines: Vec<(f32, f32)>,
}

/// The last two [`Drawn`]: the overlay may still show the previous one.
fn drawn() -> &'static std::sync::Mutex<[Drawn; 2]> {
    static DRAWN: std::sync::OnceLock<std::sync::Mutex<[Drawn; 2]>> = std::sync::OnceLock::new();
    DRAWN.get_or_init(Default::default)
}

/// Tells tracking what the overlay now draws.
pub fn set_drawn(now: Drawn) {
    let mut d = drawn()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    d[0] = std::mem::replace(&mut d[1], now);
}

/// Badge radius on screen, logical pixels (the overlay's badge is 22 px
/// across with a rim), plus slack for anti-aliasing.
const BADGE_RADIUS: f32 = 14.0;
/// Half-width of an area's outline band, logical pixels: the outline is
/// drawn 9 px outside the hull, with its count badge at the centre.
const OUTLINE_RADIUS: f32 = 12.0;

/// [`Drawn`] in frame pixels, for a frame of `size` showing a monitor
/// `px_per_logical` frame pixels per logical pixel wide.
fn footprint(size: (f32, f32), px_per_logical: f32) -> arclens_vision::Footprint {
    let d = drawn()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut footprint = arclens_vision::Footprint::default();
    for drawn in d.iter() {
        for &(x, y) in &drawn.badges {
            footprint
                .circles
                .push((x * size.0, y * size.1, BADGE_RADIUS * px_per_logical));
        }
        for &(x, y) in &drawn.outlines {
            footprint
                .circles
                .push((x * size.0, y * size.1, OUTLINE_RADIUS * px_per_logical));
        }
    }
    footprint
}

/// Whether the main menu is up, with hysteresis: two frames in a row to
/// appear, [`MAP_CLOSE_GRACE`] gone to disappear.
#[derive(Debug, Default)]
struct MenuWatch {
    shown: bool,
    seen: u8,
    missing_since: Option<Instant>,
}

impl MenuWatch {
    fn check(&mut self, frame: &RgbImage, out: &mut Outbox) {
        if arclens_vision::is_main_menu(frame) {
            self.missing_since = None;
            self.seen = self.seen.saturating_add(1);
            if !self.shown && self.seen >= 2 {
                self.shown = true;
                out.send_lossy(Event::MainMenu(true));
            }
            return;
        }
        self.seen = 0;
        if self.shown
            && self
                .missing_since
                .get_or_insert_with(Instant::now)
                .elapsed()
                >= MAP_CLOSE_GRACE
        {
            self.shown = false;
            self.missing_since = None;
            out.send_lossy(Event::MainMenu(false));
        }
    }
}

/// Reads a workshop station's level now and then, reporting changes.
#[derive(Debug, Default)]
struct StationWatch {
    read_at: Option<Instant>,
    last: Option<arclens_vision::StationLevel>,
    last_levels: Vec<(String, u32)>,
    last_quests: Vec<String>,
}

impl StationWatch {
    fn check(&mut self, analyzer: &Analyzer, frame: &RgbImage, out: &mut Outbox) {
        if self
            .read_at
            .is_some_and(|at| at.elapsed() < STATION_INTERVAL)
        {
            return;
        }
        self.read_at = Some(Instant::now());
        // The workshop overview shows every station's level at once.
        if arclens_vision::is_workshop_overview(frame) {
            let levels: Vec<(String, u32)> = arclens_vision::read_workshop_levels(frame)
                .into_iter()
                .filter_map(|(tile, level)| {
                    arclens_vision::WORKSHOP_TILES
                        .get(tile)
                        .map(|id| ((*id).to_owned(), level))
                })
                .collect();
            if !levels.is_empty() && levels != self.last_levels {
                self.last_levels.clone_from(&levels);
                out.send_lossy(Event::WorkshopLevels(levels));
            }
            return;
        }
        // The logbook and the traders' quest pages list quests in progress.
        if arclens_vision::quest_screen(frame).is_some() {
            if let Ok(quests) = analyzer.read_active_quests(frame)
                && !quests.is_empty()
                && quests != self.last_quests
            {
                self.last_quests.clone_from(&quests);
                out.send_lossy(Event::ActiveQuests(quests));
            }
            return;
        }
        if let Ok(Some(station)) = analyzer.read_station_header(frame)
            && self.last.as_ref() != Some(&station)
        {
            self.last = Some(station.clone());
            out.send_lossy(Event::StationLevel(station));
        }
    }
}

/// Place names read on one map frame.
#[derive(Debug, Clone)]
pub struct MapLabels {
    /// Boxes in frame pixels.
    pub labels: Vec<arclens_data::anchors::ScreenLabel>,
    /// Frame size, pixels.
    pub frame: (f32, f32),
    /// The quest panel covers the map's left.
    pub quests_open: bool,
    /// How the map moved from the previous `MapLabels`' frame to this one
    /// (`None`: unknown), so a frame whose names can't be matched still
    /// carries the view forward.
    pub moved: Option<Motion>,
}

impl MapView {
    /// The map closed: start over next time. A read still running belongs
    /// to the old view.
    fn reset(&mut self) {
        if self.fingerprint.is_none() && self.reading.is_none() {
            return;
        }
        let stale = self.reading.is_some() || self.stale_read;
        self.tracker.reset();
        *self = Self {
            tracker: std::mem::take(&mut self.tracker),
            stale_read: stale,
            ..Self::default()
        };
    }
}

/// Reads map labels on its own thread so tracking never waits for OCR.
struct LabelWorker {
    frames: std::sync::mpsc::Sender<RgbImage>,
    results: std::sync::mpsc::Receiver<MapLabels>,
}

impl LabelWorker {
    fn spawn(model: &Path) -> Option<Self> {
        let mut analyzer = NameReader::from_model_file(model).ok().map(Analyzer::new)?;
        let (frames, frame_rx) = std::sync::mpsc::channel::<RgbImage>();
        let (result_tx, results) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("arclens-labels".into())
            // Ends when the vision thread drops `frames`.
            .spawn(move || {
                while let Ok(frame) = frame_rx.recv() {
                    let read = read_labels(&mut analyzer, &frame);
                    if result_tx.send(read).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self { frames, results })
    }
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
    /// Place names read on the open map. Sent when the view changed.
    MapLabels(MapLabels),
    /// How the map moved (frame pixels) since the frame of the last
    /// `MapLabels`; `None` when tracking lost it. Sent while it moves.
    MapMotion(Option<Motion>),
    /// A workshop station's page shows its level (sent when it changes).
    StationLevel(arclens_vision::StationLevel),
    /// The workshop overview: station id and level for each tile read.
    WorkshopLevels(Vec<(String, u32)>),
    /// Quest titles the game lists as in progress (as read).
    ActiveQuests(Vec<String>),
    /// The game's main menu appeared (`true`) or went away.
    MainMenu(bool),
    /// Where the pointer is over the open map, normalised to the screen
    /// (`None`: unknown). Sent when it moves.
    MapPointer(Option<(f32, f32)>),
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

    /// Where the pointer is, in frame pixels, if known.
    fn cursor(&self) -> Option<(f32, f32)> {
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

    /// For updates the next one supersedes: a full queue drops it.
    fn send_lossy(&mut self, event: Event) {
        if let Err(error) = self.0.try_send(event)
            && error.is_disconnected()
        {
            tracing::debug!("UI gone");
        }
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
    let model = ensure_model(&paths).map_err(|e| format!("{e:#}"));
    let analyzer = model
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|model| NameReader::from_model_file(model).map_err(|e| format!("{e:#}")));
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

    let monitor = source.monitor();
    if let Some(monitor) = monitor {
        out.send(Event::Monitor(monitor));
    }

    let mut last: Option<Hover> = None;
    let mut fast_until = Instant::now();
    // When the map header was last read, while the map is open.
    let mut map_watch = MapWatch::default();
    let mut map_view = MapView::default();
    let mut station = StationWatch::default();
    let mut menu = MenuWatch::default();
    let labels = model.ok().and_then(|model| LabelWorker::spawn(&model));
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
            // Only frames that do show the map.
            if map_watch.missing_since.is_none() {
                report_pointer(source.cursor(), &frame, &mut map_view, &mut out);
            }
            if map_watch.missing_since.is_none()
                && let Some(labels) = &labels
                && !follow_map(
                    labels,
                    &frame,
                    &mut map_view,
                    &mut out,
                    monitor.map(|m| m.width),
                )
            {
                return;
            }
            // Sample quickly while the map is open: panning and zooming.
            source.set_interval(MAP_INTERVAL);
            continue;
        }
        map_view.reset();
        station.check(&analyzer, &frame, &mut out);
        menu.check(&frame, &mut out);
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

/// Tracks the map's motion and keeps a label read going while the view
/// changes. `false` when the UI is gone.
fn follow_map(
    worker: &LabelWorker,
    frame: &RgbImage,
    view: &mut MapView,
    out: &mut Outbox,
    monitor_width: Option<i32>,
) -> bool {
    #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
    let size = (frame.width() as f32, frame.height() as f32);
    #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
    let px_per_logical = monitor_width
        .filter(|&w| w > 0)
        .map_or(1.0, |w| size.0 / w as f32);
    // Motion since the previous frame (`None`: unknown).
    let drawn = footprint(size, px_per_logical);
    let step = match view.tracker.track_ignoring(frame, &drawn) {
        Some(e) if e.confidence >= MIN_CONFIDENCE => Some(e.motion),
        Some(e) => {
            tracing::debug!(confidence = e.confidence, "map motion lost");
            None
        }
        // First frame: nothing to compare with yet.
        None => None,
    };
    let compose = |since: Option<Motion>| since.zip(step).map(|(since, step)| step.after(&since));
    view.since_labels = compose(view.since_labels);
    if let Some((_, since)) = &mut view.reading {
        *since = compose(*since);
    }
    let fingerprint = view_fingerprint(frame);
    if view.fingerprint != Some(fingerprint) || step.is_none_or(|m| !m.is_still(size, 1.0)) {
        view.fingerprint = Some(fingerprint);
        view.dirty = true;
    }

    // A read finished: the app gets its labels, and the motion since.
    if let Ok(mut read) = worker.results.try_recv() {
        if std::mem::take(&mut view.stale_read) {
            // Started before the map closed; the view is unrelated.
        } else if let Some((moved, since)) = view.reading.take() {
            read.moved = moved;
            view.since_labels = since;
            view.sent = Sent::Nothing;
            if !out.send(Event::MapLabels(read)) {
                return false;
            }
        }
    }
    // Start the next read when the view changed and the worker is free.
    if view.dirty
        && view.reading.is_none()
        && !view.stale_read
        && view.read_at.is_none_or(|at| at.elapsed() >= LABEL_INTERVAL)
        && worker.frames.send(frame.clone()).is_ok()
    {
        view.dirty = false;
        view.read_at = Some(Instant::now());
        view.reading = Some((view.since_labels, Some(Motion::NONE)));
    }

    // Tell the app how far the map moved since its labels.
    let changed = match (view.sent, view.since_labels) {
        (Sent::Nothing, _) => true,
        (Sent::Motion(Some(a)), Some(b)) => !Motion {
            scale: b.scale / a.scale,
            dx: b.dx - b.scale / a.scale * a.dx,
            dy: b.dy - b.scale / a.scale * a.dy,
        }
        .is_still(size, MOTION_EPSILON),
        (Sent::Motion(a), b) => a.is_some() != b.is_some(),
    };
    if changed {
        view.sent = Sent::Motion(view.since_labels);
        // Superseded by the next one: dropped, not fatal, when the UI lags.
        out.send_lossy(Event::MapMotion(view.since_labels));
    }
    true
}

/// Sends the pointer's position when it moved by a pixel or more.
fn report_pointer(
    cursor: Option<(f32, f32)>,
    frame: &RgbImage,
    view: &mut MapView,
    out: &mut Outbox,
) {
    #[allow(clippy::cast_precision_loss, reason = "pixel sizes")]
    let size = (frame.width() as f32, frame.height() as f32);
    let now = cursor.map(|(x, y)| (x / size.0, y / size.1));
    let moved = match (view.pointer, now) {
        (Some(a), Some(b)) => {
            (a.0 - b.0).abs() * size.0 >= 1.0 || (a.1 - b.1).abs() * size.1 >= 1.0
        }
        (a, b) => a.is_some() != b.is_some(),
    };
    if moved {
        view.pointer = now;
        out.send_lossy(Event::MapPointer(now));
    }
}

/// Reads the place names on a map frame.
fn read_labels(analyzer: &mut Analyzer, frame: &RgbImage) -> MapLabels {
    #[allow(clippy::cast_precision_loss, reason = "pixel coordinates")]
    let size = (frame.width() as f32, frame.height() as f32);
    let labels = analyzer.read_map_labels(frame).unwrap_or_else(|error| {
        tracing::warn!(error = format!("{error:#}"), "map labels unreadable");
        Vec::new()
    });
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
    MapLabels {
        labels,
        frame: size,
        quests_open: arclens_vision::quest_panel_open(frame),
        moved: None,
    }
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
    Ok(Box::new(Live(capture)))
}

/// Live frames from the screen.
struct Live(arclens_capture::Capture);

impl FrameSource for Live {
    fn next_frame(&mut self) -> Option<RgbImage> {
        self.0.next_frame()
    }

    fn set_interval(&mut self, interval: Duration) {
        self.0.set_interval(interval);
    }

    fn cursor(&self) -> Option<(f32, f32)> {
        self.0.cursor()
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

    /// `ARCLENS_REPLAY_CURSOR=x,y` (frame pixels): a pointer for testing.
    fn cursor(&self) -> Option<(f32, f32)> {
        let value = std::env::var("ARCLENS_REPLAY_CURSOR").ok()?;
        let (x, y) = value.split_once(',')?;
        Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
    }
}
