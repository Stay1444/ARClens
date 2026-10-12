//! How the app fits into the desktop, per platform:
//! - Linux (`linux.rs`): a desktop entry and icon in `~/.local/share` for
//!   runs outside a package, and the Wayland app id they match;
//! - Windows (`windows.rs`): nothing to install; the window icon is enough.
//!
//! Both show desktop notifications ([`notify`]): the desktop's own
//! notification service on Linux (D-Bus), toasts on Windows.

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod backend;
#[cfg(windows)]
#[path = "windows.rs"]
mod backend;

pub use backend::{integrate, window_platform};

/// A desktop notification: title, body, an image (the condition's icon),
/// and the label of a button that opens the app.
#[derive(Debug, Clone)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub image: Option<std::path::PathBuf>,
    #[cfg_attr(
        windows,
        expect(
            dead_code,
            reason = "toasts from an unpackaged app can't report clicks"
        )
    )]
    pub open_label: String,
}

/// Shows `notice`; resolves to whether the player clicked it (or its
/// button) to open the app. Blocking work runs off the UI thread.
pub async fn notify(mut notice: Notice) -> bool {
    tokio::task::spawn_blocking(move || {
        // Notification servers read PNG reliably; condition icons are WebP.
        notice.image = notice.image.as_deref().and_then(png_copy);
        backend::notify(&notice).unwrap_or_else(|error| {
            tracing::warn!(%error, "desktop notification failed");
            false
        })
    })
    .await
    .unwrap_or(false)
}

/// A PNG copy of an image file next to it (made once), for notifications.
fn png_copy(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let png = path.with_extension("notice.png");
    if !png.is_file() {
        let image = image::open(path).ok()?;
        image.save_with_format(&png, image::ImageFormat::Png).ok()?;
    }
    Some(png)
}

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
