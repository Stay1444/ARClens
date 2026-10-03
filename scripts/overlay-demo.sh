#!/usr/bin/env bash
# Drive arclens-overlay without the companion app: socat plays the app side
# of the IPC socket and replays scripts/overlay-demo.jsonl.
#
# Usage: scripts/overlay-demo.sh   (from a Wayland session with layer-shell)
# Requires: socat. Stop the companion app first (it owns the socket).
set -euo pipefail

cd "$(dirname "$0")/.."
sock="${XDG_RUNTIME_DIR:-/tmp}/arclens.sock"
rm -f "$sock"

cargo build -p arclens-overlay
(cat scripts/overlay-demo.jsonl; sleep "${HOLD_SECONDS:-30}") | socat - "UNIX-LISTEN:$sock" &
server=$!
trap 'kill $server 2>/dev/null; rm -f "$sock"' EXIT
sleep 0.5
timeout "${HOLD_SECONDS:-30}" ./target/debug/arclens-overlay || true
