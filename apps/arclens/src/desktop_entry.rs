//! The desktop entry and icon, for runs outside a package.
//!
//! On Wayland a window has no icon of its own: KDE shows the icon of the
//! installed `.desktop` file whose name matches the window's app id, and
//! the `GlobalShortcuts` portal keys saved bindings by it too. Flatpak and
//! `AppImage` installs bring their own; a plain build (`cargo run`, a copied
//! binary) writes one to `~/.local/share` pointing at itself.

use std::path::Path;

/// The window's app id; the desktop file and icon are named after it.
pub const APP_ID: &str = "io.github.Stay1444.ARClens";
/// Set to `1` to leave `~/.local/share` alone.
pub const DISABLE_ENV: &str = "ARCLENS_NO_DESKTOP_ENTRY";

pub const ICON_SVG: &[u8] =
    include_bytes!("../../../packaging/icons/io.github.Stay1444.ARClens.svg");
const DESKTOP: &str = include_str!("../../../packaging/io.github.Stay1444.ARClens.desktop");
/// 256 px rendering of the icon, for the window icon where the platform
/// takes one (X11).
pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

/// The icon for the window itself (used on X11; Wayland ignores it).
pub fn window_icon() -> Option<iced::window::Icon> {
    let image = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    iced::window::icon::from_rgba(image.into_raw(), width, height).ok()
}

/// Writes the desktop file and icon unless a package provides them or the
/// user opted out. Only rewrites files that differ. Errors are logged.
pub fn install() {
    let packaged =
        std::env::var_os("FLATPAK_ID").is_some() || std::env::var_os("APPIMAGE").is_some();
    if packaged || std::env::var(DISABLE_ENV).is_ok_and(|v| v == "1") {
        return;
    }
    let Some(base) = directories::BaseDirs::new() else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if let Err(error) = write(base.data_dir(), &exe) {
        tracing::warn!(%error, "could not install the desktop entry");
    }
}

fn write(data_dir: &Path, exe: &Path) -> std::io::Result<()> {
    let icon = data_dir
        .join("icons/hicolor/scalable/apps")
        .join(format!("{APP_ID}.svg"));
    let desktop = data_dir
        .join("applications")
        .join(format!("{APP_ID}.desktop"));
    write_if_changed(&icon, ICON_SVG)?;
    write_if_changed(&desktop, entry(exe).as_bytes())
}

/// The packaged desktop file, launching `exe`.
fn entry(exe: &Path) -> String {
    let exe = exe.to_string_lossy();
    // Desktop-entry Exec quoting: wrap in quotes, escape `"`, `` ` ``, `$`, `\`.
    let quoted: String = exe
        .chars()
        .flat_map(|c| {
            let escape = matches!(c, '"' | '`' | '$' | '\\');
            escape.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect();
    DESKTOP
        .lines()
        .map(|line| {
            if line.starts_with("Exec=") {
                format!("Exec=\"{quoted}\"")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)?;
    tracing::info!(path = %path.display(), "installed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_an_entry_launching_this_binary() {
        let dir = tempfile::tempdir().unwrap();
        let exe = Path::new("/home/me/My Games/arclens");
        write(dir.path(), exe).unwrap();
        let desktop = std::fs::read_to_string(
            dir.path()
                .join("applications/io.github.Stay1444.ARClens.desktop"),
        )
        .unwrap();
        assert!(desktop.contains("Exec=\"/home/me/My Games/arclens\"\n"));
        assert!(desktop.contains("Icon=io.github.Stay1444.ARClens\n"));
        assert!(
            dir.path()
                .join("icons/hicolor/scalable/apps/io.github.Stay1444.ARClens.svg")
                .is_file()
        );
        // Unchanged files are left alone.
        write(dir.path(), exe).unwrap();
        assert!(entry(Path::new("/a/$b")).contains("Exec=\"/a/\\$b\""));
    }
}
