# Research: knowing what the game is showing

The overlay needs four signals:

1. Is the **inventory** open?
2. Which **item is hovered**?
3. Is the **map** open, and which map?
4. Where is the map view **panned and zoomed**, so markers line up?

This document compares every way to get them. The decision is at the bottom.

## Options

### A. Official API / game events: not available

- Embark has no public API (see [data-sources.md](data-sources.md)).
- Overwolf's Game Events Provider supports ARC Raiders, but only with
  `match_start`, `match_end`, `death`, `extraction` and `scene`
  (lobby / ingame / summary). It has no map, position or inventory data.
- It is Windows-only and works by injecting into the game process.

### B. Log files: weak, but free

- Path:
  `~/.steam/steam/steamapps/compatdata/1808500/pfx/drive_c/users/steamuser/AppData/Local/PioneerGame/Saved/Logs/PioneerGame.log`.
- Map or level loads *may* appear there (**unverified**; tail it during a
  raid to check).
- Even at best this answers "which map am I on", not 1, 2 or 4.
- In March 2026 the log leaked Discord DMs and tokens and was hot-fixed, so
  expect logging to stay minimal. Never upload or persist its contents.

### C. Reading game memory: rejected

- **Policy.** Embark allows overlays "only if they do not interfere with
  gameplay or offer an unfair advantage". It bans "third-party tools that
  affect gameplay" and offers no allow-list.
  ([anti-cheat FAQ](https://id.embark.games/arc-raiders/support/faq/167-common-anti-cheat-questions))
- **Detection doesn't care about intent.** Since May 2026 ARC Raiders uses
  **Denuvo Anti-Cheat**. Its "forbidden tools ARAV1011" kicks have already hit
  Linux players running ordinary tools. An external process holding a
  `ptrace` / `process_vm_readv` handle on the game is the textbook signal
  anti-cheats look for. "Read-only" is not a category they distinguish.
- **Under Proton specifically.** The anti-cheat runs inside Wine without a
  kernel driver, so host-side reads *may* be invisible to it. That is a bet
  on a gap that can close in any patch, with players' accounts as the stake.
- **Maintenance.** Offsets move every patch, and the binary is obfuscated.
  We'd be shipping a reverse-engineering project, not a companion app.
- **Bans observed.** Players report bans for HudSight, a crosshair overlay
  (2026). No bans are known for pure info overlays like MetaForge.

### D. Text OCR (what ARLO and MetaForge's scanner do): possible, not preferred

- Proven to work. ARLO uses Tesseract on a crop next to the cursor every
  300 ms while the inventory is open.
- Downsides: it's language-dependent, Tesseract is a heavy dependency, it's
  fragile with stylised fonts, and it only solves signal 2.

### E. Screen capture + computer vision: chosen

- We capture frames through the **xdg-desktop-portal ScreenCast** API
  (PipeWire), the same path OBS and Discord streaming use. The user approves
  it once, and nothing touches the game process.

| Signal | Technique | Difficulty |
|---|---|---|
| Inventory open | Template-match a fixed UI element (header or tab bar) on a downscaled frame | Easy |
| Map open | Same, with the map screen's chrome | Easy |
| Which map | Compare against reference map images (colour histogram, then feature match) | Easy–medium |
| Hovered item | Find the cream tooltip panel (colour threshold + largest rectangle), read its **name line** and fuzzy-match against the catalogue (see update below) | Easy–medium |
| Map pan/zoom | The map is top-down with no rotation, so view = scale + translation. Feature match (ORB/AKAZE + RANSAC) against the reference map, or phase correlation at the game's discrete zoom levels | Medium–hard |

**Cost.**
- Detection runs only when useful: at 2–5 fps on a downscaled frame or a
  region of interest, and only while a game window is focused.
- Icon matching on a ~64×64 crop costs microseconds.
- Map registration runs only while the map screen is open and the view
  changes.
- PipeWire can deliver DMA-BUF frames, which avoids copies.
- Estimated at a few percent of one core. **Measure before committing.**

**Risks.**
- UI changes in patches break templates. Keep templates as data files, not
  code, and version them per game build.
- The dataset's item icons are AI-upscaled renders, so we'll likely need to
  capture our own reference icons.
- Markers may trail the map by a frame or two while it is being dragged.
- HDR, colour filters and resolution scaling need normalisation.

## Update 2026-10-03: real footage changes the plan for signal 2

A stash recording (see `docs/vision/findings-2026-10-03-stash-video.md`)
shows the following:

- **Detecting the tooltip is easy.** It's an opaque cream panel, the
  brightest object on screen.
- **Its name is bold, dark, single-line uppercase text** that matches dataset
  names exactly.
- **It has no item icon**, so icon matching is the wrong tool. Reading the
  one name line and fuzzy-matching it against the ~580 catalogue names is
  simpler and more robust.

This narrow, closed-vocabulary text recognition is a different thing from
ARLO-style "OCR the screen". Everything else on this page stands.

## Decision

- **E (computer vision on portal screen capture)** is the long-term path.
- **B (logs)** is a cheap bonus signal for "current map".
- **D (OCR)** stays a fallback for item names only if icon matching proves
  unreliable.
- **C (memory reading) is out of scope.** This is a project rule: see
  `AGENTS.md`.
- **Prototype offline first.** Build `arclens-vision` against recorded
  screenshots and video in `crates/arclens-vision/tests/fixtures/`, with
  precision and recall tests, before wiring up live capture. See
  [ROADMAP.md](../ROADMAP.md).

## Update 2026-10-03: is the game running?

Capture now runs only while the game runs. The app scans `/proc/*/cmdline`
every 5 s for the game's executable (`PioneerGame*.exe` under Proton) or its
Steam install path (`steamapps/common/ARC Raiders/`). This is the process
list `ps` shows: no handle on the game is opened, nothing is read from its
memory, nothing is signalled, so rule 1 holds. Executable name
**unverified** on a live install. Inside a Flatpak sandbox the host's
processes are invisible, so there the user picks "Always".

## Progress screens (2026-10-04)

From the maintainer's 2560×1440 screenshots (fixtures `workshop_overview`,
`quest_*`, `logbook`, `project_*`):

- **Workshop overview:** eight tiles along the bottom: Scrappy, then
  Workbench, Gunsmith, Gear Bench, Utility, Medical, Explosives,
  Refiner. Each station tile shows its level as a roman numeral at the
  bottom right, read by counting strokes. RaidTheory gives the Workbench
  a max level of 0 although the game shows "I"; it isn't tracked
  (no item costs). **Verified** on one screenshot.
- **Quests:** the logbook (pause menu) and a trader's QUESTS tab both
  name quests in progress in bold white capitals; the screens are told
  apart by the outlined tab's width and position. A listed quest implies
  its prerequisites are done. **Verified** on four screenshots; title
  wrapping and other resolutions **unverified**.
- **Projects:** the page title sits where station titles do; phases are
  numbered circles, the highlighted one being the phase viewed. Not used
  yet (see ROADMAP).

## CPU budget per frame (2026-10-04)

Measured with `criterion` (`crates/arclens-vision/benches/frame.rs`):
`CARGO_INCREMENTAL=0 cargo bench -p arclens-vision --bench frame --
--warm-up-time 1 --measurement-time 3`. Frames are the 2560×1440 JPEG
fixtures, decoded once outside the measured loop; the "pan" motion frame is
`map/dam_zoom_mid` shifted by (40, 24) px. Release profile, single thread.
Code at commit `87180e7`.

Machine: a 4-vCPU cloud container (`nproc` = 4), `Intel(R) Xeon(R)
Processor @ 2.80GHz`. Shared, virtualised hardware: treat the numbers as
an order of magnitude, not a spec for a gaming PC (which should be faster).

Median per call:

| Check | Frame | Median |
|---|---|---|
| `find_panels` (tooltip detection) | `stash_osprey_ii` (tooltip) | 1.62 ms |
| `find_panels` | `raid_jolt_mine` (tooltip) | 1.40 ms |
| `find_panels` | `stash_none_1` (no tooltip) | 1.20 ms |
| `is_map_screen` | stash / map | 1.2 µs / 89 µs |
| `is_main_menu` | stash | 26.5 µs |
| `is_workshop_overview` | stash / overview | 0.3 µs / 26 µs |
| `quest_screen` | stash / quest | 346 µs / 415 µs |
| `station_header_box` | stash / project | 61.5 µs / 52.5 µs |
| all of the above in sequence | `stash_osprey_ii` | **2.09 ms** |
| `MotionTracker::track` | pan | 16.7 ms |
| `MotionTracker::track` | zoom (`dam_zoom_mid` → `dam_zoom_in`) | 113 ms |

Not measured: `Analyzer::analyze`. It needs `NameReader`, which loads the
OCR recognition model from a file (`NameReader::from_model_file`), and its
cost is dominated by OCR whenever a panel is found. Without a panel it is
`find_panels` plus `name_lines` on nothing, so close to the `find_panels`
row. Benchmarking OCR needs the model in the bench environment: a follow-up.

Conclusion against the 4 fps budget (250 ms per frame), **verified on this
machine only**:

- The cheap checks together take about 2.1 ms per frame: **under 1 % of the
  budget**, and at 4 fps about 8.4 ms of CPU per second, i.e. **~0.8 % of
  one core**. At the 10 fps "fast" pace after tooltip activity, ~2 %.
  `find_panels` is ~75 % of that, `quest_screen` most of the rest.
- Map tracking is the expensive part. The app samples the map at 20 fps
  (`MAP_INTERVAL` = 50 ms): a pan step at 16.7 ms is a third of one core at
  that pace, within the 50 ms interval. A zoom step (scale search) at
  113 ms exceeds the 50 ms interval and is ~45 % of even the 250 ms
  budget, so while the user zooms tracking runs at under 9 fps on this
  machine. If that shows up live, the scale search is the place to cut.
- OCR is outside these numbers and runs only when a panel is found.
