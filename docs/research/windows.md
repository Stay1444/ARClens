# Windows support (2026-10-03)

How ARClens works on Windows, what is verified and what is not.

## Status

- **Builds**: the workspace passes `cargo clippy` for
  `x86_64-pc-windows-gnu` (cross-checked from Linux with mingw-w64) and
  CI builds, lints and tests natively on `windows-latest`.
- **Tested under Wine** (2026-10-03): the IPC named-pipe tests, the capture
  slot/conversion tests and the hotkey tests run as Windows binaries. Wine
  found a real bug: a second `bind` succeeded despite
  `FILE_FLAG_FIRST_PIPE_INSTANCE`, so the backend now probes the pipe as a
  client first (like the Unix socket backend).
- **Not tested on real Windows yet** (**unverified**): Graphics Capture of
  the game, the overlay window over the game, hotkeys while the game has
  focus.

## Design

| Area | Choice | Why |
|---|---|---|
| Capture | Windows Graphics Capture of the primary monitor (`windows-capture`), cursor and border off, ≤ 30 fps | The OS screen-capture API screen recorders use; no game access. |
| Overlay | One transparent, undecorated, always-on-top window, opened once and kept | Showing a window activates it, which would take focus from the game whenever the map opens. |
| Click-through | Whole window click-through; over the map panel the cursor is polled (50 ms) and click-through is switched off | Windows has no per-region input like Wayland's input region. |
| Hotkeys | `RegisterHotKey` for Ctrl+Shift+O / I through `global-hotkey`, registered on the main thread before iced's loop | Messages are delivered through the registering thread's message loop. |
| IPC | Named pipe `\\.\pipe\arclens-<user>` | Per user, like the Unix socket. |

## Known limits

- **Exclusive fullscreen** hides any overlay window; the game must run
  borderless windowed for the overlay to show. Capture works either way.
- Only the **primary monitor** is captured and overlaid.
- The hotkeys are fixed (Windows has no system place to rebind them);
  registration fails if another program holds the combination.
- **Anti-cheat**: capture and a topmost window are what screen recorders
  and overlays (Discord, Steam) do, with no injection or memory access.
  Whether Denuvo Anti-Cheat treats them any differently on Windows is
  **unverified**.
