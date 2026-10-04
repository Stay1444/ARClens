# Architecture overview

## Processes

```
┌──────────────────────────── arclens (companion app) ────────────────────────────┐
│  iced window (winit)                                                            │
│  ├─ data:     Catalog ← DiskCache ← RaidTheory download   (arclens-data)        │
│  ├─ hotkeys:  portal (Linux) / RegisterHotKey (Windows)   (arclens-hotkeys)     │
│  ├─ vision:   screen capture → detect → OCR → match       (capture + vision)    │
│  └─ IPC server  Unix socket / named pipe                  (arclens-ipc)         │
└──────────────────────────────────────┬──────────────────────────────────────────┘
                                       │ newline-delimited JSON (ToOverlay / ToApp)
┌──────────────────────────────────────▼──────────────────────────────────────────┐
│  arclens-overlay: layer-shell (Linux) / topmost window (Windows), wgpu only     │
│  Renders item cards and map markers. Holds no game logic.                       │
└─────────────────────────────────────────────────────────────────────────────────┘

        The game process is never opened, read, injected into or hooked.
```

**Why two processes?**
- **Portability.** The companion window is plain iced/winit and runs on any
  desktop. The overlay is the platform-specific piece (layer-shell; GNOME has
  none), so other backends can replace it.
- **Isolation.** The overlay can crash, restart or be killed (which also
  frees the compositor's direct-scanout path) without losing app state.
- **A dumb renderer.** Detection and data work stay out of the overlay's
  render loop.

The app is the single source of truth. On (re)connect it pushes the full
overlay state (visibility, interactivity, current item).

## Crates

| Crate | Kind | Responsibility | May depend on |
|---|---|---|---|
| `arclens-core` | lib | Domain types (`Item`, `Marker`, `Transform`), loot advice. **Pure: no I/O, async or UI** | serde |
| `arclens-data` | lib | Providers (RaidTheory), download, disk cache, icon cache, fuzzy search | core |
| `arclens-ipc` | lib | Wire protocol, framing, version handshake, `Endpoint` (Unix socket / named pipe) | core, tokio |
| `arclens-hotkeys` | lib | Global hotkeys, stable action ids (portal / `RegisterHotKey`) | ashpd / global-hotkey |
| `arclens-ui` | lib | Design tokens, the shared item card and game names in the interface language, used by app and overlay | core, i18n, iced |
| `arclens-i18n` | lib | Interface translations: Fluent messages per language, the current language, `t!` | fluent-bundle, sys-locale |
| `arclens-vision` | lib | Frame → tooltip panels → name lines → OCR text (`Analyzer`); map labels, header, pan/zoom tracking | image, ocrs, rustfft |
| `arclens-capture` | lib | Screen → RGB frames at a set pace (portal + PipeWire / Graphics Capture) | ashpd, pipewire / windows-capture |
| `arclens` | bin | Companion app: owns state, wires everything | all libs, iced |
| `arclens-overlay` | bin | Overlay renderer: platform-free core + `shell` per OS | core, ipc, iced (+ iced_layershell on Linux) |

## Platform backends

Each platform-dependent area keeps one API; the OS picks one backend
module with a single `#[cfg]`:

| Area | API | Linux | Windows |
|---|---|---|---|
| IPC transport | `arclens_ipc::Endpoint` | Unix socket in `$XDG_RUNTIME_DIR` | named pipe `\\.\pipe\arclens-<user>` |
| Screen capture | `arclens_capture::Capture` | XDG ScreenCast portal + PipeWire | Windows Graphics Capture (primary monitor) |
| Hotkeys | `arclens_hotkeys::{init, listen}` | GlobalShortcuts portal | `RegisterHotKey` (global-hotkey) |
| Overlay surface | `arclens-overlay` `shell::{run, sync_surface, input_task}` | layer-shell, OVERLAY layer, input region | topmost transparent window, cursor-polled click-through |
| Desktop integration | `arclens::platform::{integrate, window_platform}` | desktop entry + app id | nothing (window icon) |
| Game detection | `game_process` | sysinfo (portable) | sysinfo (portable) |

Rules:
- Dependencies point **down** the table: no lib depends on a bin, and
  `core` depends on nothing internal.
- Anything testable without a display lives in a lib crate.
- `cargo build` / `cargo test` with no flags builds only the libraries
  (`default-members`), so CI and contributors don't need GTK/Wayland headers
  for the core work.

## Coordinate spaces

See `crates/arclens-core/src/map.rs`:

- **source space:** an upstream dataset's pixels;
- **map space:** normalised `[0,1]²` over our map image;
- **screen space:** monitor pixels.

`Transform` (affine) converts between them. Calibration or vision produces the
map→screen transform, and the overlay applies it per marker.

## Data lifecycle

1. On start, load `$XDG_CACHE_HOME/arclens/catalog.json`.
2. If it is missing or older than 24 h, download the RaidTheory tarball
   (JSON only), rebuild the catalog and write the cache atomically.
3. If the refresh fails, fall back to the stale cache. If there is no cache,
   show an error.
4. `ARCLENS_RAIDTHEORY_DIR=/path/to/checkout` bypasses steps 1–3 (offline
   development).

## IPC protocol

- Defined in `crates/arclens-ipc/src/lib.rs`.
- One JSON object per line, internally tagged with `"type"` in snake_case.
- Both sides send `hello {protocol}` first. Bump `PROTOCOL_VERSION` on any
  breaking change.
- The overlay talks back too (`ToApp`): toggles in its map panel are
  sent to the app, which owns the marker filter, saves it and sends a new
  `ShowMapPanel`.
- Handy for debugging: `socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/arclens.sock`,
  or replay `scripts/overlay-demo.jsonl` with `scripts/overlay-demo.sh`.
