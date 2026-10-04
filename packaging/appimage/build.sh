#!/usr/bin/env bash
# Builds ARClens-<version>-x86_64.AppImage in the repository root.
#
# Needs the app's build dependencies (see docs/development.md) and network
# access to fetch linuxdeploy. CI runs it on Ubuntu 24.04 (the oldest LTS
# with PipeWire headers the pipewire crate accepts).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
version="${VERSION:-$(cargo metadata --no-deps --format-version 1 \
  | sed -n 's/.*"name":"arclens","version":"\([^"]*\)".*/\1/p' | head -n1)}"
work="$root/target/appimage"
appdir="$work/AppDir"

cargo build --release --locked -p arclens -p arclens-overlay

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
# Both binaries side by side: the app starts the overlay from its own dir.
install -m755 target/release/arclens target/release/arclens-overlay "$appdir/usr/bin/"
install -Dm644 LICENSE "$appdir/usr/share/licenses/arclens/LICENSE"

tool="$work/linuxdeploy-x86_64.AppImage"
if [ ! -x "$tool" ]; then
  curl -fsSL -o "$tool" \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  chmod +x "$tool"
fi

# No FUSE in containers/CI.
export APPIMAGE_EXTRACT_AND_RUN=1
export LDAI_OUTPUT="ARClens-${version}-x86_64.AppImage"
"$tool" --appdir "$appdir" \
  --executable "$appdir/usr/bin/arclens" \
  --executable "$appdir/usr/bin/arclens-overlay" \
  --desktop-file packaging/io.github.Stay1444.ARClens.desktop \
  --icon-file packaging/icons/io.github.Stay1444.ARClens.svg \
  --output appimage
echo "built $LDAI_OUTPUT"
