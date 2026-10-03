//! Hands frames from a capture backend's thread to the consumer: the
//! latest frame only, converted only when the consumer wants one and the
//! pace allows (so surplus frames cost nothing).

use image::RgbImage;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::IDLE_INTERVAL;

#[derive(Debug, Default)]
pub struct Slot {
    state: Mutex<State>,
    ready: Condvar,
    /// Minimum time between converted frames, in ms (0 = default).
    interval_ms: AtomicU64,
}

#[derive(Debug, Default)]
struct State {
    /// The consumer wants a frame converted.
    wanted: bool,
    frame: Option<RgbImage>,
    seq: u64,
    /// The backend stopped; no more frames will come.
    closed: bool,
}

impl Slot {
    pub fn set_interval(&self, interval: Duration) {
        let ms = u64::try_from(interval.as_millis()).unwrap_or(u64::MAX);
        self.interval_ms.store(ms, Ordering::Relaxed);
    }

    /// For the backend: whether to convert the frame at hand, given when it
    /// last converted one.
    pub fn wants_frame(&self, last_convert: Option<Instant>) -> bool {
        let interval = match self.interval_ms.load(Ordering::Relaxed) {
            0 => IDLE_INTERVAL,
            ms => Duration::from_millis(ms),
        };
        let due = last_convert.is_none_or(|t| t.elapsed() >= interval);
        due && self.lock().wanted
    }

    /// For the backend: hands over a converted frame.
    pub fn offer(&self, frame: RgbImage) {
        let mut state = self.lock();
        state.frame = Some(frame);
        state.seq += 1;
        state.wanted = false;
        self.ready.notify_all();
    }

    /// No more frames (backend ended, or the capture was dropped).
    pub fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }

    /// Blocks until a frame newer than `last_seq` is available; `None` once
    /// closed.
    pub fn next_frame(&self, last_seq: &mut u64) -> Option<RgbImage> {
        let mut state = self.lock();
        state.wanted = true;
        loop {
            if state.seq > *last_seq
                && let Some(frame) = state.frame.take()
            {
                *last_seq = state.seq;
                return Some(frame);
            }
            if state.closed {
                return None;
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn hands_over_only_wanted_frames_and_ends_on_close() {
        let slot = Arc::new(Slot::default());
        // Nobody asked yet.
        assert!(!slot.wants_frame(None));
        let consumer = std::thread::spawn({
            let slot = Arc::clone(&slot);
            move || {
                let mut seq = 0;
                let first = slot.next_frame(&mut seq);
                let second = slot.next_frame(&mut seq);
                (first.map(|f| f.width()), second)
            }
        });
        while !slot.wants_frame(None) {
            std::thread::yield_now();
        }
        slot.offer(RgbImage::new(3, 1));
        // Not due again right away at the idle pace.
        assert!(!slot.wants_frame(Some(Instant::now())));
        slot.close();
        assert_eq!(consumer.join().unwrap(), (Some(3), None));
    }
}
