# Capture guide: reference screenshots for vision

These captures become the test fixtures for `arclens-vision` (roadmap M3).
Detectors are developed and measured offline against them before any live
capture exists.

## Settings for every shot

- Full-screen PNG at **native resolution**. Use Spectacle → "Full Screen"
  with **"Include mouse pointer" on**.
- Game language **English**, HDR **off**.
- Write down your resolution and in-game UI scale. They go in
  `manifest.toml` next to the images.

## What to capture

We only need to **find** things on screen and **identify** the item. Rarity,
value and everything else come from the item database once we know which
item it is, so a small set is enough.

| # | Set | Count | Notes | Filename |
|---|---|---|---|---|
| 1 | Inventory, item hovered | ~10 | Any mix of items; 2 of them near the right or bottom screen edge, where the tooltip flips | `inv_<item>.png` |
| 2 | Inventory open, nothing hovered | 2–3 | | `inv_none_<n>.png` |
| 3 | Map screen | 3 per map | Zoomed out, zoomed in, panned | `map_<map>_<n>.png` |
| 4 | Negatives | ~5 | Gameplay HUD, menu, lobby | `neg_<what>.png` |
| 5 | Recording (optional) | 20–30 s | OBS: mouse sweeping across a full inventory | `rec_inventory.mkv` |

## Storage

Full-resolution PNGs are several MB each, so **don't commit the raw set**.
Keep it locally (e.g. `~/arclens-captures/`). A small, curated subset goes
into `crates/arclens-vision/tests/fixtures/` via Git LFS once that crate
exists.
