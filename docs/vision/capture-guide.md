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

| # | Set | Count | Notes | Filename |
|---|---|---|---|---|
| 1 | Inventory, item hovered | ~40 | ~30 different items across all rarities and types (materials, weapons, mods, quick-use, keys). 3–4 items in different slots, including near the right/bottom edges (tooltip flips). Stacked items. Both hideout stash and in-raid backpack, plus a trader and a crafting screen | `inv_<item>_<where>.png` |
| 2 | Inventory open, nothing hovered | ~5 | Each tab or screen variant | `inv_none_<where>.png` |
| 3 | Map screen | ~6 per map | Fully zoomed out, fully zoomed in, 3–4 pans at mid zoom, player marker visible; one or two during a map condition | `map_<map>_<zoom>_<n>.png` |
| 4 | Negatives | ~10 | Gameplay HUD, main menu, lobby, loading screen, pause menu | `neg_<what>.png` |
| 5 | Recordings (optional) | 2 × 20–30 s | OBS, native resolution: mouse sweeping a full inventory; opening the map, then panning and zooming | `rec_<what>.mkv` |

## Storage

Full-resolution PNGs are several MB each, so **don't commit the raw set**.
Keep it locally (e.g. `~/arclens-captures/`). A small, curated subset goes
into `crates/arclens-vision/tests/fixtures/` via Git LFS once that crate
exists.
