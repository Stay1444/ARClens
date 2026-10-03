# Findings: Dam Battlegrounds map video (2026-10-03)

**Source:** maintainer's recording of the in-raid map screen on Dam
Battlegrounds. 2560×1440, 19 s. It shows panning, zooming and hovering over
points of interest. Fixture frames are in
`crates/arclens-vision/tests/fixtures/map/`.

## What the map screen shows (verified)

- **Identifying the screen.** The top navigation has the **MAP** tab
  highlighted (a white pill). The footer reads "PAN · ZOOM · PLACE WAYPOINT".
- **Right panel, as plain text:**
  - **map name and raid time left**, e.g. "DAM BATTLEGROUNDS — 19:02";
  - the **active map condition** ("Matriarch");
  - the player name;
  - a legend: Cargo Elevator, Raider Hatch, ARC Probe, Baron Husk, Field Depot,
    Supply Drop, then loot categories (Nature, Industrial, Security, ARC,
    Mechanical, Commercial, Technological, Medical, Electrical).

  The OCR pipeline can read all of this, which answers "which map" and
  "which condition" without any image registration.
- **Left panel:** the active quests with their objectives, also plain text.
- **What the game already draws** (so ARClens shouldn't duplicate it):
  - extraction points with **timers** (cargo elevators "14:01", raider
    hatches);
  - POI outlines, red/yellow/white by loot value;
  - region labels zoomed out (VICTORY RIDGE, THE DAM, SWAMP, RED LAKES,
    FORMICAI HILLS);
  - **POI labels** zoomed in (Pale Apartments, Ben Welder's Sunroof,
    Old Battleground, South Swamp Outpost, Water Treatment Control, Primary
    Facility, Control Tower, Hydroponic Dome Complex, Pipeline Tower, …);
  - the player arrow (blue).
- **Rendering:**
  - the map is a top-down greyscale satellite-style image with **no
    rotation**;
  - zoom looks continuous (several levels between the shots);
  - zoomed in, the map shows *through* the semi-transparent side panels.
- **POI hover card:** a dark card with a photo, the POI name, "LOOT VALUE
  HIGH/MEDIUM/LOW" and its loot categories.

## How to place our markers: label anchoring

The view is pure **scale + translation**. Every POI label is text at a fixed
spot on the map, and at any zoom at least two are visible.

1. **Reference positions.** Each map gets a table of label anchors in ARClens
   map space (`[0,1]²`). These are built once per map with a calibration
   tool:
   - start from a fully zoomed-out frame;
   - then chain through overlapping zoomed frames: each new frame shares two
     or more already-known labels, so its transform is known, and its new
     labels get positions.
2. **At runtime, when the map is open:**
   - OCR the visible labels (white text with a dark outline: a different
     colour rule from tooltips, same recogniser);
   - match them to the table;
   - two or more matches give `Transform::from_two_points` (a least-squares
     fit for more points).
3. **Markers.** Our markers (containers, quest items, keycard rooms) are
   projected through that transform. The overlay only draws them inside the
   map viewport, so not over the side panels.

Why this beats feature matching:
- it reuses the OCR we already have;
- it's robust to the dark vignette, POI outlines and fog;
- every match is human-readable, which makes debugging easy.

Feature matching (ORB/AKAZE against a reference mosaic) stays the fallback
for views with fewer than two labels.

## First OCR experiment (2026-10-03)

- Ran ocrs' full pipeline (text detection + recognition) on
  `fixtures/map/dam_zoom_in.jpg` with
  `crates/arclens-vision/examples/read_map.rs`.
- **Read correctly or near-correctly:**
  - "DAM BATTLEGROUNDS - 18:55" and "Matriarch";
  - POI labels with screen positions: "Pale Apartments", "Ben Welder's
    Sunroof", "Old Battleground", "Rubv Residence", "Floodgates", "South
    Swamp Outpost", "Water Treatment Control" (the last two merged into one
    line).
- Errors are the usual character-level ones (`v` for `y`, dropped letters).
  Fuzzy-matching against a known label list absorbs them, just as with item
  names.
- **Too slow as-is: ~2 s per frame**, because generic text detection runs on
  the full 1440p screen.
- **Plan:**
  - find map labels the way we find tooltips, by colour: white text (≥ 225,
    low saturation) with a dark outline, inside the map viewport;
  - run only the recognition model on those crops (~30 ms each);
  - only do it when the map is open and the view changed (a cheap
    frame-difference check);
  - read the right-panel header once per map open, for the map name, time
    and condition.

## Marker data

- The game already shows extractions and their timers.
- What players want on top: loot containers, quest objectives, keycard and
  locked rooms, ARC spawns.
- Sources:
  - **MetaForge `game-map-data`.** Crowd-sourced, uses its own pixel
    coordinates, attribution required, terms unclear for an app. Ask them
    before using it.
  - **Our own hand-placed set**, in ARClens map space, stored in-repo as
    `data/markers/<map>.json`.

## Open questions

- Do POI labels keep the same screen size at every zoom (i.e. only their
  positions move)? It looks that way; it matters for OCR.
- Exact pan limits and the viewport rectangle. The side panels look
  semi-transparent when zoomed in. Should markers be hidden under them?
- The time-left readout ("19:02"): is it raid time? It counts down across
  the video (19:02 → 18:55 → …), so very likely yes.
