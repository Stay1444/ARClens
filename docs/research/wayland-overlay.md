# Research: drawing an overlay on Linux / Wayland

_Last researched: 2026-10-03. Target: KDE Plasma 6 Wayland on Fedora, game
under Steam/Proton._

## Window type: wlr-layer-shell on the OVERLAY layer

- KWin, Sway, Hyprland, other wlroots compositors and COSMIC support
  `zwlr_layer_shell_v1`. **GNOME/Mutter does not.**
- In KWin, only the **OVERLAY** layer is stacked above fullscreen windows.
  It maps to KWin's `OverlayLayer`, above `ActiveLayer`. The TOP layer maps to
  `AboveLayer`, which sits *below* a fullscreen game. Source:
  `kwin/src/layershellv1window.cpp`.
- This holds whether Proton runs through XWayland (the default) or the native
  Wine Wayland driver (`PROTON_ENABLE_WAYLAND=1` on Proton-GE/Experimental).
  Both are ordinary toplevels to KWin.
- Wine's "exclusive fullscreen" is emulated on Wayland, so it behaves like
  borderless and the overlay still shows.

## Field test results

| Date | Desktop | Game mode | Result |
|---|---|---|---|
| 2026-10-03 | KDE Plasma 6 Wayland, Fedora | ARC Raiders, Proton with `PROTON_ENABLE_WAYLAND=1` | ✅ Overlay drawn above the game, click-through works, GlobalShortcuts bound (Ctrl+Shift+O / Ctrl+Shift+I) |
| — | KDE Plasma 6 Wayland | Proton via XWayland (default) | Not yet tested |

## Click-through and interactive mode

- **Passive:** empty input region (`wl_surface.set_input_region`) and
  `keyboard_interactivity = none`. All input goes to the game.
- **Interactive:** a real input region and `keyboard_interactivity =
  on_demand`. KWin activates the surface when it starts accepting focus.
  Expect focus not to return to the game automatically.

## Performance

- A *mapped* overlay surface over a fullscreen game defeats direct scanout,
  which forces composition: a few percent FPS, plus latency and power. KWin
  6.5+ KMS overlay planes can mitigate this.
- **When hidden, the overlay must unmap or destroy its surface**, not just draw
  transparent pixels. Implemented (2026-10-03): daemon mode, with the surface
  created and removed on demand.

## Rendering: transparency pitfall (verified)

- iced's software fallback (tiny-skia + `softbuffer` 0.4) allocates
  `wl_shm` buffers as **`Xrgb8888`**, with no alpha. A full-screen overlay
  rendered that way is **opaque black** and hides the game.
- We therefore build iced **without** the `tiny-skia` feature, so the overlay
  is wgpu-only. If wgpu can't start, the overlay fails to launch instead of
  blacking out the screen.

## UI toolkit choice

| Option | Layer shell? | Verdict |
|---|---|---|
| **iced 0.14 + `iced_layershell` 0.19** | Yes, plus input regions and keyboard modes | **Chosen** for the overlay |
| iced 0.14 (winit) | No (winit has no layer-shell; issue #2582 open) | **Chosen** for the companion window |
| Dioxus desktop 0.7 (tao + WebKitGTK) | Only through a fragile gtk-layer-shell hack (init before realize); WebKit transparency bugs | Rejected |
| Dioxus native / Blitz | No (blitz-shell is hard-wired to winit, beta) | Revisit later |
| egui + smithay-client-toolkit | Yes, but you write the glue | Viable fallback |
| gtk4 + gtk4-layer-shell | Yes, very mature | Viable, but C dependencies and a different paradigm |
| Slint | Only via the third-party `layer-shika` | Too young |

`iced_layershell` is being renamed to **`iced_exwlshell`** (0.20, Sept 2026).
Stay on 0.19 until 0.20 settles, then migrate.

### Known issue: winit-core pin

`iced_exdevtools` 0.19.1 (a dependency of `iced_layershell`) fails to build
against `winit-core 0.31.0-beta.3`, which adds a variant to a matched enum.
`Cargo.lock` pins `winit-core`/`winit-common` to `0.31.0-beta.2`. **Don't
`cargo update` those two** until upstream fixes it.

## Global hotkeys

- Wayland gives clients no global key access by design. Use the
  **XDG GlobalShortcuts portal** (`ashpd` 0.13, `global_shortcuts` feature).
- KDE shows a confirmation dialog on first bind. Bindings then appear in
  System Settings → Shortcuts.
- The app needs a `.desktop` file for its id to persist
  (`packaging/arclens.desktop`).
- Keys still reach the game; the portal only notifies us.
- Fallbacks: reading `/dev/input` via `evdev` needs the `input` group, which is
  a security smell. Only add it opt-in.

## Never inject into the game

- Vulkan layers (MangoHud-style), `LD_PRELOAD` and hooking into Proton all
  load code into the game process.
- With Denuvo Anti-Cheat that is a kick or ban risk ("ARAV1011 prohibited
  software" hit Linux users in June 2026).
- Overlays stay **out of process**.

## Gamescope

- In nested mode, gamescope is one toplevel on KDE, and a layer-shell overlay
  still draws above it.
- Inside a gamescope *session* (Steam Deck game mode), we would need
  gamescope's own layer-shell support. Out of scope for now.

## Other platforms (later)

- **X11:** override-redirect ARGB window plus an XShape empty input region
  (`x11rb`).
- **Windows:** `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST |
  WS_EX_NOACTIVATE`. Works over borderless only.
