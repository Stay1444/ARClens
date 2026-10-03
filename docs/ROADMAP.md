# Roadmap

Each step should land as its own commit (or a small series) on `master` with
green CI.

## ✅ M0: Skeleton (done)

- Workspace, lints, CI, docs, `AGENTS.md`.
- `arclens-core`: items, advice, maps, transforms.
- `arclens-data`: RaidTheory loader, download, cache, fuzzy search.
- `arclens-ipc`: protocol and handshake.
- `arclens-hotkeys`: GlobalShortcuts portal.
- `arclens`: companion window (search, detail, overlay control).
- `arclens-overlay`: OVERLAY-layer, click-through, item card and markers,
  status badge while shown.
  Verified rendering under headless Sway.

## M1: Overlay you can use in a raid

- [x] **Unmap when hidden.** The overlay runs in `iced_layershell` daemon
      mode and only has a layer surface while something is drawn, so KWin can
      scan the game out directly the rest of the time.
- [x] **Input region.** `SetInputRegion` makes the whole surface
      clickable in interactive mode, only the map panel while the in-game
      map is open, and nothing otherwise. Keyboard is `OnDemand` while any
      region is clickable, so the panel's search box takes typing after a
      click.
- [ ] Verify clicks and typing in the map panel on KDE (the headless seat
      has no pointer).
- [ ] Quick item search in the overlay (`ToApp::Search` already exists).
- [ ] Overlay position and scale settings (anchor corner, per-monitor
      `StartMode::TargetScreen`).
- [ ] Settings file (`$XDG_CONFIG_HOME/arclens/config.toml`): data refresh
      interval, overlay corner, opacity.
- [x] Test on KDE Plasma 6 Wayland over ARC Raiders with
      `PROTON_ENABLE_WAYLAND=1` (works, 2026-10-03).
- [ ] Same test with Proton via XWayland (the default). Record the results in
      `docs/research/wayland-overlay.md`.
- [x] Start the overlay automatically from the app, and restart it if it
      exits.
- [x] Packaging: AppImage and Flatpak bundle built by the release
      workflow on `v*` tags (`docs/packaging.md`).
- [ ] Verify both packages on Fedora/KDE (portals inside the Flatpak).
- [ ] Optional: Fedora COPR, Flathub (needs vendored crates).

## M1.5: Smarter advice (done 2026-10-03)

- [x] Workshop progress editor in the app (`~/.config/arclens/progress.json`).
- [x] KEEP only for requirements still ahead of you; workshop levels you've
      built stop counting.
- [x] RECYCLE for parts when they feed a remaining upgrade **and** are worth
      ≥ 60 % of the sell price; otherwise SELL with a "parts would help" hint.
      Stash contents are unknown, so we can't count missing parts.
- [x] "Used to craft" for materials (recipes and weapon tier upgrades).
- [ ] Track quests and projects too (they still always count as needed).

## M2: Maps and timers

- [ ] Map images, with a per-map `Transform` from source pixels into map
      space.
- [x] MetaForge `game-map-data` provider (attributed, cached for a day,
      offline fallback). Tolerant of shape drift: one bad record is dropped,
      not the response.
- [ ] Check the provider against a live response; confirm the category
      names and the `y = -lat` orientation.
- [x] **Map** tab in the companion app: map picker, marker search, category
      and subcategory toggles with icons and counts, show/hide all, and an
      icon plot with place names and hover tooltips
      (`docs/assets/map-tab.png`, synthetic markers). Derived data and the
      plot layer are cached and rebuilt only when markers, map, search or
      filter change.
- [x] Marker icons: our own SVG glyphs (`arclens-ui/assets/markers`),
      picked by keyword from subcategory, then category. The filter is saved, and the overlay will use the
      same one.
- [ ] Map image behind the plot, once its alignment with MetaForge
      coordinates is known; pan/zoom.
- [ ] **Manual calibration.** The user opens the in-game map and clicks two
      landmarks; `Transform::from_two_points` gives map→screen. Markers then
      render while the view doesn't move. This is the stepping stone to M3.
- [x] Event timers: a MetaForge `events-schedule` provider (cached for
      30 min, offline fallback, attributed) and an **Events** tab in the
      app: active now, next per map, schedule per condition in local time
      (`docs/assets/events.png`).
- [ ] Verify the per-region rotation claim against the live feed and the
      game; add a region selector if MetaForge exposes one.
- [x] Condition icons on the Events tab: MetaForge's per-event icon, else
      RaidTheory's per-type icon; downloaded once, decoded once; initials
      while loading or when missing.
- (Dropped 2026-10-03: conditions in the overlay. The maintainer wants them
  in the app only; the in-game panel is just the marker filter.)
- [ ] Optional: tail `PioneerGame.log` for the current map. Never persist
      its contents.

## M3: Vision (see `docs/research/game-state-detection.md`)

- [ ] **Collect fixtures** (see `docs/vision/capture-guide.md`). A stash hover
      video is in; findings: `docs/vision/findings-2026-10-03-stash-video.md`. Screenshots and short recordings at 1080p and
      1440p of the inventory (with tooltips), the map screen at several
      zooms and pans on each map, and the HUD. Store them under
      `crates/arclens-vision/tests/fixtures/` (git LFS if large).
- [ ] `arclens-vision` offline detectors with precision/recall tests:
  - [ ] inventory-open and map-open template matchers;
  - [ ] which-map classifier;
  - [x] tooltip detector (cream panel → rectangle → name line), golden
        tests on 18 frames from stash, raid and trader;
  - [x] name-line reader (ocrs recognition model, Roman-numeral stroke
        count) and catalogue match: 17/17 fixture names exact;
  - [x] OCR model download + cache in the app; skip OCR when the crop is
        unchanged (`Analyzer`);
  - [x] app vision worker + replay frame source; overlay draws the card
        beside the game tooltip (`ShowHover`, protocol v2), verified
        headlessly on stash and trader frames;
  - [ ] avoid covering other panels when placing (trader purchase panel); pick the OCR engine (`ocrs` vs
        Tesseract) by benchmarking on crops from the stash video;
  - [x] read the tooltip footer: real sell value in the menu, raid detection
        (no value cell) → salvage outputs and SALVAGE wording in raid;
  - [x] map screen: read map name, raid time and condition from the right
        panel (`read_map_header`, ~90 ms, 0.06 ms when the map is closed);
  - [x] map-open detector: outlined "MAP" tab + bright panel title, no
        OCR (`is_map_screen`); passes on all 23 fixture frames (Dam at
        1440p only so far). The header is re-read every 5 s while open;
        the title picks the map (`metaforge::map_for_title`);
  - [x] overlay **map panel** while the map is open: the marker filter
        with icons (search, per-category and per-subcategory toggles,
        show/hide all) shared with the app's Map tab (`ShowMapPanel`,
        protocol v5). Verified headlessly by replaying the Dam frames
        (`docs/assets/overlay-map-panel.png`, `overlay-map-panel-open.png`);
  - [x] map label finder: white text with a dark outline inside the
        viewport, quest-panel text rejected by its flat background; ~15 ms
        per 1440p frame. Recognition-only OCR reads all 8 labels of
        `dam_zoom_mid` exactly in ~340 ms (release), vs ~2 s for the full
        ocrs pipeline (`find_map_labels`, golden + OCR tests);
  - [x] robust pan/zoom fit from label matches
        (`Transform::fit_uniform_robust`: best pair by agreement, then
        least squares on the agreeing ones);
  - [ ] map registration → `Transform` by **label anchoring** (OCR the POI
        labels, fit scale + translation; see
        `docs/vision/findings-2026-10-03-map-video.md`); calibration tool
        to build per-map label tables;
- [x] Debounce repeated `Hover` events for the same item (≤ 8 px jitter).
- [x] Session cache of name + value crops → reading, so re-hovers skip OCR.
      (A cross-session cache is possible but fingerprints are fragile; not
      worth it yet.)
- [x] Adaptive capture: 10 fps for 3 s after tooltip activity, 4 fps idle.
- [ ] Benchmarks (`criterion`): a CPU budget per frame at 4 fps.
- [x] Live capture: ScreenCast portal (`ashpd`) plus the PipeWire stream
      (`crates/arclens-capture`). Opt-in toggle, restore token persisted,
      ≤ 5 fps, cursor hidden. **Works on KDE (2026-10-03 field test).**
- [ ] Pause capture when the game isn't focused (no direct signal on
      Wayland; maybe pause when no ARC Raiders UI has been seen for N
      seconds).
- [x] Wire the hover detector to the overlay (`ShowHover`).
- [ ] Map open → `ShowMarkers` with the live transform.

## Later

- X11 and Windows overlay backends.
- Localisation (the dataset already ships ~20 languages).
- Crafting and workshop planner, and quest tracker in the companion window.
