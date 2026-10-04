#!/usr/bin/env bash
# Regenerates cargo-sources.json (every crate in Cargo.lock as a Flatpak
# source) for the offline Flathub build. Run it after Cargo.lock changes.
# Needs `uv` (it pulls the generator's Python dependencies itself).
set -euo pipefail
cd "$(dirname "$0")"
generator=$(mktemp -d)/flatpak-cargo-generator.py
curl -sSfL -o "$generator" \
  https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
uv run --quiet "$generator" ../../Cargo.lock -o cargo-sources.json
echo "cargo-sources.json updated"
