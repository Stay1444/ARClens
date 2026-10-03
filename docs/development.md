# Development setup

## Prerequisites

Rust stable (pinned by `rust-toolchain.toml`; rustup installs it
automatically). The library crates need nothing else.

The two apps need Wayland, xkbcommon and Vulkan headers.

**Fedora:**

```sh
sudo dnf install wayland-devel libxkbcommon-devel vulkan-loader-devel \
    mesa-vulkan-drivers pkgconf-pkg-config openssl-devel
```

**Debian / Ubuntu:**

```sh
sudo apt install libwayland-dev libxkbcommon-dev libvulkan-dev \
    mesa-vulkan-drivers pkg-config
```

## Everyday commands

```sh
cargo test                                  # library crates only (default-members)
cargo test --workspace                      # everything
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

cargo run -p arclens                        # companion app
cargo run -p arclens-overlay                # overlay (start it after the app)
```

Useful environment variables:

| Variable | Effect |
|---|---|
| `RUST_LOG=arclens=debug,arclens_overlay=debug` | Verbose logs |
| `ARCLENS_RAIDTHEORY_DIR=~/src/arcraiders-data` | Load the dataset from a local checkout; skip cache and network |

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
(lavapipe). This is how `docs/assets/overlay-demo.png` was produced.

```sh
sudo apt install sway grim wtype socat mesa-vulkan-drivers
export XDG_RUNTIME_DIR=$(mktemp -d) WAYLAND_DISPLAY=wayland-1
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway &
cargo run -p arclens &           # needs ARCLENS_RAIDTHEORY_DIR if offline
wtype "rusted gear"              # keyboard input works
grim shot.png
```

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
