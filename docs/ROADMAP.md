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
- [x] **Quick item search in the overlay** (2026-10-03): in interactive
      mode a search box sits at the top of the screen and takes the
      keyboard (exclusive on Linux until interactive mode is toggled off).
      The app answers each query with its top 8 matches and icons
      (`SearchResults`, protocol v12). Enter or a click shows the item's
      card (`PickItem`), and the overlay is shown if it was hidden
      (`docs/assets/overlay-search.png`, headless with wtype). Unverified
      on KDE and Windows.
- [x] **Overlay size and card corner** (2026-10-03): a Settings tab in the
      app (`docs/assets/settings.png`) sets the overlay's scale (80–150 %,
      applied as iced's scale factor, input regions scaled to match) and
      the corner of the pinned item card. Saved in `settings.json` and sent
      on connect and on change (`Configure`, protocol v13). The overlay
      already opens on the captured monitor. Verified headlessly at 125 %;
      **unverified** on Windows, where iced reports sizes after the scale.
- [ ] More settings: data refresh interval, overlay opacity.
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

- [x] **Home tab** (2026-10-03), where the app opens: game / capture /
      overlay status, item lookup, conditions running now and next, maps
      (last one seen in game, its condition and preset), workshop progress,
      in-game tips (`docs/assets/home.png`). Ctrl+1…5 switch tabs.
- [x] **App icon** (lens over a map pin) and app id
      `io.github.Stay1444.ARClens`. KDE on Wayland takes a window's icon
      from the desktop file matching its app id, so a plain build writes
      that file (icon by absolute path: KDE caches theme lookups) and the
      icon to `~/.local/share` on start (not in Flatpak
      or AppImage; `ARCLENS_NO_DESKTOP_ENTRY=1` turns it off).

## M1.5: Smarter advice (done 2026-10-03)

- [x] Workshop progress editor in the app (`~/.config/arclens/progress.json`).
- [x] KEEP only for requirements still ahead of you; workshop levels you've
      built stop counting.
- [x] RECYCLE for parts when they feed a remaining upgrade **and** are worth
      ≥ 60 % of the sell price; otherwise SELL with a "parts would help" hint.
      Stash contents are unknown, so we can't count missing parts.
- [x] "Used to craft" for materials (recipes and weapon tier upgrades).
- [x] **Progress tab** (2026-10-03, was "Workshop"): sections of game
      progress, the workshop first (`docs/assets/progress.png`). Workshop
      levels fill in from the game: opening a station shows
      `GUNSMITH — LEVEL 02` in the top bar, which is read and matched to the
      catalogue station (`arclens_vision::read_station_header`; region
      measured on one screenshot, **unverified** elsewhere).
- [x] **Main-menu card** (2026-10-03): on the game's main menu (its
      yellow PLAY button plus the outlined PLAY tab, no OCR;
      `arclens_vision::is_main_menu`), the overlay shows a card under the
      game's Quests box: conditions running now and next with countdowns,
      and workshop progress (`docs/assets/overlay-menu-card.png`, protocol
      v10). Regions measured on one screenshot, **unverified** elsewhere;
      verified headlessly on a synthetic frame and against all fixtures.
- [ ] Workshop overview tiles (roman numerals under each station) for
      autofill without opening each station; needs fixture frames.
- [x] **Quests and projects on the Progress page** (2026-10-03): tick
      finished quests, grouped per trader in chain order (ticking one ticks
      the quests before it, unticking clears the ones after), and step
      through delivered project phases, expeditions included
      (`docs/assets/progress-quests.png`). Items asked for by finished
      quests or phases stop counting as reasons to keep. Requirements now
      carry the quest or project id (catalogue schema 4).
- [ ] Autofill quests and project phases from the game's screens (needs
      fixture frames of the quest log and project pages).

## M2: Maps and timers

- [ ] Map images, with a per-map `Transform` from source pixels into map
      space.
- [x] MetaForge `game-map-data` provider (attributed, cached for a day,
      offline fallback). Tolerant of shape drift: one bad record is dropped,
      not the response.
- [x] Provider checked against a live response (2026-10-03): categories as
      listed in data-sources.md, `y = lat`.
- [x] **Map** tab in the companion app: map picker, marker search, category
      and subcategory toggles with icons and counts, show/hide all, and an
      icon plot with place names and hover tooltips
      (`docs/assets/map-tab.png`, synthetic markers). Derived data and the
      plot layer are cached and rebuilt only when markers, map, search or
      filter change.
- [x] Marker icons: our own SVG glyphs (`arclens-ui/assets/markers`),
      picked by keyword from subcategory, then category.
- [x] **Areas**: dense same-kind groups (spawn candidates such as raider
      caches, lockers in a building) drawn as one shaded outline with a
      count instead of dozens of icons (`arclens_core::areas`, DBSCAN per
      kind, radius 45 / min 5 tuned on real Dam data;
      `docs/assets/map-areas.png`). Same rendering in the overlay. The filter is saved, and the overlay will use the
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
- [x] Region: the live schedule says `"region":"europe"`, so rotations are
      per region. The Events tab asks for the server region on first launch
      (pills, saved to `settings.json`), requests `?region=<id>` and warns
      if the response is for another region.
- [x] **Condition-aware markers** (2026-10-03): markers carry the
      conditions they exist in (`eventConditionMask`, a per-map bit set,
      see data-sources.md). The condition read from the in-game map panel
      (or picked on the Map tab) leaves out markers of other conditions,
      e.g. hurricane caches outside Hurricane.
- [x] **Map presets**: named marker selections per map and/or condition
      (`arclens_core::presets`, defaults in `arclens-data/data/presets.json`:
      Everything, Loot run, Ways out, ARC threats, Quests, Gathering, and one
      per condition such as "First Wave caches" for Hurricane). The current
      map and condition pick one (the player's last choice there, else the
      best fit); switch in game with the panel's ◂ ▸ arrows (protocol v8),
      or on the Map tab, where presets can also be saved as new (scoped to
      the map or condition), updated from the toggles, deleted, or reset to
      the shipped default (`~/.config/arclens/presets.json`).
- [x] **Marker tooltips in game** (2026-10-03): pointing at a marker or
      area on the in-game map shows its icon, name and a line of info
      (area count, "not all are there every raid"). The pointer comes from
      the capture, never the game: PipeWire cursor metadata on Linux, the
      OS cursor position on Windows (protocol v11, `Pointer`;
      `docs/assets/overlay-tooltip.png`). Verified headlessly with a
      replayed cursor; **unverified** on a live KDE session.
- [ ] Confirm the region query parameter and ids (only `europe` seen so
      far).
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
  - [x] map registration by **label anchoring**: labels read whenever the
        view changes (≤ 4/s; OCR cached by crop content, so panning
        re-reads nothing), matched to the source's named markers
        (`arclens_data::anchors`), robust fit. No calibration tool needed
        if MetaForge's label markers carry the in-game names;
  - [x] label table: the game's place names from MetaForge's map page
        (one-off extraction, maintainer's decision), with per-layer anchor
        (text centre or top-left). Fit in frame pixels; labels agree within
        0–3 px on Dam and Buried City, game icons land within ~12 px
        (2560×1440);
  - [x] map-screen check hardened (both ends of the MAP tab outline, dark
        panel behind the title): the raid compass plus bright foliage
        fooled it (field report). Quest panel detected; markers are
        clipped right of it;
- [x] Debounce repeated `Hover` events for the same item (≤ 8 px jitter).
- [x] Session cache of name + value crops → reading, so re-hovers skip OCR.
      (A cross-session cache is possible but fingerprints are fragile; not
      worth it yet.)
- [x] Adaptive capture: 10 fps for 3 s after tooltip activity, 4 fps idle.
- [ ] Benchmarks (`criterion`): a CPU budget per frame at 4 fps.
- [x] Live capture: ScreenCast portal (`ashpd`) plus the PipeWire stream
      (`crates/arclens-capture`). Opt-in toggle, restore token persisted,
      ≤ 5 fps, cursor hidden. **Works on KDE (2026-10-03 field test).**
- [x] Capture only while the game runs: "Game capture" mode Auto (default)
      starts it when an ARC Raiders process appears in `/proc` and stops it
      when it exits; also Always / Off. Stopping now really ends the
      portal session and the PipeWire thread (each toggle used to leak a
      screencast session, reported 2026-10-03).
- [ ] Pause while the game isn't focused (no portable focus signal on
      Wayland; a KWin script could provide one).
- [x] Wire the hover detector to the overlay (`ShowHover`).
- [x] Map open → `ShowMarkers` with the live transform (map →
      normalised screen, protocol v6), markers inside the viewport only,
      drawn as cached icon badges. End-to-end on `dam_zoom_mid` with
      synthetic marker data: 8/8 labels agree and the badges land on the
      game's own icons (`docs/assets/overlay-map-markers.png`).
- [x] **Pan/zoom tracking** (2026-10-03): markers follow the map at
      capture rate (20 fps) between label reads: phase correlation with a
      zoom search, outvoting the pointer-following place card, then
      Gauss–Newton refinement. Measured on two recordings: while the map
      moves, markers sit 2.3 px (median) from where they belong, versus
      331 px before. Label OCR moved to its own thread; overlay gets
      `MoveMarkers` (protocol v9). See `docs/research/map-tracking.md`.
- [ ] Fix the label fit at the widest zoom (~10 % scale error on Dam with
      only region labels).
- [x] Map screen state is sticky: an unreadable title keeps the last
      recognised map, and the map must be gone for 1 s before it counts as
      closed (field report 2026-10-03: it flipped to "Unknown map" while
      panning).
- [x] The overlay takes keyboard focus only while the pointer is over its
      map panel (or in interactive mode); before, opening the map handed
      it focus and the game lost focus and audio (field report).
- [x] Hover card goes on the side of the tooltip *away* from the hovered
      item: slot outline detected beside the tooltip, else the game's
      placement rule (`item_side`, golden test on 9 frames; field report:
      the card covered the item).
- [x] A spawned overlay exits when its app's stdin pipe closes (any app
      death, SIGKILL included), so an orphan can't attach to the next app.

## Later

- X11 and Windows overlay backends.
- Localisation (the dataset already ships ~20 languages).
- Crafting and workshop planner, and quest tracker in the companion window.
