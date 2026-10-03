# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.
`CLAUDE.md` imports this file.

## What this is

ARClens is a Linux-first companion app and in-game overlay for **ARC Raiders**
(Embark Studios). It does item lookup with keep/sell/recycle advice, map
markers and event timers. Primary target: **KDE Plasma 6 on Wayland
(Fedora)**, with the game under Steam/Proton.

## Hard rules

1. **Never interact with the game process.** No memory reading (not even
   "read-only"), no injection, no Vulkan layers, no `LD_PRELOAD`, no hooking,
   no sniffing its network traffic, and no synthesising input into it. The
   game runs Denuvo Anti-Cheat, and users' accounts are at stake. Game state
   comes only from public data and **screen capture through the XDG portal**.
   See `docs/research/game-state-detection.md`.
2. **Respect data sources.** Use only sources listed in
   `docs/research/data-sources.md`, with their caching and terms.
   Attribution goes in the README's "Data and attribution" section, not in
   the app or overlay UI (maintainer's call). No scraping of sites without
   an API. No Embark private API.
3. **No `unsafe`** without a justified, isolated exception (see
   `docs/guidelines/code-quality.md`).
4. Keep `arclens-core` pure: no I/O, async or UI dependencies.
5. The overlay must stay **wgpu-only**. Don't enable iced's `tiny-skia`
   feature: its Wayland buffers have no alpha, so the overlay would paint
   opaque black over the game.
6. Never decode images in `view`. Decode once with `arclens_ui::decode_icon`
   and keep the handle. (iced's lazy `Handle::from_path` draws nothing under
   `iced_layershell`.)
7. Don't `cargo update` `winit-core`/`winit-common` past `0.31.0-beta.2`
   until `iced_exdevtools` is fixed upstream (see
   `docs/research/wayland-overlay.md`).

## Layout

```
crates/arclens-core      pure domain types + logic (items, advice, maps, transforms)
crates/arclens-data      RaidTheory provider, download, disk cache, fuzzy search
crates/arclens-ipc       app <-> overlay protocol (NDJSON over a Unix socket)
crates/arclens-hotkeys   XDG GlobalShortcuts portal
crates/arclens-ui        shared iced widgets + design tokens (item card, palette)
crates/arclens-vision    frame analysis: tooltip panels, name lines, OCR (pure, no I/O)
crates/arclens-capture   screen capture: XDG ScreenCast portal + PipeWire
apps/arclens             companion app (iced): owns all state
apps/arclens-overlay     overlay (iced_layershell, OVERLAY layer): dumb renderer
docs/                    research, architecture, guidelines, roadmap
scripts/                 dev tools (overlay-demo.sh)
packaging/               .desktop file etc.
```

Start with `docs/architecture/overview.md`. Plans live in `docs/ROADMAP.md`.

## Commands

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p arclens-overlay && cargo run -p arclens   # app (auto-starts the overlay)
```

Plain `cargo build`/`cargo test` builds only the library crates
(`default-members`). The apps need Wayland, xkbcommon and Vulkan dev packages
(`docs/development.md`).

Offline data: `ARCLENS_RAIDTHEORY_DIR=/path/to/arcraiders-data cargo run -p arclens`.

## Definition of done for a change

- fmt, clippy (`-D warnings`) and tests are green for the whole workspace.
- New logic has unit tests; new data sources have fixture tests.
- Docs are updated when behaviour, architecture or research findings change:
  - `docs/ROADMAP.md` checkboxes;
  - research notes, with a date;
  - this file, if a rule changes.
- The IPC protocol changed → `PROTOCOL_VERSION` bumped if breaking, and
  `scripts/overlay-demo.jsonl` still works.
- Commits are small, with imperative subjects. Each one builds.

## Workflow notes

- The maintainer wants a commit on `master` after each completed step.
- UI work can be verified headlessly. See "Headless screenshots" in
  `docs/development.md` (Sway + lavapipe + grim; keyboard input only).
- Mark research claims as verified or unverified and date them. Community APIs
  and game patches change fast.
