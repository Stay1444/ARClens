# Development setup

## Prerequisites

Rust stable (pinned by `rust-toolchain.toml`; rustup installs it
automatically). The library crates need nothing else.

The two apps need Wayland, xkbcommon and Vulkan headers.

**Fedora:**

```sh
sudo dnf install wayland-devel libxkbcommon-devel vulkan-loader-devel \
    mesa-vulkan-drivers pipewire-devel clang-devel pkgconf-pkg-config
```

**Debian / Ubuntu:**

```sh
sudo apt install libwayland-dev libxkbcommon-dev libvulkan-dev \
    mesa-vulkan-drivers libpipewire-0.3-dev libclang-dev pkg-config
```

## Everyday commands

```sh
cargo test                                  # library crates only (default-members)
cargo test --workspace                      # everything
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

cargo run -p arclens                        # companion app (starts the overlay)
```

`arclens` launches `arclens-overlay` from the same directory (or `$PATH`) and
restarts it if it exits. The overlay is started with `--exit-with-app`, so it
quits when the app does. To run the overlay by hand while working on it:

```sh
ARCLENS_OVERLAY_AUTOSTART=0 cargo run -p arclens
cargo run -p arclens-overlay
```

Build the overlay first (`cargo build -p arclens-overlay`), or
`cargo run -p arclens` won't find it.

The overlay starts hidden. Toggle it with **Ctrl+Shift+O** (or the app's
"Show overlay" button). While shown, a small "ARClens" badge sits top-right.
Click an item in the app to put its card on the overlay.

Useful environment variables:

| Variable | Effect |
|---|---|
| `RUST_LOG=arclens=debug,arclens_overlay=debug` | Verbose logs |
| `ARCLENS_RAIDTHEORY_DIR=~/src/arcraiders-data` | Load the dataset from a local checkout; skip cache and network |
| `ARCLENS_OVERLAY_AUTOSTART=0` | Don't launch the overlay from the app |
| `ARCLENS_EVENTS_FILE=events.json` | Load a saved MetaForge `events-schedule` response instead of fetching it |
| `ARCLENS_MAP_DATA_DIR=dir` | Load markers from `dir/<map>.json` (saved MetaForge `game-map-data` responses, map ids as in `metaforge::MAPS`) instead of fetching |
| `ARCLENS_REPLAY_DIR=crates/arclens-vision/tests/fixtures/frames` | Feed recorded frames to item detection instead of capturing the screen |
| `ARCLENS_REPLAY_INTERVAL_MS=1500` | Time between replayed frames |
| `ARCLENS_OCR_MODEL=/path/text-recognition.rten` | Use a local OCR model instead of downloading it to the cache |
| `ARCLENS_OVERLAY_BIN=/path/to/arclens-overlay` | Overlay binary to launch |

## Item detection (vision)

- The app runs a background worker: frame → tooltip → OCR → catalogue match
  → `ShowHover` to the overlay, which draws the card beside the game's
  tooltip.
- On first use it downloads the ocrs recognition model (~10 MB) to
  `$XDG_CACHE_HOME/arclens/models/`.
- Turn it on with the **"Detect items"** button, or start with
  `ARCLENS_VISION=1`.
  - The first time, KDE asks which screen to share. Pick the one the game is
    on.
  - The choice is remembered: the restore token lives in
    `$XDG_STATE_HOME/arclens/`.
  - The overlay moves to the shared monitor. Hover positions are relative to
    the captured screen, so the app restarts the overlay there with
    `--monitor x,y,w,h` (matched to a Wayland output by logical position).
- Capture goes through the XDG ScreenCast portal and PipeWire
  (`crates/arclens-capture`):
  - 4 fps when idle, 10 fps for 3 s after a tooltip appears or changes;
  - the cursor is hidden from captures;
  - frames are converted only when the analyser is ready for one.
- Live capture is verified on KDE Plasma 6 (2026-10-03). It can't be tested
  in the headless Sway session: the portal handshake succeeds there, but
  Sway's software renderer can't provide screencopy frames.
- To try the whole path without capture, replay the fixture frames:

  ```sh
  ARCLENS_REPLAY_DIR=crates/arclens-vision/tests/fixtures/frames cargo run -p arclens
  ```

- To see the alignment, show the same frame full-screen behind the overlay
  (e.g. `imv -f frame.jpg`). This is how `docs/assets/hover-detected.png` was
  made.

## Global hotkeys

- These need `xdg-desktop-portal` with a GlobalShortcuts backend
  (`xdg-desktop-portal-kde` on Plasma).
- For the binding to persist, install the desktop file:

  ```sh
  install -Dm644 packaging/arclens.desktop ~/.local/share/applications/arclens.desktop
  ```

- Defaults are `Ctrl+Shift+O` (toggle overlay) and `Ctrl+Shift+I` (toggle
  interactive). Change them in System Settings → Shortcuts.

## Working on the overlay without the app

```sh
scripts/overlay-demo.sh          # socat replays scripts/overlay-demo.jsonl
```

Edit the JSONL file to try new messages. The format is
`arclens_ipc::ToOverlay` with `"type"` tags.

## Headless screenshots (CI, cloud agents)

The overlay and app can run inside a headless Sway with software Vulkan
(lavapipe). This is how the screenshots in `docs/assets/` were produced.

```sh
sudo apt install sway grim wtype socat mesa-vulkan-drivers
export XDG_RUNTIME_DIR=$(mktemp -d) WAYLAND_DISPLAY=wayland-1
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway &
cargo run -p arclens &           # needs ARCLENS_RAIDTHEORY_DIR if offline
wtype "rusted gear"              # keyboard input works
grim shot.png
```

`wtype` drops the first key of each invocation, so lead with a throwaway key
(`wtype -k Shift_L -k Return`).
Send modifier chords in the same invocation:
`wtype -k Shift_L -k Shift_L -M ctrl -k 2 -m ctrl` opens the Map tab
(Ctrl+1/2/3/4 switch between Items, Map, Events and Workshop). Enter in the search box opens the top result,
which is enough to reach the detail view without a mouse.

Known limitation: the headless seat has no pointer, so simulated clicks
(`swaymsg seat … cursor`, `wlrctl`) don't reach clients. Use keyboard input
or drive the overlay over IPC.

## Testing against the real game

- Run the game normally through Steam/Proton. Do **not** add launch options
  that load anything into the game (no MangoHud or other Vulkan layers when
  testing ARClens; they confuse anti-cheat triage).
- Test in both fullscreen and borderless, and with and without
  `PROTON_ENABLE_WAYLAND=1` where your Proton supports it.
- Record what you find in `docs/research/wayland-overlay.md`.
