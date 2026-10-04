# Research: real map images behind the marker plot

_Researched: 2026-10-04. Result: `crates/arclens-data/data/map-image-transforms.json`._

Goal: draw the real map behind the Map tab's marker plot. Markers and labels
are in MetaForge **map space** (`MapPoint::new(lng, lat)`, y grows down).
The images come from **RaidTheory/arcraiders-data** (MIT, allowed by
[data-sources.md](data-sources.md)), checkout at commit `2a4abeb`.

## TL;DR

- Map space → image pixels is `image_px = scale · map + (tx, ty)` for every
  map: uniform scale, **no rotation, no y flip** (rotation measured
  0.00° on all registered frames; the flipped fit is off by > 1 000 px).
- Dam, Buried City and Blue Gate are **verified** against in-game recordings
  (rms 3–12 px at 8 000 px). Stella Montis is **verified** from the game's
  own labels baked into its images (rms 8–12 px). Spaceport and Riven Tides
  are **unverified estimates** (rms ~40–60 px) because we have no recording.

| Map (MetaForge id) | Image | scale | tx | ty | Residual (max-zoom px) | Status |
|---|---|---|---|---|---|---|
| `dam` | tiles `dam-battleground` | 1.28133 | −1091.6 | 507.5 | rms 2.8, p95 5.3, max 10 | verified, recording |
| `buried-city` | tiles `buried-city` | 1.41650 | −6129.6 | −2851.2 | rms 11.6, p95 13.4, max 23 | verified, recording |
| `blue-gate` | tiles `blue-gate` | 0.75130 | −1460.3 | 303.0 | rms 3.2, p95 5.9, max 14 | verified, recording |
| `stella-montis` (zlayers 1) | `stella_montis_upper.png` | 1.06814 | −2766.8 | −1430.0 | rms 12, max 19 | verified, baked-in labels |
| `stella-montis` (zlayers 2) | `stella_montis_lower.png` | 1.07661 | −1925.3 | −1133.1 | rms 7.8, max 12 | verified, baked-in labels |
| `spaceport` | tiles `the-spaceport` | 1.56668 | −2204.8 | 258.7 | rms ~41, max 83 | **unverified** |
| `riven-tides` | tiles `riven-tides` | 0.89341 | −2357.3 | −746.1 | rms ~62, max 80 | **unverified** |

For scale: one max-zoom pixel is 0.6–1.3 map units. A whole tiled map drawn
1 000 px wide is 1/8 of max zoom, so 12 px of error becomes 1.5 screen px.

## Tile scheme (verified 2026-10-04)

- `images/maps/<dir>/v2/{high,low}/{z}/{x}/{y}.webp`, Leaflet-style 512 px
  tiles, **x = column, y = row**. `riven-tides/v2/*/manifest.json` documents
  it (`"urlTemplate": "{z}/{x}/{y}.webp"`, `L.CRS.Simple`, coordinate space
  1000 × 1000, `[lat, lng] == [y, x]`); the other four dirs have no
  manifest but the same layout and tile sizes.
- Zoom `z` is a square image of **1000 · 2^z px**: z0 = 1 000 (2 × 2
  tiles), z1 = 2 000 (4 × 4), both under `v2/low` (WebP q55); z2 = 4 000
  (8 × 8) and **z3 = 8 000 (16 × 16), the full resolution**, under
  `v2/high` (WebP q80). Edge tiles are cropped (z3 last column/row is 320
  px, z2 416 px), so tiles never extend past the image.
- At zoom `z`, divide `scale`, `tx` and `ty` by `2^(3−z)`.
- The source was a 4 096 px square render (`manifest.json` `source`), so z3
  is upscaled ~2×. Content has a thin dark margin on the right and bottom.
- The images are the **same top-down render the in-game map uses**, without
  labels or icons (Riven Tides has its loot-zone outlines baked in). The
  in-game map matched them with thousands of SIFT inliers per frame.
- `legacy/*.png` (older single images, some actually JPEG) and
  `riven-tides/riven-tides.png` (869 × 448 thumbnail) were not used.
- Stella Montis has no tiles: `stella_montis_upper.png` (4 096 × 3 072) and
  `stella_montis_lower.png` (5 120 × 3 072) are **screenshots of the in-game
  map**, labels, icons and legend included. Upper holds the labels with
  MetaForge `zlayers` 1, lower those with 2 (Loading Bay, zlayers 3, is on
  both). The two images have different scales and offsets.

## Method

### Dam, Buried City, Blue Gate: registration against recordings

Inputs: in-game map recordings (2560 × 1440) with per-frame ground truth
`frame_px = s · map + (tx, ty)` from label tracking (rows with `agree ≥ 6`).

1. Stitch z2 tiles (4 000 px), downscale to 2 000 px, SIFT (OpenCV 5).
2. For ~40 frames per map, take the map area of the frame (x 700–2000,
   y 170–1260, clear of the side panels), SIFT-match to the stitched image
   (ratio test 0.75), fit a similarity with RANSAC. Results: 1 500–5 300
   inliers per frame, rotation 0.00°, frame→image scale constant per zoom
   (e.g. Dam 2.2989 z3 px per frame px).
3. Compose with the frame's truth: sample a 5 × 5 grid in the frame, map
   it to map space (inverse truth) and to image space (registration), and
   least-squares fit one uniform scale + translation over all frames. Drop
   frames whose rms is above 3× the median (mid-zoom-animation frames,
   where the truth lags), refit.

The residual is almost entirely the ground truth's own noise: the
registration fits each frame to ~0.05 px. Buried City's two recording
sessions disagree by ~22 px in y (low-zoom session −10, high-zoom
session +13 against the joint fit); each session alone fits to 1–8 px. We
can't tell which is right, so the joint fit splits the difference.

Visual check: Dam containers and other MetaForge markers
(`mapdata-real/dam.json`) land in buildings across the whole map (e.g. the
Pattern House cluster sits on the building). Buried City labels sit on the
buildings they name (Hospital, Library, Parking Garage). Blue Gate's
labels at the round structure sit centred on it.

### Stella Montis: labels baked into the image

The labels in the images are the game's, centre-anchored (matches
MetaForge's `center:!0` layer). For each label I located its white text box
in the full image (threshold min-channel > 215, low saturation, in a window
around a hand-picked position) and fitted the text centres to MetaForge's
label positions. Labels the threshold missed were held out. When the fitted
transform is drawn, they land on their text.

### Spaceport: hand-matched landmarks (unverified)

There is no recording, and the image has no text. I fitted the 13 place
labels whose building I could pick out: Water Towers (three round towers),
Jiangsu/Shipping/East Plains warehouses, Arrival/Departure buildings,
West/East/Little Hangar, Container Storage, Vehicle Maintenance,
Electrical Substation (grid yard), Service Buildings. A label's centre
isn't exactly a building's centre, so the ~41 px rms is a precision
estimate, not an error bound. Overlay check (north, west and south-east quadrants viewed): the other
labels also fall on or next to a plausible structure.

### Riven Tides: loot-zone outlines (unverified)

The image has the four loot-zone outlines (two red, two yellow) baked in.
Their hull centroids, fitted to the four loot-zone labels (Hotel Panorama
Azzurro, Port Authority Building, Stacking Yard, Customs House), give rms
~62 px. The labels are **top-left** anchored on this map, so a label is
offset from its zone's centre by about half its text size. The offset
varies with name length, which is why the residual is large. Overlay
check: Tennis Court lands on the courts, Wavebreaker on the breakwater,
Railyard on the tracks, Poolside at the hotel pool.

## What's unverified / open

- **Spaceport and Riven Tides**: estimates only. Re-run the recording
  method on one map recording each (any zoom, ≥ 6 agreeing labels). The
  pipeline is the same.
- **Buried City** ~±11 px session bias in y (see above).
- **Spaceport underground** (MetaForge tile layer "-1") and **Blue Gate
  underground**: RaidTheory has no images for them. Draw the surface image
  (or nothing) for markers on those layers.
- Stella Montis `zlayers` ↔ image pairing comes from which labels appear
  on which image. It holds for all 29 labels.
- RaidTheory may re-render maps (the folder is already `v2`). Pin the
  dataset version or re-check the transforms when `images/maps` changes.

## Side note: MetaForge's map space (unverified, not used)

The MetaForge map page bundle in the maintainer's HAR (the one
`map-labels.json` was extracted from) has per-map Leaflet configs. For Dam
(`tileSize` 256, `maxZoom` 5) the tiles requested at z3 (x 2–5, y 0–3) are
consistent with map units being pixels at their zoom 5, i.e. `lng / 4 / 256`
= tile x at z3. MetaForge's images are separate renders with different
framing, so this doesn't give the RaidTheory transform. Nothing is fetched
from MetaForge for this.
