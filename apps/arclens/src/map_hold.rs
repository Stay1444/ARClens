//! Keeps the in-game map "open" while the pointer is on the overlay's map
//! panel. Using the panel can cost the game its focus (the compositor
//! shows the taskbar over it, or the player alt-tabs to type), and the
//! frame then stops looking like the map screen. A close seen then is
//! held, and applied only if the map hasn't come back [`GRACE`] after the
//! pointer left the panel.

use std::time::{Duration, Instant};

/// How long after leaving the panel a held close waits for the map to
/// show again.
pub const GRACE: Duration = Duration::from_millis(1500);

#[derive(Debug, Default)]
pub struct MapHold {
    hovered: bool,
    /// A close was seen while held; applied once due.
    pending: bool,
    /// When the pointer left the panel with a close pending.
    left_at: Option<Instant>,
}

impl MapHold {
    /// The pointer entered (`true`) or left the panel.
    pub fn hover(&mut self, hovered: bool, now: Instant) {
        self.hovered = hovered;
        self.left_at = (!hovered && self.pending).then_some(now);
    }

    /// Vision says the map closed: `true` to close it now, `false` when
    /// held.
    pub fn closed(&mut self, now: Instant) -> bool {
        if self.hovered || self.left_at.is_some_and(|at| now - at < GRACE) {
            self.pending = true;
            return false;
        }
        self.pending = false;
        true
    }

    /// The map is on screen again: nothing to close.
    pub fn opened(&mut self) {
        self.pending = false;
        self.left_at = None;
    }

    /// Whether a held close should be applied now (clears it).
    pub fn due(&mut self, now: Instant) -> bool {
        let due = self.pending && !self.hovered && self.left_at.is_none_or(|at| now - at >= GRACE);
        if due {
            self.pending = false;
            self.left_at = None;
        }
        due
    }

    /// Whether a close is held (the app then checks [`Self::due`]).
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// Capture stopped or the overlay went away: forget everything.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_at_once_without_the_panel() {
        let mut hold = MapHold::default();
        assert!(hold.closed(Instant::now()));
        assert!(!hold.is_pending());
    }

    #[test]
    fn holds_while_hovered_and_closes_after_the_grace() {
        let t0 = Instant::now();
        let mut hold = MapHold::default();
        hold.hover(true, t0);
        assert!(!hold.closed(t0));
        assert!(hold.is_pending());
        assert!(!hold.due(t0 + GRACE * 4), "still on the panel");
        hold.hover(false, t0 + GRACE);
        assert!(!hold.due(t0 + GRACE + GRACE / 2));
        assert!(hold.due(t0 + GRACE * 2));
        assert!(!hold.is_pending());
    }

    #[test]
    fn the_map_coming_back_cancels_the_close() {
        let t0 = Instant::now();
        let mut hold = MapHold::default();
        hold.hover(true, t0);
        assert!(!hold.closed(t0));
        hold.hover(false, t0);
        hold.opened();
        assert!(!hold.due(t0 + GRACE * 2));
    }

    #[test]
    fn a_close_just_after_leaving_is_held_too() {
        let t0 = Instant::now();
        let mut hold = MapHold::default();
        hold.hover(true, t0);
        assert!(!hold.closed(t0));
        hold.hover(false, t0);
        // Vision's own grace re-reports the close shortly after.
        assert!(!hold.closed(t0 + GRACE / 2));
        assert!(hold.due(t0 + GRACE));
    }
}
