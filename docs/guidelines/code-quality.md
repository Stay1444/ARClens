# Code quality guidelines

## Non-negotiables

- `cargo fmt --all` is clean.
- `cargo clippy --workspace --all-targets -- -D warnings` is clean. Lints are
  configured in the root `Cargo.toml` (`[workspace.lints]`): `clippy::pedantic`
  plus `unwrap_used`/`expect_used` warnings outside tests.
- `cargo test --workspace` passes.
- **No `unsafe`** (`unsafe_code = "deny"`). If a platform API truly needs it,
  isolate it in a small module with `#[allow(unsafe_code)]` and a `// SAFETY:`
  comment on every block, and justify it in the PR.
- Never touch the game process: no memory reads, injection, hooks, input
  synthesis into the game, or network sniffing. See
  `docs/research/game-state-detection.md`.

## Structure

- Pure logic goes in `arclens-core`. If it can be a function over plain data,
  it belongs there with unit tests.
- I/O sits at the edges: providers in `arclens-data`, sockets in
  `arclens-ipc`, and portals in their own crates.
- UI crates (`apps/*`) wire things together and render. Keep `update`
  functions thin and move logic into libs.
- One concept per module. A file over ~400 lines is a smell.

## Errors

- Libraries define a `thiserror` enum. Binaries use `anyhow` with
  `.context("doing X")`.
- Don't `unwrap()`/`expect()` in non-test code. Handle the error, or
  propagate it with `?`.
- Degrade, don't crash. A missing portal, network or overlay should produce a
  status message and a working app. Examples: a stale cache when offline, and
  buttons when there are no hotkeys.
- Log with `tracing` using structured fields:
  `tracing::warn!(%error, path = %p.display(), "msg")`.

## Async and threads

- The companion app runs on iced's tokio executor. Blocking file or CPU work
  goes in `tokio::task::spawn_blocking`.
- Long-lived background work is an iced `Subscription` (see
  `apps/arclens/src/overlay_link.rs`).
- Nothing on the render path should allocate per frame without need,
  especially in the overlay.

## Data and external sources

- Every upstream source gets:
  - a provider module;
  - a fixture-based test (`tests/fixtures/<source>/`);
  - attribution in the README ("Data and attribution"), not in the UI;
  - an entry in `docs/research/data-sources.md` with its terms.
- Cache everything and respect upstream rate limits. Set a `User-Agent` of
  `ARClens/<version>`.
- Parse defensively: use `#[serde(default)]` for optional upstream fields,
  and don't fail the whole load on one bad record unless the data is
  corrupt.

## Tests

- Unit tests sit next to the code (`#[cfg(test)] mod tests`).
- Fixture tests use small, hand-trimmed real data. Don't commit full
  datasets.
- Tests must not hit the network or need a display. Anything that does goes
  behind `#[ignore]` with a comment saying how to run it.
- Prefer asserting behaviour ("wires is required by Gear Bench 1") over
  snapshotting whole structs.

## Naming and style

- Follow the Rust API Guidelines. Use the game's terms (`Raider`, `ARC`,
  `Workshop`, `Topside`) consistently.
- Doc comments explain *why* and invariants; code shows *what*.
- Every public item in a lib crate has a doc comment.
- Keep dependencies in `[workspace.dependencies]` with minor-version pins.
  Justify new heavy dependencies in the PR.

## Commits

- Use small, focused commits with an imperative subject line (≤ 72 chars),
  and a body explaining *why* when it isn't obvious.
- Each commit builds and passes tests.
