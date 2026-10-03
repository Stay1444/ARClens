# Contributing

Thanks for helping! In short:

1. Read [`AGENTS.md`](AGENTS.md). Its hard rules apply to humans too,
   especially **never touching the game process**.
2. Set up your machine: [`docs/development.md`](docs/development.md).
3. Follow [`docs/guidelines/code-quality.md`](docs/guidelines/code-quality.md).
4. Before pushing:

   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

5. Keep PRs focused. Update docs and roadmap checkboxes in the same PR.

## Adding a data source

- Check its terms and record them in
  [`docs/research/data-sources.md`](docs/research/data-sources.md).
- Add a provider module with a small fixture test.
- Attribute it in `Catalog::source`.
- Cache the responses.
