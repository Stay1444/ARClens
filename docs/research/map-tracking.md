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

## Our own overlay is in the capture (2026-10-03, field report)

On Blue Gate a fast zoom-out left the markers bunched up off the island.
The screen capture includes ARClens's overlay; its badges sit still for
a frame or more while the map moves under them, and hundreds of
high-contrast dots outvote a dim, zoomed-out map: the tracker saw "no
zoom" (×0.74 tracked over the zoom-out, ×0.32 real), so the markers
didn't move, so they kept looking still. A feedback loop the earlier
recordings (without the overlay) couldn't show.

Fix: the app tells the tracker where the overlay draws (badge centres
and area outlines, the last two updates), and the tracker fills those
pixels from around them in both frames before comparing and leaves
them out of the refinement (`Footprint`, `track_ignoring`). Filling
rather than zeroing matters: a hard-edged hole at the same place in
both frames is itself a still feature. A first attempt that masked
every unchanged pixel made Buried City worse and didn't fix Blue Gate.

On the user's Blue Gate recording (markers found by colour standing in
for the footprint), reads 330 ms late: p90 error 230 → 10 px, worst
1330 → 76 px (at the widest zoom, where the truth itself is ~50 px off).
Dam and Buried City are unchanged. A unit test paints a grid of badges
over a dimmed, zooming fixture: plain tracking is fooled, footprint-aware
tracking lands within 4 px.

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

## Widest zoom: region labels can't set the scale (2026-10-03)

Checked with `examples/map_label_probe.rs`: Dam's 10-label fit at f0081
carried back to f0070 by the tracker, then the region labels read there
compared with where their anchors land. Measured at f0070 (5 region
labels, top-left anchors):

| Scale | Residuals, RMS | Source |
|---|---|---|
| 0.186 | 29.6 px | tracker, carried 11 frames |
| 0.194 | 27.4 px | hand check (icon spacing ×2.10 to f0076) |
| 0.197 | 27.1 px | least squares over all 5 |
| 0.214 | 32.5 px | robust fit (4 agreeing): what the app used |

No scale fits them well. Swamp and Victory Ridge sit ~40 px from where
any view puts them, so their positions in the extracted data don't match
the game at this zoom (the game may lay region names out per zoom
level). The robust fit drops a different label depending on the frame,
so its scale jumps around.

Change: a fit backed by fewer than 5 labels (`anchors::STRONG_FIT`) no
longer replaces a tracked view's scale. The app keeps the tracked scale
and only moves the view to the labels' median offset (`anchors::recenter`),
unless the two scales differ by more than 30 %, which means tracking has
lost the zoom. At f0070 that puts the scale 4 % off instead of 10 %. A
map opened directly at the widest zoom has no tracked view yet and still
uses the label fit. **Partly verified:** one recording, and the hand
check is itself ±2 %.
- The capture is cursor-free (`CursorMode::Hidden`), and the game's own
  map UI is the only input; nothing touches the game process.

## Marker tooltips need the compositor's pointer (2026-10-03)

Field report: on KDE the markers track the map, but hovering them shows
no tooltip. The pointer comes from the ScreenCast stream's cursor
metadata (`CursorMode::Metadata`). KWin (source, master, read
2026-10-03: `plugins/screencast/screencaststream.cpp`,
`outputscreencastsource.cpp`, `cursor.cpp`) sets `spa_meta_cursor.id = 1`
with the position only when `Cursor::isOnOutput` holds. That is false
while the cursor is hidden (`Cursors::isCursorHidden()`) or its geometry
misses the output; otherwise it sends `id = 0` and no position. KWin does
queue cursor-only buffers when just the pointer moves.

**Resolved 2026-10-04:** the second cause. The maintainer reports the game
hides the system pointer and keeps it centred on the window (as games
usually do), drawing its own arrow. So the pointer is now found in the
frame instead: `arclens_vision::find_cursor` looks for the game's arrow
(white, tip top-left, ~19 × 27 px at 1440p, rows widening one pixel each
along a straight left edge, neutral rather than the cream of the map
icons). On the Dam, Buried City and Blue Gate recordings it finds the
arrow in 256 of 261 map frames with no false hits; the misses have the
arrow over white label text or snow, and the app keeps the last position
for a second. The capture's cursor metadata is no longer used.

The two candidates, as first written:
- a restore token saved before metadata was requested brings back a
  session without it. Fixed by storing tokens under a new name, so the
  portal asks once more;
- the game hides the system pointer on its map and draws its own. Then
  KWin sends no position, and tooltips can't work this way.

The capture now logs either "pointer position available" or, after 10 s,
a warning with the buffer and metadata counts and the last cursor id.

## Widest zoom: region labels re-measured (2026-10-04)

Follow-up to the section above. The region names' positions in
`map-labels.json` were wrong for where the game draws them at the widest
zoom; they are now measured in game, and a map opened at the widest zoom
gets the right scale from its labels.

**Independent view.** Instead of carrying a label fit with the tracker,
each frame was registered to the RaidTheory map image (z1, 2 048 px) with
SIFT + RANSAC (similarity; 1 200–2 400 inliers per frame, rotation
0.00°) and composed with `map-image-transforms.json`
([map-images.md](map-images.md)) to get map → frame. This uses no labels.
Checked against the label-fit truth on mid-zoom frames (agree ≥ 6): Dam
194 frames, worst corner of the map area median 1.9 px, p90 4.4 px, scale
within 0.4 %; Buried City 29 frames, median 3.6 px; Blue Gate 7 frames,
~10 px, scale within 1 % (the truth itself jitters by that much there).

Widest-zoom scales (2560 × 1440): **Dam 0.1858** (tracker said 0.186; the
earlier hand check, 0.194, was 4 % off), Buried City 0.2054, Blue Gate
0.1089.

**Measurement.** Labels read with `Analyzer::read_map_labels`
(`examples/map_label_dump.rs` dumps every read with its box), each text
box mapped back to map space through the frame's SIFT view. Frames used:
Dam f0001–f0070 (map opened at the widest zoom; region names show only
there), Buried City f0001–f0034 (opened at the widest zoom) and
f0085–f0159 (after zooming back out, scales 0.205–0.246), Blue Gate
f0208–f0264 (after a zoom-out, scales 0.109–0.126, with a pan).

Findings:

- **Region names sit at fixed map positions.** On Blue Gate and Buried
  City, where the scale varies by up to 20 % across the frames, a label's
  offset from its data point is constant in map units (standard
  deviation 1–5 map units, about 1 px; West Village's OCR box jitters
  more), not in pixels. So a corrected position per label
  is the right fix, not a pixel offset or another anchor convention.
- **No anchor convention explains the old positions.** Dam's text
  top-lefts sit (+9, +36), (+5, −20), (−6, −6), (+38, −40), (−4, −5) px
  from the old data points (Victory Ridge, The Dam, Red Lakes, Swamp,
  Formicai Hills); against the text centre it's no better. The old points
  are simply misplaced for these labels.
- **Region names show only at the widest zoom** (and while zooming out
  to it). They are never read at mid zoom in the three recordings, so the
  change can't affect mid-zoom fits.
- Buried City's Outskirts, New District and West Village were already
  right (0–5 px; West Village's OCR box jitters by ±13 map units). Only
  Old Town was off (−29, +17 px).

**Change** (`crates/arclens-data/data/map-labels.json`, each changed
label carries a `measured` note with its old position; anchors
unchanged: Dam top-left, Buried City and Blue Gate centre). Shifts in map
units (lng, lat) and in pixels at the widest zoom:

| Map | Label | Shift, map units | px at widest |
|---|---|---|---|
| dam | VICTORY RIDGE | −50.5, −193.6 | −9, −36 |
| dam | SWAMP | −204.3, +214.3 | −38, +40 |
| dam | THE DAM | −25.8, +105.8 | −5, +20 |
| dam | RED LAKES | +30.5, +34.0 | +6, +6 |
| dam | FORMICAI HILLS | +23.8, +27.7 | +4, +5 |
| buried-city | OLD TOWN | −142.7, +82.8 | −29, +17 |
| blue-gate | THE MOUNTAINS | −158.8, +7.5 | −17, +1 |
| blue-gate | GATE APPROACH | +310.1, +147.4 | +34, +16 |
| blue-gate | FARMLANDS | +465.6, +319.0 | +51, +35 |
| blue-gate | THE FOREST | +379.3, −299.1 | +41, −33 |

**Before/after** (`examples/map_truth.rs` over every frame of the three
recordings, scale compared with the SIFT view):

| Recording | Frames whose fit changed | Scale error before | after | Label residual RMS before → after |
|---|---|---|---|---|
| Dam (f0001–f0070) | 70 | 15.1 % (0.214 vs 0.186), 4 of 5 agree | 0.02 %, 5 of 5 | 35 → 0 px |
| Buried City | 106 | median 0.3 %, max 4.7 % | median 0.4 %, max 1.6 % | 14 → 2 px |
| Blue Gate | 58 | median 5.9 %, max 17.5 %, 3 of 4 | median 0.1 %, max 1.0 %, 4 of 4 | 38 → 0.3 px |

All other frames (mid and close zoom: 493 Dam, 161 Buried City, 206
Blue Gate) give exactly the same fit as before. Blue Gate f0207 (held
out of the measurement, mid-zoom-out at 0.126) now fits 0.1247 (was
0.104/2 labels); Buried City's two widest-zoom sessions give the same
Old Town position to 0.2 map units.

**Unverified / open:**

- Dam and Blue Gate are measured from one widest-zoom visit each, so the
  "after" numbers there are in-sample; only Buried City (two sessions)
  and Blue Gate f0207 are held out. Dam's region names never appeared at
  another scale, so for Dam "fixed in map units" is assumed from the
  other two maps.
- One resolution (2560 × 1440). If the game scales the map and the text
  differently at other resolutions or UI scales, the text's top-left
  (Dam) may move in map units. Centre-anchored maps are less sensitive.
- Spaceport, Riven Tides and Stella Montis region labels are not
  re-measured (no recordings).
- With 4 region names on Buried City and Blue Gate, a widest-zoom fit is
  still below `anchors::STRONG_FIT` (5), so the app keeps a tracked
  scale when it has one. Now that region fits are accurate this guard
  could be revisited; its doc comment still describes the old ~30 px
  disagreement.
