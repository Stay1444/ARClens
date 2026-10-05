//! The stash scan: what the player has, read from the stash grid on the
//! INVENTORY tab as they scroll through it.
//!
//! - **Identifying a slot.** A slot whose picture matches one the player
//!   hovered before ([`Exemplars`]) is that item, for sure. Otherwise its
//!   icon is ranked against the dataset's item images ([`Identifier`]);
//!   tiers and blueprints share an image, so the slot's tier numeral picks
//!   the tier. A slot hovered right now is what its tooltip says.
//! - **Learning.** A hovered slot's picture is stored only once the cursor
//!   has left it (the hover frame has the pointer, KDE's or the game's, on
//!   top of the icon).
//! - **Scrolling.** Each still frame shows a window of rows; [`Scan`]
//!   stitches windows together by matching slot pictures, preferring the
//!   placement nearest the previous scroll position (identical rows of
//!   ammo stacks would fit in several places).
//! - **Done.** The stash header says how many slots are used ("75/280"):
//!   when that many non-empty slots have been seen, the scan is complete
//!   and becomes a [`Snapshot`] in the [`History`].

#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "row and slot counts are tiny (a stash has at most a few hundred \
              slots); row indices are checked non-negative before use"
)]

use arclens_core::ItemId;
use arclens_vision::{
    IconFeatures, IconIndex, SAME_ROW_SLOT, SAME_SLOT, SlotBadge, SlotThumb, thumb_distance,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Slots per stash row.
pub const COLUMNS: usize = 4;

/// An item's family: its tiers and blueprint share one image
/// (`osprey_ii` → `osprey`, `vulcano_blueprint` → `vulcano`).
pub fn family(id: &str) -> &str {
    let id = id.strip_suffix("_blueprint").unwrap_or(id);
    match id.rsplit_once('_') {
        Some((stem, "i" | "ii" | "iii" | "iv")) => stem,
        _ => id,
    }
}

/// The tier an id ends in (`osprey_ii` → 2).
pub fn id_tier(id: &str) -> Option<u8> {
    match id.rsplit('_').next()? {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        _ => None,
    }
}

/// Ranks slot icons against the dataset's item images.
#[derive(Debug, Default)]
pub struct Identifier {
    index: IconIndex<ItemId>,
}

impl Identifier {
    pub fn add(&mut self, id: ItemId, icon: &image::RgbaImage) {
        self.index.add(id, icon);
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// The likeliest item: the best-ranked family, then the member with
    /// the slot's tier (or, without a tier, the best-ranked item that
    /// isn't a blueprint).
    pub fn identify(&self, features: &IconFeatures, tier: Option<u8>) -> Option<ItemId> {
        let ranked = self.index.rank(features, 12);
        let best = family(ranked.first()?.0.as_str()).to_owned();
        let members: Vec<&ItemId> = ranked
            .iter()
            .map(|(id, _)| id)
            .filter(|id| family(id.as_str()) == best)
            .collect();
        if let Some(tier) = tier {
            // The member with that tier, even if it ranked lower (tiers
            // look alike); the dataset id is predictable when it didn't.
            if let Some(id) = members.iter().find(|id| id_tier(id.as_str()) == Some(tier)) {
                return Some((*id).clone());
            }
            let numeral = ["i", "ii", "iii", "iv"][usize::from(tier.clamp(1, 4)) - 1];
            return Some(ItemId::new(format!("{best}_{numeral}")));
        }
        members
            .iter()
            .find(|id| !id.as_str().ends_with("_blueprint"))
            .or(members.first())
            .map(|id| (*id).clone())
    }
}

/// Slot pictures of items the player hovered, stored once the cursor had
/// left the slot. Exact: the same render, the same stack size.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Exemplars {
    entries: Vec<Exemplar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Exemplar {
    item: ItemId,
    thumb: Vec<u8>,
}

/// Exemplars kept per item (stacks of different sizes look different).
const EXEMPLARS_PER_ITEM: usize = 6;

impl Exemplars {
    pub fn load(path: &Path) -> Self {
        crate::store::load(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        crate::store::save(path, self);
    }

    /// Remembers `thumb` as `item`. `false` when it was already known.
    pub fn add(&mut self, item: ItemId, thumb: &SlotThumb) -> bool {
        if self.lookup(thumb).is_some_and(|known| known == item) {
            return false;
        }
        // A picture now known as something else was misread before.
        self.entries
            .retain(|e| thumb_distance(&SlotThumb(e.thumb.clone()), thumb) >= SAME_SLOT);
        let same_item: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].item == item)
            .collect();
        if same_item.len() >= EXEMPLARS_PER_ITEM {
            self.entries.remove(same_item[0]);
        }
        self.entries.push(Exemplar {
            item,
            thumb: thumb.0.clone(),
        });
        true
    }

    /// The item whose stored picture `thumb` matches, if any.
    pub fn lookup(&self, thumb: &SlotThumb) -> Option<ItemId> {
        self.entries
            .iter()
            .map(|e| (thumb_distance(&SlotThumb(e.thumb.clone()), thumb), &e.item))
            .filter(|(d, _)| *d < SAME_SLOT)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, item)| item.clone())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// How sure we are of a slot's item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    /// From its tooltip, or a picture the player hovered before.
    Sure,
    /// From the icon alone.
    Likely,
}

/// One slot as scanned.
#[derive(Debug, Clone)]
pub struct Slot {
    pub thumb: SlotThumb,
    pub empty: bool,
    pub item: Option<(ItemId, Certainty)>,
    /// Stack size; 1 without a number.
    pub quantity: u32,
}

/// What the vision worker read from one slot.
#[derive(Debug, Clone)]
pub struct SlotRead {
    pub thumb: SlotThumb,
    pub empty: bool,
    pub features: Option<IconFeatures>,
    pub badge: Option<SlotBadge>,
}

impl Slot {
    /// Identifies a freshly read slot.
    pub fn from_read(read: SlotRead, exemplars: &Exemplars, identifier: &Identifier) -> Self {
        let tier = match read.badge {
            Some(SlotBadge::Tier(t)) => Some(t),
            _ => None,
        };
        let quantity = match read.badge {
            Some(SlotBadge::Quantity(n)) => n.max(1),
            _ => 1,
        };
        let item = if read.empty {
            None
        } else if let Some(item) = exemplars.lookup(&read.thumb) {
            Some((item, Certainty::Sure))
        } else {
            read.features
                .as_ref()
                .and_then(|f| identifier.identify(f, tier))
                .map(|item| (item, Certainty::Likely))
        };
        Self {
            thumb: read.thumb,
            empty: read.empty,
            item,
            quantity,
        }
    }
}

/// Where the grid was last seen: which stitched row is at the top of the
/// view, and at what height.
#[derive(Debug, Clone, Copy)]
struct View {
    first: isize,
    top: f32,
    /// The scrollbar thumb's position then.
    scroll: Option<f32>,
}

/// The rows seen so far, stitched across scrolling.
#[derive(Debug, Default)]
pub struct Scan {
    rows: Vec<Vec<Slot>>,
    view: Option<View>,
    /// Content pixels scrolled per pixel the scrollbar thumb moves (it
    /// depends on how many rows the stash has), learned as it scrolls.
    thumb_ratio: Option<f32>,
    /// "75/280": slots used and capacity, from the stash header.
    pub count: Option<(u32, u32)>,
}

/// Content pixels per thumb pixel before any were measured (a stash of
/// ~19 rows at 1440p).
const DEFAULT_THUMB_RATIO: f32 = 10.0;

/// Thumbs of the rows a frame shows, top to bottom (four per row).
pub type Window = Vec<Vec<SlotThumb>>;

/// Where a window sits on screen.
#[derive(Debug, Clone, Copy)]
pub struct Position {
    /// The first full row's top, pixels.
    pub top: f32,
    /// Row spacing, pixels.
    pub pitch: f32,
    /// The scrollbar thumb's top, pixels (`None`: no scrollbar seen).
    pub scroll: Option<f32>,
}

impl Scan {
    /// Where the window's first row goes among the stitched rows: every
    /// overlapping row must match. Several placements can fit (rows of
    /// identical stacks); the one that scrolled the least wins, in the
    /// direction the scrollbar thumb moved (`scroll`; after half a row
    /// either way fits otherwise). `pitch`: row spacing in pixels. `None`
    /// when nothing fits (scrolled too far between still frames, or the
    /// stash changed).
    pub fn place(&self, window: &Window, at: Position) -> Option<isize> {
        let Position { top, pitch, scroll } = at;
        if self.rows.is_empty() {
            return Some(0);
        }
        let len = self.rows.len() as isize;
        let n = window.len() as isize;
        // How far the content scrolled (pixels, down positive) if the
        // window's first row is stitched row `first`.
        let scrolled = |first: isize| {
            self.view
                .map_or(0.0, |v| (first - v.first) as f32 * pitch - (top - v.top))
        };
        let thumb_moved = self
            .view
            .and_then(|v| Some(scroll? - v.scroll?))
            .filter(|d| d.abs() >= 1.0);
        let ratio = self.thumb_ratio.unwrap_or(DEFAULT_THUMB_RATIO);
        // The scroll the thumb suggests, else none.
        let expected = thumb_moved.map_or(0.0, |t| t * ratio);
        let cost = |first: isize| (scrolled(first) - expected).abs();
        let mut best: Option<(isize, f32)> = None;
        // Placements that overlap the stitched rows by at least one row.
        for first in (1 - n)..len {
            let mut overlaps = 0;
            let fits = (0..n).all(|i| {
                let at = first + i;
                if at < 0 || at >= len {
                    return true;
                }
                overlaps += 1;
                rows_match(&self.rows[at as usize], &window[i as usize])
            });
            if !fits || overlaps == 0 {
                continue;
            }
            let c = cost(first);
            if best.is_none_or(|(_, b)| c < b) {
                best = Some((first, c));
            }
        }
        best.map(|(first, _)| first)
    }

    /// Stitches a window in at `first` (from [`Self::place`]); `read`
    /// supplies slots for rows not seen before (identified by the caller).
    /// Returns the stitched index of the window's first row (it shifts when
    /// rows are added above).
    pub fn stitch(
        &mut self,
        first: isize,
        at: Position,
        window_len: usize,
        mut read: impl FnMut(usize) -> Vec<Slot>,
    ) -> usize {
        self.learn_thumb_ratio(first, at);
        let Position { top, scroll, .. } = at;
        let mut first = first;
        if first < 0 {
            let missing = first.unsigned_abs();
            let above: Vec<Vec<Slot>> = (0..missing).map(&mut read).collect();
            self.rows.splice(0..0, above);
            if let Some(view) = &mut self.view {
                view.first += missing as isize;
            }
            first = 0;
        }
        let first = first as usize;
        for i in 0..window_len {
            if first + i >= self.rows.len() {
                let row = read(i);
                self.rows.push(row);
            }
        }
        self.view = Some(View {
            first: first as isize,
            top,
            scroll,
        });
        first
    }

    /// Learns how far the content scrolls per pixel of thumb movement from
    /// a step that moved both clearly.
    fn learn_thumb_ratio(&mut self, first: isize, at: Position) {
        let (Some(v), Some(now)) = (self.view, at.scroll) else {
            return;
        };
        let Some(then) = v.scroll else {
            return;
        };
        let rows = (first - v.first) as f32;
        let thumb = now - then;
        if thumb.abs() < 3.0 || rows == 0.0 {
            return;
        }
        let ratio = (rows * at.pitch - (at.top - v.top)) / thumb;
        if (1.0..200.0).contains(&ratio) {
            self.thumb_ratio = Some(self.thumb_ratio.map_or(ratio, |r| 0.7 * r + 0.3 * ratio));
        }
    }

    /// Starts over (the stash changed under us), keeping the header count.
    pub fn restart(&mut self) {
        self.rows.clear();
        self.view = None;
        self.thumb_ratio = None;
    }

    /// The stitched index of the top visible row, if the grid is where it
    /// was at the last stitch (within a pixel).
    pub fn view_first(&self, top: f32) -> Option<usize> {
        self.view
            .filter(|v| (v.top - top).abs() <= 1.5)
            .and_then(|v| usize::try_from(v.first).ok())
    }

    pub fn row(&self, index: usize) -> Option<&[Slot]> {
        self.rows.get(index).map(Vec::as_slice)
    }

    pub fn slot_mut(&mut self, row: usize, column: usize) -> Option<&mut Slot> {
        self.rows.get_mut(row)?.get_mut(column)
    }

    /// Non-empty slots seen.
    pub fn used(&self) -> u32 {
        self.rows.iter().flatten().filter(|s| !s.empty).count() as u32
    }

    /// Every used slot has been seen (the header's count matches).
    pub fn complete(&self) -> bool {
        self.count
            .is_some_and(|(used, _)| used > 0 && self.used() >= used)
    }

    /// Re-identifies slots that only had a likely item, now that a new
    /// exemplar may match them.
    pub fn refine(&mut self, exemplars: &Exemplars) {
        for slot in self.rows.iter_mut().flatten() {
            if slot.empty || matches!(slot.item, Some((_, Certainty::Sure))) {
                continue;
            }
            if let Some(item) = exemplars.lookup(&slot.thumb) {
                slot.item = Some((item, Certainty::Sure));
            }
        }
    }

    pub fn snapshot(&self, taken_at: i64) -> Snapshot {
        let mut items: BTreeMap<ItemId, u32> = BTreeMap::new();
        let mut likely = 0;
        let mut unknown = 0;
        for slot in self.rows.iter().flatten().filter(|s| !s.empty) {
            match &slot.item {
                Some((item, certainty)) => {
                    *items.entry(item.clone()).or_default() += slot.quantity;
                    likely += u32::from(*certainty == Certainty::Likely);
                }
                None => unknown += 1,
            }
        }
        Snapshot {
            taken_at,
            items,
            slots: self.used(),
            likely,
            unknown,
        }
    }
}

fn rows_match(a: &[Slot], b: &[SlotThumb]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(slot, thumb)| thumb_distance(&slot.thumb, thumb) < SAME_ROW_SLOT)
}

/// A finished scan: item counts at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unix seconds.
    pub taken_at: i64,
    pub items: BTreeMap<ItemId, u32>,
    /// Slots in use.
    pub slots: u32,
    /// Slots identified from their icon alone.
    pub likely: u32,
    /// Slots not identified.
    pub unknown: u32,
}

impl Snapshot {
    pub fn count(&self, item: &ItemId) -> u32 {
        self.items.get(item).copied().unwrap_or(0)
    }

    /// Changes from `before` to `self`: items whose count went up or down,
    /// largest change first.
    pub fn changes_since(&self, before: &Self) -> Vec<(ItemId, i64)> {
        let mut changes: Vec<(ItemId, i64)> = self
            .items
            .keys()
            .chain(before.items.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|id| {
                (
                    id.clone(),
                    i64::from(self.count(id)) - i64::from(before.count(id)),
                )
            })
            .filter(|(_, d)| *d != 0)
            .collect();
        changes.sort_by_key(|(id, d)| (std::cmp::Reverse(d.abs()), id.clone()));
        changes
    }

    /// Total sell value at the dataset's prices.
    pub fn value(&self, lookup: impl Fn(&ItemId) -> Option<u32>) -> u64 {
        self.items
            .iter()
            .map(|(id, n)| u64::from(lookup(id).unwrap_or(0)) * u64::from(*n))
            .sum()
    }
}

/// Past scans, oldest first.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    pub scans: Vec<Snapshot>,
}

/// Scans kept.
const HISTORY_LEN: usize = 100;

impl History {
    pub fn load(path: &Path) -> Self {
        crate::store::load(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        crate::store::save(path, self);
    }

    pub fn latest(&self) -> Option<&Snapshot> {
        self.scans.last()
    }

    /// Adds a scan unless it says the same as the latest. `true` if added.
    pub fn push(&mut self, scan: Snapshot) -> bool {
        if self.latest().is_some_and(|last| last.items == scan.items) {
            return false;
        }
        self.scans.push(scan);
        if self.scans.len() > HISTORY_LEN {
            self.scans.remove(0);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_vision::THUMB_SIDE;

    fn thumb(shade: u8) -> SlotThumb {
        SlotThumb(vec![shade; (THUMB_SIDE * THUMB_SIDE * 3) as usize])
    }

    fn slot(shade: u8) -> Slot {
        Slot {
            thumb: thumb(shade),
            empty: false,
            item: Some((ItemId::new(format!("item_{shade}")), Certainty::Likely)),
            quantity: 1,
        }
    }

    /// A window of rows; each row is four slots of one shade.
    fn window(shades: &[u8]) -> Window {
        shades.iter().map(|&s| vec![thumb(s); COLUMNS]).collect()
    }

    fn at(top: f32, scroll: Option<f32>) -> Position {
        Position {
            top,
            pitch: 139.0,
            scroll,
        }
    }

    fn stitch_at(scan: &mut Scan, shades: &[u8], top: f32, scroll: Option<f32>) -> Option<usize> {
        let w = window(shades);
        let first = scan.place(&w, at(top, scroll))?;
        Some(scan.stitch(first, at(top, scroll), w.len(), |i| {
            vec![slot(shades[i]); COLUMNS]
        }))
    }

    fn stitch(scan: &mut Scan, shades: &[u8], top: f32) -> Option<usize> {
        stitch_at(scan, shades, top, None)
    }

    fn shades(scan: &Scan) -> Vec<u8> {
        scan.rows.iter().map(|r| r[0].thumb.0[0]).collect()
    }

    #[test]
    fn families_and_tiers() {
        assert_eq!(family("osprey_ii"), "osprey");
        assert_eq!(family("vulcano_blueprint"), "vulcano");
        assert_eq!(family("looting_mk2"), "looting_mk2");
        assert_eq!(family("light_ammo"), "light_ammo");
        assert_eq!(id_tier("anvil_iv"), Some(4));
        assert_eq!(id_tier("medium_ammo"), None);
    }

    #[test]
    fn stitches_rows_while_scrolling_down_and_up() {
        let mut scan = Scan::default();
        assert_eq!(stitch(&mut scan, &[10, 20, 30], 367.0), Some(0));
        // Scrolled down two rows.
        assert_eq!(stitch(&mut scan, &[30, 40, 50], 367.0), Some(2));
        assert_eq!(shades(&scan), [10, 20, 30, 40, 50]);
        // Back up past where the scan started (it began mid-stash).
        let mut scan = Scan::default();
        stitch(&mut scan, &[30, 40, 50], 367.0);
        assert_eq!(stitch(&mut scan, &[10, 20, 30], 367.0), Some(0));
        assert_eq!(shades(&scan), [10, 20, 30, 40, 50]);
    }

    #[test]
    fn identical_rows_are_placed_by_the_scrollbar() {
        // Rows of identical ammo stacks: [a, a, a, b]. Scrolled exactly one
        // row, the window [a, a] looks just like before; only the thumb
        // tells "one row down" from "no change".
        let mut scan = Scan::default();
        stitch_at(&mut scan, &[10, 10, 10, 20], 367.0, Some(362.0));
        let w = window(&[10, 10]);
        assert_eq!(scan.place(&w, at(367.0, Some(362.0))), Some(0));
        assert_eq!(scan.place(&w, at(367.0, Some(376.0))), Some(1));
    }

    #[test]
    fn the_scrollbar_settles_half_row_scrolls() {
        // After half a row of scrolling down, the next full row sits half a
        // row lower than the old first row did: as far from "one row down"
        // as from "same row". The thumb moved down, so it's one row down.
        let mut scan = Scan::default();
        stitch_at(&mut scan, &[10, 10, 10, 10, 20], 367.0, Some(362.0));
        let w = window(&[10, 10, 10]);
        assert_eq!(scan.place(&w, at(437.0, Some(369.0))), Some(1));
        assert_eq!(scan.place(&w, at(437.0, Some(355.0))), Some(0));
    }

    #[test]
    fn nothing_fits_after_a_jump() {
        let mut scan = Scan::default();
        stitch(&mut scan, &[10, 20], 367.0);
        assert_eq!(scan.place(&window(&[70, 80]), at(367.0, None)), None);
    }

    #[test]
    fn complete_when_the_header_count_is_reached() {
        let mut scan = Scan::default();
        stitch(&mut scan, &[10, 20], 367.0);
        scan.count = Some((8, 280));
        assert!(scan.complete());
        scan.count = Some((9, 280));
        assert!(!scan.complete());
        if let Some(s) = scan.slot_mut(1, 3) {
            s.empty = true;
        }
        scan.count = Some((8, 280));
        assert!(!scan.complete());
    }

    #[test]
    fn exemplars_are_exact_and_replace_misreads() {
        let mut ex = Exemplars::default();
        assert!(ex.add(ItemId::new("battery"), &thumb(50)));
        assert!(!ex.add(ItemId::new("battery"), &thumb(50)));
        assert_eq!(ex.lookup(&thumb(51)), Some(ItemId::new("battery")));
        assert_eq!(ex.lookup(&thumb(90)), None);
        // The same picture later turns out to be something else.
        assert!(ex.add(ItemId::new("chemicals"), &thumb(50)));
        assert_eq!(ex.lookup(&thumb(50)), Some(ItemId::new("chemicals")));
        assert_eq!(ex.len(), 1);
    }

    #[test]
    fn refine_upgrades_likely_slots() {
        let mut scan = Scan::default();
        stitch(&mut scan, &[10], 367.0);
        let mut ex = Exemplars::default();
        ex.add(ItemId::new("battery"), &thumb(10));
        scan.refine(&ex);
        let row = scan.row(0).unwrap();
        assert_eq!(row[0].item, Some((ItemId::new("battery"), Certainty::Sure)));
    }

    #[test]
    fn snapshots_count_stacks_and_show_changes() {
        let mut scan = Scan::default();
        stitch(&mut scan, &[10], 367.0);
        if let Some(s) = scan.slot_mut(0, 0) {
            s.quantity = 60;
        }
        let a = scan.snapshot(1);
        assert_eq!(a.count(&ItemId::new("item_10")), 63);
        assert_eq!(a.slots, 4);
        assert_eq!(a.likely, 4);
        let mut b = a.clone();
        b.items.insert(ItemId::new("item_10"), 60);
        b.items.insert(ItemId::new("lemon"), 2);
        assert_eq!(
            b.changes_since(&a),
            [(ItemId::new("item_10"), -3), (ItemId::new("lemon"), 2)]
        );
        let mut history = History::default();
        assert!(history.push(a.clone()));
        assert!(!history.push(a));
        assert!(history.push(b));
        assert_eq!(history.scans.len(), 2);
    }
}
