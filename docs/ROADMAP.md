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

- [ ] **Unmap when hidden.** Destroy and recreate the layer surface
      (`iced_layershell` daemon mode, `RemoveWindow` / `NewLayerShell`) so
      KWin regains direct scanout.
- [ ] **Interactive mode.** Swap the input region (`SetInputRegion`) to
      cover only the overlay's panels, and add a search box in the overlay
      (`ToApp::Search` already exists).
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
- [ ] Packaging: `.desktop` install, Fedora COPR or Flatpak (the Flatpak
      needs the portal permissions).

## M2: Maps and timers

- [ ] Map images, with a per-map `Transform` from source pixels into map
      space.
- [ ] Marker sets:
  - start with a curated in-repo set (`data/markers/<map>.json`, our own,
    hand-placed);
  - optionally add a MetaForge `game-map-data` provider with attribution and
    caching (check their terms first).
- [ ] Map view in the companion app (pan/zoom, category filters).
- [ ] **Manual calibration.** The user opens the in-game map and clicks two
      landmarks; `Transform::from_two_points` gives map→screen. Markers then
      render while the view doesn't move. This is the stepping stone to M3.
- [ ] Event timers: a MetaForge `events-schedule` provider (cached,
      attributed). Verify the per-region rotation claim first.
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
  - [ ] OCR model download + cache in the app; skip OCR when the crop is
        unchanged; pick the OCR engine (`ocrs` vs
        Tesseract) by benchmarking on crops from the stash video;
  - [ ] read the tooltip footer value (weapons differ from the dataset);
  - [ ] place the overlay card next to the detected tooltip;
  - [ ] map screen: read map name, raid time and condition from the right
        panel (text, same OCR);
  - [ ] map registration → `Transform` by **label anchoring** (OCR the POI
        labels, fit scale + translation; see
        `docs/vision/findings-2026-10-03-map-video.md`); calibration tool
        to build per-map label tables;
- [ ] Benchmarks (`criterion`): a CPU budget per frame at 4 fps.
- [ ] Live capture: ScreenCast portal (`ashpd`) plus the PipeWire stream
      (`pipewire` crate) with cursor metadata. Run only while the game is
      focused.
- [ ] Wire the detectors to the overlay: hovered item → `ShowItem`, map open →
      `ShowMarkers` with the live transform.

## Later

- X11 and Windows overlay backends.
- Localisation (the dataset already ships ~20 languages).
- Crafting and workshop planner, and quest tracker in the companion window.
