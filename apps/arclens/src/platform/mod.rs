//! How the app fits into the desktop, per platform:
//! - Linux (`linux.rs`): a desktop entry and icon in `~/.local/share` for
//!   runs outside a package, and the Wayland app id they match;
//! - Windows (`windows.rs`): nothing to install; the window icon is enough.

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod backend;
#[cfg(windows)]
#[path = "windows.rs"]
mod backend;

pub use backend::{integrate, window_platform};

pub const ICON_SVG: &[u8] =
    include_bytes!("../../../../packaging/icons/io.github.Stay1444.ARClens.svg");
/// 256 px rendering of the icon, for window icons (Windows, X11).
pub const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-256.png");

/// The icon for the window itself (Windows and X11; Wayland ignores it).
pub fn window_icon() -> Option<iced::window::Icon> {
    let image = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    iced::window::icon::from_rgba(image.into_raw(), width, height).ok()
}
