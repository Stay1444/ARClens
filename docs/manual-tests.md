# Manual tests

The open roadmap items that need a real desktop, the game or the
maintainer's accounts. Each one says what to do and what to send back. The
headless setup (`docs/development.md`) covers everything else.

## 1. Map panel clicks on KDE

Roadmap: "Verify clicks and typing in the map panel on KDE".

1. `cargo build -p arclens-overlay && ARCLENS_VISION=1 cargo run -p arclens`
2. In game, open the map. The marker panel appears on the left.
3. Click a category checkbox; type in the panel's search box.
4. Close the map. Check the game takes clicks again everywhere.

Send back: works / doesn't, and `RUST_LOG=arclens=debug` output if not.

## 2. Proton via XWayland

Roadmap: "Same test with Proton via XWayland". Same as the Wayland test
already done, without `PROTON_ENABLE_WAYLAND=1` in the launch options.

1. Start the game from Steam with default launch options.
2. Hover items in the stash, open the map, toggle interactive mode
   (Ctrl+Shift+I).

Send back: whether the overlay draws above the game in borderless and in
fullscreen, and whether the game keeps focus. The results go into
`docs/research/wayland-overlay.md`.

## 3. Packages on Fedora/KDE

Roadmap: "Verify both packages on Fedora/KDE".

1. Tag a release (`git tag v0.x.y && git push --tags`) or take the
   artifacts of the release workflow.
2. AppImage: `chmod +x ARClens-*.AppImage && ./ARClens-*.AppImage`.
3. Flatpak: `flatpak install --user ARClens-<version>-x86_64.flatpak && flatpak run io.github.Stay1444.ARClens`.
4. In each: the screen-share dialog appears on "Detect items", the
   global-shortcuts dialog appears on first start, the overlay starts.

Send back: which of the dialogs and the overlay worked in each package.

## 4. Flathub / COPR (optional)

The licence is in place (GPL-3.0-or-later); this needs the maintainer's
Flathub and Fedora accounts. `packaging/flathub/` already
builds offline in CI.

## 5. Stash scan

With game capture on and the overlay running:

1. Open the stash (Inventory tab) and wait a second: tags (KEEP, SELL,
   RECYCLE, LEARN) appear in the visible slots' top-left corners, inside
   the slots.
2. Scroll down step by step to the end: tags hide while the grid moves
   and come back on the new rows. In the app, Progress → Stash shows a new
   scan with the stash's slot count once the last row has been seen.
3. Hover a slot tagged with "?" so its tooltip shows, then move to another
   slot: the "?" goes away. Close and reopen the stash: still no "?" on it.
4. Above the stash, a bar says "Scroll down to scan your stash" with a
   count ("28 / 75") that grows as you scroll; at the end it turns green
   ("Stash scanned").
5. Pick another filter tab (augments, shields…): the bar asks for the
   first tab and stops counting; the shown slots still get tags. Back on
   the first tab, the count carries on where it was.
6. Report slots whose tag is for the wrong item (a screenshot with the
   tooltip shown helps).

## 6. Screenshots for fixtures

Full-resolution PNGs, game in the foreground, nothing over it (Spectacle's
"Full screen" or Steam's screenshot key). They go to
`crates/arclens-vision/tests/fixtures/frames/`.

- A project page of a project with **some phases finished** (roadmap:
  "Project phases").
- At **1920×1080**: a stash tooltip, a trader tooltip, the map at three
  zooms, the workshop overview, the logbook.
- If available, the same at **16:10** (2560×1600) or **21:9**
  (3440×1440).
- The workshop overview with a station **not yet built** (level 0).
