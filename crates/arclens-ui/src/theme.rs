//! The look shared by the app and the overlay, after ARC Raiders' own UI:
//! condensed uppercase headings, cream header bars over dark panels, a
//! yellow accent, outlined pills for the selected tab. Bigger type and
//! more room than a typical desktop app, so it reads at a glance.
//!
//! Fonts: Barlow and Barlow Condensed (SIL Open Font License, bundled in
//! `assets/fonts` with the licence).

use crate::palette;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Padding, font};

/// Font files to load at start-up (`iced::application(..).font(..)`).
pub const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/BarlowCondensed-Bold.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Barlow-Regular.ttf"),
    include_bytes!("../assets/fonts/Barlow-Medium.ttf"),
    include_bytes!("../assets/fonts/Barlow-SemiBold.ttf"),
];

/// Body text.
pub const BODY: Font = Font::with_name("Barlow");
/// Emphasised body text (names, values).
pub const STRONG: Font = Font {
    weight: font::Weight::Semibold,
    ..BODY
};
/// Headings, tabs, labels: condensed, set in capitals.
pub const DISPLAY: Font = Font {
    weight: font::Weight::Bold,
    ..Font::with_name("Barlow Condensed")
};
/// Smaller condensed labels.
pub const DISPLAY_SEMI: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::with_name("Barlow Condensed")
};

/// Type scale, logical pixels.
pub mod size {
    pub const TITLE: f32 = 34.0;
    pub const H1: f32 = 26.0;
    pub const H2: f32 = 20.0;
    pub const BODY: f32 = 17.0;
    pub const SMALL: f32 = 15.0;
    pub const TINY: f32 = 13.0;
}

/// Spacing, logical pixels.
pub mod space {
    /// Around a page.
    pub const PAGE: f32 = 32.0;
    /// Between sections.
    pub const SECTION: f32 = 28.0;
    /// Between items in a list or row.
    pub const GAP: f32 = 14.0;
    /// Inside a panel.
    pub const PANEL: f32 = 18.0;
}

/// The game's colours.
pub const BG: Color = Color::from_rgb(0.043, 0.051, 0.067);
pub const PANEL: Color = Color::from_rgb(0.075, 0.090, 0.114);
pub const PANEL_RAISED: Color = Color::from_rgb(0.110, 0.129, 0.161);
/// Header bars, selected items: the game's cream.
pub const CREAM: Color = Color::from_rgb(0.937, 0.910, 0.851);
/// Text on cream.
pub const INK: Color = Color::from_rgb(0.071, 0.075, 0.090);
/// The PLAY button's yellow.
pub const ACCENT: Color = Color::from_rgb(0.976, 0.769, 0.090);
pub const RADIUS: f32 = 6.0;

/// A page or section heading: condensed capitals.
pub fn heading<'a, M: 'a>(label: &str, size: f32) -> Element<'a, M> {
    text(label.to_uppercase())
        .size(size)
        .font(DISPLAY)
        .color(palette::TEXT)
        .into()
}

/// A small caps label over a value ("SELL", "NOW · ENDS IN").
pub fn label<'a, M: 'a>(label: &str) -> Element<'a, M> {
    text(label.to_uppercase())
        .size(size::TINY)
        .font(DISPLAY_SEMI)
        .color(palette::TEXT_MUTED)
        .into()
}

/// A panel like the game's Quests box: a cream header bar with a
/// condensed title (and something on its right), over a dark body.
pub fn panel<'a, M: 'a>(
    title: &str,
    aside: Option<Element<'a, M>>,
    body: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    let mut header = row![
        text(title.to_uppercase())
            .size(size::H2)
            .font(DISPLAY)
            .color(INK)
            .width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if let Some(aside) = aside {
        header = header.push(aside);
    }
    column![
        container(header)
            .padding([8.0, space::PANEL])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(CREAM.into()),
                border: Border {
                    radius: iced::border::Radius::default().top(RADIUS),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        container(body)
            .padding(space::PANEL)
            .width(Length::Fill)
            .style(panel_body),
    ]
    .into()
}

/// The dark body of a [`panel`].
pub fn panel_body(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(PANEL.into()),
        border: Border {
            radius: iced::border::Radius::default().bottom(RADIUS),
            color: palette::BORDER,
            width: 1.0,
        },
        text_color: Some(palette::TEXT),
        ..container::Style::default()
    }
}

/// A plain dark card (no header).
pub fn card(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(PANEL.into()),
        border: Border {
            radius: RADIUS.into(),
            color: palette::BORDER,
            width: 1.0,
        },
        text_color: Some(palette::TEXT),
        ..container::Style::default()
    }
}

/// A top-bar tab like the game's (PLAY, WORKSHOP, …): capitals, the
/// selected one outlined in a pill.
pub fn tab<'a, M: Clone + 'a>(label: &str, active: bool, on_press: M) -> Element<'a, M> {
    button(text(label.to_uppercase()).size(size::H2).font(DISPLAY_SEMI))
        .padding([4, 16])
        .on_press(on_press)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: None,
                text_color: if active || hovered {
                    palette::TEXT
                } else {
                    palette::with_alpha(palette::TEXT, 0.62)
                },
                border: Border {
                    color: if active {
                        palette::TEXT
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 2.0,
                    radius: 20.0.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

/// A choice among a few (filters, settings): cream when picked.
pub fn chip<'a, M: Clone + 'a>(label: &str, active: bool, on_press: M) -> Element<'a, M> {
    button(text(label.to_owned()).size(size::SMALL).font(STRONG))
        .padding([6, 14])
        .on_press(on_press)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(if active {
                    CREAM.into()
                } else if hovered {
                    palette::with_alpha(palette::TEXT, 0.12).into()
                } else {
                    palette::with_alpha(palette::TEXT, 0.05).into()
                }),
                text_color: if active { INK } else { palette::TEXT },
                border: Border {
                    color: if active { CREAM } else { palette::BORDER },
                    width: 1.0,
                    radius: RADIUS.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

/// The main action: the game's yellow.
pub fn primary_button<'a, M: Clone + 'a>(label: &str, on_press: Option<M>) -> Element<'a, M> {
    button(text(label.to_uppercase()).size(size::H2).font(DISPLAY))
        .padding([8, 22])
        .on_press_maybe(on_press)
        .style(|_, status| button::Style {
            background: Some(
                match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Color::from_rgb(1.0, 0.83, 0.25)
                    }
                    button::Status::Disabled => palette::with_alpha(ACCENT, 0.35),
                    button::Status::Active => ACCENT,
                }
                .into(),
            ),
            text_color: INK,
            border: Border {
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .into()
}

/// A secondary action: outlined.
pub fn secondary_button<'a, M: Clone + 'a>(label: &str, on_press: Option<M>) -> Element<'a, M> {
    button(text(label.to_owned()).size(size::SMALL).font(STRONG))
        .padding([7, 16])
        .on_press_maybe(on_press)
        .style(|_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(
                    palette::with_alpha(palette::TEXT, if hovered { 0.12 } else { 0.04 }).into(),
                ),
                text_color: if matches!(status, button::Status::Disabled) {
                    palette::TEXT_MUTED
                } else {
                    palette::TEXT
                },
                border: Border {
                    color: palette::with_alpha(palette::TEXT, 0.25),
                    width: 1.0,
                    radius: RADIUS.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

/// A search or text box on the dark panels.
pub fn input_style(
    _: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    use iced::widget::text_input::Status;
    let focused = matches!(status, Status::Focused { .. });
    iced::widget::text_input::Style {
        background: PANEL_RAISED.into(),
        border: Border {
            radius: RADIUS.into(),
            width: if focused { 2.0 } else { 1.0 },
            color: if focused {
                CREAM
            } else {
                palette::with_alpha(palette::TEXT, 0.15)
            },
        },
        icon: palette::TEXT_MUTED,
        placeholder: palette::TEXT_MUTED,
        value: palette::TEXT,
        selection: palette::with_alpha(ACCENT, 0.4),
    }
}

/// Room around a page's content.
pub const PAGE_PADDING: Padding = Padding {
    top: space::PAGE,
    right: space::PAGE,
    bottom: space::PAGE,
    left: space::PAGE,
};

/// The app's iced theme.
pub fn iced_theme() -> iced::Theme {
    iced::Theme::custom(
        "ARClens".to_owned(),
        iced::theme::Palette {
            background: BG,
            text: palette::TEXT,
            primary: CREAM,
            success: palette::verdict(arclens_core::Verdict::Keep),
            warning: ACCENT,
            danger: Color::from_rgb8(0xe5, 0x48, 0x4d),
        },
    )
}
