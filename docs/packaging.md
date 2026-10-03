# Packaging and releases

Two formats are built by `.github/workflows/release.yml`:

| Format | Built on | Notes |
|---|---|---|
| AppImage (`ARClens-<version>-x86_64.AppImage`) | Ubuntu 24.04 (22.04's PipeWire 0.3.48 headers are too old for the `pipewire` crate) | glibc ≥ 2.39 (Fedora 40+, Ubuntu 24.04+). Wayland, xkbcommon and Vulkan are loaded from the host at runtime; `libpipewire-0.3` is linked but deliberately not bundled (it must match the host's PipeWire). Smoke-tested headlessly 2026-10-03: app and overlay start from the AppImage and connect. |
| Flatpak bundle (`ARClens-x86_64.flatpak`) | Freedesktop 25.08 SDK + rust-stable and llvm20 extensions | App id `io.github.Stay1444.ARClens`. Install with `flatpak install --user ARClens-x86_64.flatpak`. |

Both ship the app and the overlay side by side in one `bin/`; the app
starts the overlay from its own directory.

## Making a release

1. Bump `version` in the workspace `Cargo.toml` and the `<release>` entry in
   `packaging/io.github.Stay1444.ARClens.metainfo.xml`.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. The workflow builds both formats and attaches them to a GitHub release
   with generated notes.

A manual run (Actions → Release → Run workflow) builds both and keeps them
as workflow artifacts, without a release.

## Building locally

```sh
packaging/appimage/build.sh            # → ARClens-<version>-x86_64.AppImage

flatpak install --user flathub org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08 \
  org.freedesktop.Sdk.Extension.llvm20//25.08
flatpak-builder --user --install --force-clean build-dir \
  packaging/flatpak/io.github.Stay1444.ARClens.yml
flatpak run io.github.Stay1444.ARClens
```

## Sandbox notes (Flatpak)

- Wayland socket (the overlay is a layer-shell surface), DRI for wgpu,
  network for game data. Screen capture and global shortcuts use portals,
  which need no extra permissions.
- Cargo downloads crates during the build (network allowed for that
  module). Fine for our bundles; **Flathub** would need vendored sources
  (`flatpak-cargo-generator`) and a review of the permissions.
- Config, cache and state live in `~/.var/app/io.github.Stay1444.ARClens/`
  instead of `~/.config/arclens` etc.

## Open points

- No licence file yet: the AppStream data omits `project_license`. Pick a
  licence before publishing widely.
- Not yet verified on a real desktop (2026-10-03): the Flatpak's portal
  screen capture and global shortcuts, and the AppImage on Fedora 43.
