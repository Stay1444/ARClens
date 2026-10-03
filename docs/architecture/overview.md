# Architecture overview

## Processes

```
┌──────────────────────────── arclens (companion app) ────────────────────────────┐
│  iced window (winit)                                                            │
│  ├─ data:     Catalog ← DiskCache ← RaidTheory download   (arclens-data)        │
│  ├─ hotkeys:  XDG GlobalShortcuts portal                  (arclens-hotkeys)     │
│  ├─ vision:   ScreenCast portal → detectors  [planned]    (arclens-vision)      │
│  └─ IPC server  $XDG_RUNTIME_DIR/arclens.sock             (arclens-ipc)         │
└──────────────────────────────────────┬──────────────────────────────────────────┘
                                       │ newline-delimited JSON (ToOverlay / ToApp)
┌──────────────────────────────────────▼──────────────────────────────────────────┐
│  arclens-overlay: iced_layershell, OVERLAY layer, click-through, wgpu only      │
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
| `arclens-ipc` | lib | Wire protocol, socket framing, version handshake | core, tokio |
| `arclens-hotkeys` | lib | GlobalShortcuts portal wrapper, stable action ids | ashpd |
| `arclens-ui` | lib | Design tokens and the shared item card used by app and overlay | core, iced |
| `arclens-vision` | lib | Frame → tooltip panels → name line (done); name recognition, map registration (planned) | image |
| `arclens` | bin | Companion app: owns state, wires everything | all libs, iced |
| `arclens-overlay` | bin | Overlay renderer | core, ipc, iced_layershell |

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
- Handy for debugging: `socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/arclens.sock`,
  or replay `scripts/overlay-demo.jsonl` with `scripts/overlay-demo.sh`.
