//! Windows: the window carries its own icon; nothing to install.

/// Nothing to set up.
pub fn integrate() {}

pub fn window_platform() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}
