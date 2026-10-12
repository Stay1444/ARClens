//! Windows: the window carries its own icon; nothing to install.

/// Nothing to set up.
pub fn integrate() {}

pub fn window_platform() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}

/// A Windows toast with the condition's icon. Toasts from an unpackaged app
/// can't report clicks back, so this never asks to open the app.
pub fn notify(notice: &super::Notice) -> Result<bool, notify_rust::error::Error> {
    let mut notification = notify_rust::Notification::new();
    notification
        .appname("ARClens")
        .summary(&notice.title)
        .body(&notice.body);
    if let Some(image) = notice.image.as_deref().and_then(std::path::Path::to_str) {
        notification.image_path(image);
    }
    notification.show()?;
    Ok(false)
}
