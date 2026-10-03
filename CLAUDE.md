# CLAUDE.md

@AGENTS.md

## Claude-specific notes

- Before calling a task done, run the full check:
  `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- For UI changes, take a headless screenshot (see `docs/development.md`) and
  look at it. Don't claim a visual change works without seeing it.
- When researching game, API or anti-cheat facts, write the findings into
  `docs/research/` with sources and a date. Don't leave them only in chat.
