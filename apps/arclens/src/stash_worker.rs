//! Reads the stash grid on its own thread, so slot OCR and icon matching
//! never hold up hover detection. Fed still frames of the stash by the
//! vision loop; reports what each visible slot holds and the scan so far.
//! See [`crate::stash`] for how slots are identified and stitched.

#![allow(
    clippy::cast_precision_loss,
    reason = "pixel coordinates and frame sizes are far below 2^23"
)]

use crate::stash::{Certainty, Exemplars, Identifier, Scan, Slot, SlotRead, Snapshot};
use crate::vision::Event;
use arclens_core::{Item, ItemId};
use arclens_vision::{
    NameReader, Rect, hovered_slot, read_badge, read_stash_count, slot_features, slot_is_empty,
    slot_thumb, stash_slots,
};
use futures::channel::mpsc;
use image::RgbImage;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// What the worker needs from the app: the catalogue (to resolve tooltip
/// names) and every item image on disk (to rank icons against).
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub items: Arc<Vec<Item>>,
    pub icons: Vec<(ItemId, PathBuf)>,
}

fn context() -> &'static Mutex<Context> {
    static CONTEXT: OnceLock<Mutex<Context>> = OnceLock::new();
    CONTEXT.get_or_init(Default::default)
}

/// Hands the worker the catalogue and item images (call again when either
/// changes).
pub fn set_context(now: Context) {
    *context()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = now;
}

fn current_context() -> Context {
    context()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// One visible slot, for the overlay's badges.
#[derive(Debug, Clone)]
pub struct VisibleSlot {
    /// Normalised to the frame.
    pub rect: [f32; 4],
    pub item: Option<(ItemId, Certainty)>,
    pub quantity: u32,
}

/// The scan so far.
#[derive(Debug, Clone)]
pub struct ScanState {
    pub snapshot: Snapshot,
    /// Every used slot has been seen.
    pub complete: bool,
    /// Slots used, from the stash header.
    pub used: Option<u32>,
}

/// Jobs from the vision loop.
pub enum Job {
    /// A still frame of the stash; `tooltip`: the hovered item's name and
    /// tooltip panel, when one is shown.
    Frame {
        frame: RgbImage,
        tooltip: Option<(String, Rect)>,
    },
    /// The grid is scrolling: badges would lag, hide them.
    Moving,
    /// The stash closed.
    Closed,
}

pub struct Worker {
    jobs: std::sync::mpsc::SyncSender<Job>,
}

impl Worker {
    pub fn spawn(model: &Path, exemplars: PathBuf, events: mpsc::Sender<Event>) -> Option<Self> {
        let reader = NameReader::from_model_file(model).ok()?;
        // One frame in flight: a busy worker skips frames, it doesn't queue.
        let (jobs, rx) = std::sync::mpsc::sync_channel::<Job>(1);
        std::thread::Builder::new()
            .name("arclens-stash".into())
            .spawn(move || {
                let mut state = State::new(reader, exemplars, events);
                while let Ok(job) = rx.recv() {
                    if !state.handle(job) {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self { jobs })
    }

    /// Hands over a job unless the worker is still busy (frames are
    /// dropped then; `Moving`/`Closed` are retried by the caller's state).
    pub fn offer(&self, job: Job) -> bool {
        self.jobs.try_send(job).is_ok()
    }

    /// A worker whose jobs land in the returned receiver.
    #[cfg(test)]
    pub fn for_test() -> (Self, std::sync::mpsc::Receiver<Job>) {
        let (jobs, rx) = std::sync::mpsc::sync_channel(1);
        (Self { jobs }, rx)
    }
}

struct State {
    reader: NameReader,
    events: mpsc::Sender<Event>,
    exemplars_path: PathBuf,
    exemplars: Exemplars,
    identifier: Identifier,
    /// How many images the identifier was built from.
    built_from: usize,
    scan: Scan,
    /// Frames in a row that fit nowhere: the stash changed.
    misfits: u32,
    /// Hovered slots (stitched row, column) to learn once the cursor has
    /// left them, with the item their tooltip named.
    to_learn: Vec<(usize, usize, ItemId)>,
    last_sent: Option<Snapshot>,
    badges_shown: bool,
}

/// Frames that fit nowhere before the scan starts over.
const MISFITS_BEFORE_RESET: u32 = 3;

impl State {
    fn new(reader: NameReader, exemplars_path: PathBuf, events: mpsc::Sender<Event>) -> Self {
        Self {
            reader,
            events,
            exemplars: Exemplars::load(&exemplars_path),
            exemplars_path,
            identifier: Identifier::default(),
            built_from: 0,
            scan: Scan::default(),
            misfits: 0,
            to_learn: Vec::new(),
            last_sent: None,
            badges_shown: false,
        }
    }

    /// `false` when the UI is gone.
    fn handle(&mut self, job: Job) -> bool {
        match job {
            Job::Frame { frame, tooltip } => self.on_frame(&frame, tooltip),
            Job::Moving => self.hide_badges(),
            Job::Closed => {
                self.scan = Scan::default();
                self.to_learn.clear();
                self.misfits = 0;
                self.last_sent = None;
                self.hide_badges()
            }
        }
    }

    fn send(&mut self, event: Event) -> bool {
        match self.events.try_send(event) {
            Ok(()) => true,
            Err(e) => !e.is_disconnected(),
        }
    }

    fn hide_badges(&mut self) -> bool {
        if !std::mem::take(&mut self.badges_shown) {
            return true;
        }
        self.send(Event::StashView(Vec::new()))
    }

    fn on_frame(&mut self, frame: &RgbImage, tooltip: Option<(String, Rect)>) -> bool {
        let context = current_context();
        self.update_identifier(&context);
        let slots = stash_slots(frame);
        if slots.is_empty() {
            return true;
        }
        let rows: Vec<&[Rect]> = slots.chunks(crate::stash::COLUMNS).collect();
        let top = slots[0].y as f32;
        let pitch = rows
            .get(1)
            .map_or(139.0 * frame.height() as f32 / 1440.0, |r| {
                (r[0].y - rows[0][0].y) as f32
            });

        if self.scan.count.is_none() {
            self.scan.count = read_stash_count(&self.reader, frame).ok().flatten();
        }

        // A tooltip covers the slots right of the hovered one: don't read
        // or stitch this frame, only note what's hovered (the grid hasn't
        // moved since the last still frame, or the hover can't be placed).
        if let Some((name, panel)) = tooltip {
            self.note_hover(frame, &slots, &name, panel, top, &context);
            return self.send_view(frame, &slots, Some(panel), top);
        }

        let window: Vec<Vec<_>> = rows
            .iter()
            .map(|row| row.iter().map(|s| slot_thumb(frame, *s)).collect())
            .collect();
        let at = crate::stash::Position {
            top,
            pitch,
            scroll: arclens_vision::stash_scroll(frame),
        };
        let first = if let Some(first) = self.scan.place(&window, at) {
            self.misfits = 0;
            first
        } else {
            self.misfits += 1;
            if self.misfits < MISFITS_BEFORE_RESET {
                return true;
            }
            tracing::debug!("stash changed: scan starts over");
            self.scan.restart();
            self.to_learn.clear();
            self.misfits = 0;
            0
        };
        let (reader, exemplars, identifier) = (&self.reader, &self.exemplars, &self.identifier);
        let first = self.scan.stitch(first, at, rows.len(), |i| {
            rows[i]
                .iter()
                .map(|slot| Slot::from_read(read_slot(reader, frame, *slot), exemplars, identifier))
                .collect()
        });
        self.learn(frame, &rows, first);
        if !self.send_view(frame, &slots, None, top) {
            return false;
        }
        self.send_scan()
    }

    /// Builds (or rebuilds, when more images arrived) the icon index.
    fn update_identifier(&mut self, context: &Context) {
        if context.icons.len() <= self.built_from {
            return;
        }
        let started = std::time::Instant::now();
        let mut identifier = Identifier::default();
        for (id, path) in &context.icons {
            if let Ok(img) = image::open(path) {
                identifier.add(id.clone(), &img.into_rgba8());
            }
        }
        tracing::info!(
            icons = identifier.len(),
            took = ?started.elapsed(),
            "stash icon index built"
        );
        self.identifier = identifier;
        self.built_from = context.icons.len();
    }

    /// The hovered slot is what its tooltip says; remember to learn its
    /// picture once the cursor has left.
    fn note_hover(
        &mut self,
        frame: &RgbImage,
        slots: &[Rect],
        name: &str,
        panel: Rect,
        top: f32,
        context: &Context,
    ) {
        let Some(hovered) = hovered_slot(frame, slots, Some(panel)) else {
            return;
        };
        // Only when the grid is where it was last stitched.
        let Some(first) = self.scan_first_at(top) else {
            return;
        };
        let Some(index) = slots.iter().position(|s| *s == hovered) else {
            return;
        };
        let Some((item, _)) = arclens_data::match_name(name, &context.items) else {
            return;
        };
        let (row, column) = (
            first + index / crate::stash::COLUMNS,
            index % crate::stash::COLUMNS,
        );
        let id = item.id.clone();
        if let Some(slot) = self.scan.slot_mut(row, column)
            && !slot.empty
        {
            slot.item = Some((id.clone(), Certainty::Sure));
            if !self
                .to_learn
                .iter()
                .any(|(r, c, _)| (*r, *c) == (row, column))
            {
                self.to_learn.push((row, column, id));
            }
        }
    }

    /// The stitched index of the top visible row, if the grid hasn't moved
    /// since the last stitch.
    fn scan_first_at(&self, top: f32) -> Option<usize> {
        self.scan.view_first(top)
    }

    /// Stores clean pictures of slots hovered earlier (no tooltip now, so
    /// no cursor on them).
    fn learn(&mut self, frame: &RgbImage, rows: &[&[Rect]], first: usize) {
        let mut learned = false;
        let visible = first..first + rows.len();
        let to_learn = std::mem::take(&mut self.to_learn);
        for (row, column, item) in to_learn {
            if !visible.contains(&row) {
                self.to_learn.push((row, column, item));
                continue;
            }
            let rect = rows[row - first][column];
            let thumb = slot_thumb(frame, rect);
            if let Some(slot) = self.scan.slot_mut(row, column) {
                slot.thumb = thumb.clone();
                slot.item = Some((item.clone(), Certainty::Sure));
            }
            learned |= self.exemplars.add(item, &thumb);
        }
        if learned {
            self.exemplars.save(&self.exemplars_path);
            self.scan.refine(&self.exemplars);
            tracing::debug!(known = self.exemplars.len(), "stash slot learned");
        }
    }

    /// Badges for the visible slots (none under the tooltip).
    fn send_view(
        &mut self,
        frame: &RgbImage,
        slots: &[Rect],
        panel: Option<Rect>,
        top: f32,
    ) -> bool {
        let Some(first) = self.scan.view_first(top) else {
            return self.hide_badges();
        };
        let (w, h) = (frame.width() as f32, frame.height() as f32);
        let covered = |s: &Rect| {
            panel.is_some_and(|p| {
                s.right() > p.x && s.x < p.right() && s.bottom() > p.y && s.y < p.bottom()
            })
        };
        let view: Vec<VisibleSlot> = slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !covered(s))
            .filter_map(|(i, s)| {
                let slot = self
                    .scan
                    .row(first + i / crate::stash::COLUMNS)?
                    .get(i % crate::stash::COLUMNS)?;
                (!slot.empty).then(|| VisibleSlot {
                    rect: [
                        s.x as f32 / w,
                        s.y as f32 / h,
                        s.width as f32 / w,
                        s.height as f32 / h,
                    ],
                    item: slot.item.clone(),
                    quantity: slot.quantity,
                })
            })
            .collect();
        self.badges_shown = !view.is_empty();
        self.send(Event::StashView(view))
    }

    fn send_scan(&mut self) -> bool {
        let taken_at = jiff::Timestamp::now().as_second();
        let snapshot = self.scan.snapshot(taken_at);
        if self
            .last_sent
            .as_ref()
            .is_some_and(|s| s.items == snapshot.items && s.slots == snapshot.slots)
        {
            return true;
        }
        self.last_sent = Some(snapshot.clone());
        let state = ScanState {
            snapshot,
            complete: self.scan.complete(),
            used: self.scan.count.map(|(used, _)| used),
        };
        self.send(Event::StashScan(state))
    }
}

/// Reads one slot: its picture, icon features and corner badge.
fn read_slot(reader: &NameReader, frame: &RgbImage, slot: Rect) -> SlotRead {
    let empty = slot_is_empty(frame, slot);
    SlotRead {
        thumb: slot_thumb(frame, slot),
        empty,
        features: (!empty).then(|| slot_features(frame, slot)).flatten(),
        badge: if empty {
            None
        } else {
            read_badge(reader, frame, slot).ok().flatten()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays a recorded scroll through the stash (frames in
    /// `ARCLENS_STASH_REPLAY`, e.g. a screen recording at 30 fps), feeding
    /// still frames like the vision loop does, and prints the scan.
    /// Needs `ARCLENS_RAIDTHEORY_DIR` and `ARCLENS_OCR_MODEL`.
    #[test]
    #[ignore = "needs a recording, the dataset and the OCR model"]
    fn replays_a_recorded_scroll() {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from).expect(name);
        let (frames_dir, data, model) = (
            var("ARCLENS_STASH_REPLAY"),
            var("ARCLENS_RAIDTHEORY_DIR"),
            var("ARCLENS_OCR_MODEL"),
        );
        let catalog = arclens_data::raidtheory::RaidTheoryDir::new(&data)
            .load()
            .expect("dataset");
        let images = data.join("images/items");
        let icons = catalog
            .items
            .iter()
            .map(|i| (i.id.clone(), images.join(format!("{}.png", i.id.as_str()))))
            .filter(|(_, p)| p.is_file())
            .collect();
        set_context(Context {
            items: Arc::new(catalog.items.clone()),
            icons,
        });
        let (tx, mut rx) = mpsc::channel(10_000);
        let exemplars = tempfile::tempdir().expect("tempdir");
        let mut state = State::new(
            NameReader::from_model_file(&model).expect("model"),
            exemplars.path().join("ex.json"),
            tx,
        );
        let mut files: Vec<PathBuf> = std::fs::read_dir(&frames_dir)
            .expect("frames")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        files.sort();
        let mut previous: Vec<u32> = Vec::new();
        let mut still = 0;
        for path in &files {
            let frame = image::open(path).expect("frame").into_rgb8();
            let rows: Vec<u32> = stash_slots(&frame)
                .iter()
                .step_by(crate::stash::COLUMNS)
                .map(|s| s.y)
                .collect();
            let is_still = !rows.is_empty() && rows == previous;
            previous = rows;
            if is_still {
                still += 1;
                state.handle(Job::Frame {
                    frame,
                    tooltip: None,
                });
            }
        }
        let mut last = None;
        while let Ok(Some(event)) = rx.try_recv().map(Some) {
            if let Event::StashScan(scan) = event {
                last = Some(scan);
            }
        }
        let scan = last.expect("a scan");
        eprintln!(
            "{} frames, {still} still; {} slots of {:?}, complete {}, likely {}, unknown {}",
            files.len(),
            scan.snapshot.slots,
            scan.used,
            scan.complete,
            scan.snapshot.likely,
            scan.snapshot.unknown
        );
        for (id, n) in &scan.snapshot.items {
            eprintln!("  {n:>4} × {}", id.as_str());
        }
        assert!(scan.complete);
    }
}
