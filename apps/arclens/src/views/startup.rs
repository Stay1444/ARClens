//! The start-up screen: the logo and what the app is getting ready (game
//! data, the condition schedule, the overlay, game capture), shown until
//! the game data is in. Any key or "Skip" goes straight to the app.

use crate::app::Message;
use arclens_i18n::t;
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, column, container, row, svg, text};
use iced::{Alignment, Color, Element, Length};
use std::time::{Duration, Instant};

/// Shown at least this long, so it reads as a screen rather than a flash.
const MIN_SHOWN: Duration = Duration::from_millis(1100);
/// Never longer: a first download can take a while, and the tabs say
/// "loading" themselves.
const MAX_SHOWN: Duration = Duration::from_secs(8);
/// Animation frame interval while shown.
pub const TICK: Duration = Duration::from_millis(120);

/// Where one step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Working,
    Done,
    /// Failed or unavailable; the app works without it.
    Failed,
    /// Off by the user's choice or not applicable.
    Idle,
}

/// One line: what, how far, and a detail ("581 items").
#[derive(Debug, Clone)]
pub struct Step {
    pub label: String,
    pub state: StepState,
    pub detail: Option<String>,
}

/// The start-up screen's clock.
#[derive(Debug, Clone, Copy)]
pub struct Startup {
    started: Instant,
    /// Animation frame.
    pub frame: u32,
}

impl Startup {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            frame: 0,
        }
    }

    /// Whether to leave the screen: the data has settled (loaded or
    /// failed) and it was shown long enough, or it was shown too long.
    pub fn finished(&self, data_settled: bool, now: Instant) -> bool {
        let shown = now.saturating_duration_since(self.started);
        (data_settled && shown >= MIN_SHOWN) || shown >= MAX_SHOWN
    }
}

pub struct StartupView<'a> {
    pub logo: &'a svg::Handle,
    pub steps: Vec<Step>,
    pub frame: u32,
}

pub fn view<'a>(page: &StartupView<'a>) -> Element<'a, Message> {
    let title = row![
        svg(page.logo.clone()).width(72).height(72),
        column![
            text("ARCLENS").size(size::TITLE * 1.4).font(theme::DISPLAY),
            text(t!("startup-tagline"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        ]
        .spacing(2),
    ]
    .spacing(18)
    .align_y(Alignment::Center);

    let steps = page.steps.iter().fold(column![].spacing(10), |col, step| {
        col.push(step_line(step, page.frame))
    });

    let skip = theme::secondary_button(&t!("startup-skip"), Some(Message::SkipStartup));
    let version = text(format!("v{}", env!("CARGO_PKG_VERSION")))
        .size(size::TINY)
        .color(palette::TEXT_MUTED);

    let card = column![
        title,
        theme::stripe_band(4.0),
        steps,
        row![version, Space::new().width(Length::Fill), skip].align_y(Alignment::Center),
    ]
    .spacing(space::SECTION)
    .width(460);

    container(card)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_| container::Style {
            background: Some(theme::BG.into()),
            text_color: Some(palette::TEXT),
            ..container::Style::default()
        })
        .into()
}

/// `◐ Game data … 581 items`, the mark animated while working.
fn step_line<'a>(step: &Step, frame: u32) -> Element<'a, Message> {
    const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];
    let (mark, color): (&str, Color) = match step.state {
        StepState::Working => (SPINNER[frame as usize % SPINNER.len()], theme::ACCENT),
        StepState::Done => ("✓", palette::verdict(arclens_core::Verdict::Keep)),
        StepState::Failed => ("!", palette::verdict(arclens_core::Verdict::Sell)),
        StepState::Idle => ("–", palette::TEXT_MUTED),
    };
    let mut line = row![
        container(text(mark).size(size::BODY).font(theme::STRONG).color(color)).width(22),
        text(step.label.clone())
            .size(size::BODY)
            .font(theme::STRONG)
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(detail) = &step.detail {
        line = line.push(
            text(detail.clone())
                .size(size::SMALL)
                .color(with_alpha(palette::TEXT_MUTED, 0.95)),
        );
    }
    line.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_long_enough_but_not_forever() {
        let t0 = Instant::now();
        let s = Startup::new(t0);
        assert!(!s.finished(true, t0));
        assert!(s.finished(true, t0 + MIN_SHOWN));
        assert!(!s.finished(false, t0 + MIN_SHOWN * 2));
        assert!(s.finished(false, t0 + MAX_SHOWN));
    }
}
