# Following the in-game map between label reads (2026-10-03)

Field report: markers lagged badly while panning or zooming, because the
view was only known after a label read (OCR, ~3 per second plus ~300 ms of
work). This note covers how the overlay now follows the map at capture
rate, and how it was measured.

## Method

`arclens_vision::MotionTracker` estimates how the map moved between two
consecutive frames, as a similarity (zoom + shift; the map never rotates):

1. Downsample the map area (`TRACK_REGION`: right of the quest panel, left
   of the legend) to a 256×256 grey thumbnail, zero mean.
2. **Phase correlation** (FFT, Hann window) gives the shift; the zoom is a
   1-D search over candidate scales (warp the previous thumbnail about its
   centre, keep the sharpest peak). Pans cost three correlations (scale 1
   and ±1 %); a zoom walks in 4 % steps, falls back to scanning ±50 % when
   nothing is sure, then refines by halving.
3. The top three peaks compete: the game's place card follows the pointer,
   so a second strong peak is "something on top of the map". The shift
   that most of an 8×8 grid of blocks agrees with (local correlation > 0.7)
   wins.
4. **Gauss–Newton refinement** of zoom, shift and a brightness offset, with
   Huber weights (labels and icons keep their pixel size while the map
   zooms, so they don't fit the model).

The app keeps the view of the last label read and composes the motion
since that read's frame. Label reads run on their own thread
(`arclens-labels`), so tracking never waits for OCR. A read whose names
don't match carries the view forward by the motion between the two reads.
The overlay gets the marker set once (`ShowMarkers`, with the viewport
clip) and then only `MoveMarkers { transform }` (protocol v9). Capture runs
at 20 fps while the map is open.

No learning is involved: the motion is measured from the picture, so
mouse sensitivity and screen size don't matter.

## Data

Two recordings by the maintainer (2560×1440, 60 fps), decoded at 30 fps:
Dam Battlegrounds (19 s: zoom from fully out, pans, the place card) and
Buried City (9 s, zoom). Ground truth for each frame is the label-anchor
fit (`crates/arclens-data/examples/map_truth.rs`); only frames with ≥ 5
agreeing labels count, as fits on fewer labels can be off (below).

Tools: `examples/map_track_eval.rs` (frame-to-frame accuracy) and
`examples/map_track_sim.rs` (what the overlay shows: label reads arrive
`LATENCY` frames late; renders frames with both views drawn).

## Results (release build)

Frame-to-frame error, worst point of the viewport, frames where the map
moves:

| Clip, rate | median | p90 | max |
|---|---|---|---|
| Dam, 30 fps | 1.4 px | 5.2 px | 8.7 px |
| Dam, 10 fps | 1.7 px | 4.2 px | 9.4 px |
| Buried City, 30 fps | 2.0 px | 3.9 px | 16.6 px |

Part of this is the truth's own noise: its scale jitters by ±0.7 % during
pure pans (≈ 5 px at the viewport's edge).

What the overlay shows while the map moves (reads arrive 330 ms late):

| Clip | labels only (before) | labels + tracking (now) |
|---|---|---|
| Dam | median 331 px, max 1126 px | median 2.3 px, max 10.7 px |
| Buried City | median 212 px, max 581 px | median 8.2 px, max 17.5 px |

Same at 15 fps and 10 fps (Dam median 2.3 / 2.2 px). Time per frame:
~15 ms median, ~110 ms worst (a big zoom step scans all scales).

## Findings along the way

- The game zooms about the pointer, up to ×1.3 per frame at 30 fps. Labels
  and icons keep their size; labels cross-fade between layers.
- The place card (photo + loot value) follows the pointer: the first
  version tracked it instead of the map (105 px error); fixed by step 3.
- **Fully zoomed out, the label fit is unreliable** (4 region labels): on
  Dam it put the zoom ~10 % off. Checked by hand: two icons are 82.9 px
  apart at f0070 and 174.4 px at f0076, ×2.10; the tracker said ×2.13, the
  fits ×1.90. Markers at the widest zoom may sit off until this is fixed
  (the region-label anchors need checking).
- The capture is cursor-free (`CursorMode::Hidden`), and the game's own
  map UI is the only input; nothing touches the game process.
