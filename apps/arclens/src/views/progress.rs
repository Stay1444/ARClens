//! The Progress tab: where the player is in the game, so advice knows
//! what still matters. Organised in sections; the workshop is the first,
//! and others (projects, expeditions, quests) slot in as more sections.

use crate::app::Message;
use arclens_core::{Progress, Station};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Font, Length, font};

const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

/// A part of the game's progression the page tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Workshop,
}

impl Section {
    pub const ALL: [Self; 1] = [Self::Workshop];

    fn title(self) -> &'static str {
        match self {
            Self::Workshop => "Workshop",
        }
    }

    fn summary(self, page: &ProgressView<'_>) -> String {
        match self {
            Self::Workshop => {
                let (built, total) = page.stations.iter().fold((0, 0), |(b, t), s| {
                    let level = page.progress.map_or(0, |p| p.level(&s.id));
                    (b + level.min(s.max_level), t + s.max_level)
                });
                format!("{built} of {total} levels")
            }
        }
    }
}

pub struct ProgressView<'a> {
    pub section: Section,
    pub stations: &'a [Station],
    /// `None`: never set.
    pub progress: Option<&'a Progress>,
}

pub fn view<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let sections = Section::ALL
        .iter()
        .fold(
            column![text("PROGRESS").size(11).color(palette::TEXT_MUTED)].spacing(6),
            |col, &s| col.push(section_button(s, page.section == s, s.summary(page))),
        )
        .width(220);
    let body = match page.section {
        Section::Workshop => workshop(page),
    };
    row![
        container(sections).padding(24),
        scrollable(container(body).padding(24)).height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn workshop<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let explainer = text(
        "Your workshop levels tell advice what you still need: upgrades you've built \
         stop counting as reasons to keep an item, and parts for the next ones make \
         recycling worth it.",
    )
    .size(13)
    .color(palette::TEXT_MUTED);
    let autofill = container(
        column![
            text("Fills in from the game").size(13).font(BOLD),
            text(
                "With game capture on, open a station in the game's Workshop: ARClens reads \
                 its level from the page title and updates it here.",
            )
            .size(12)
            .color(palette::TEXT_MUTED),
        ]
        .spacing(4),
    )
    .padding(12)
    .style(|_| container::Style {
        background: Some(with_alpha(palette::TEXT, 0.05).into()),
        border: Border {
            color: palette::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    });

    let rows = page
        .stations
        .iter()
        .fold(column![].spacing(10), |col, station| {
            let level = page.progress.map_or(0, |p| p.level(&station.id));
            let step = |to: u32| Message::SetStationLevel(station.id.clone(), to);
            #[allow(clippy::cast_precision_loss, reason = "levels are small")]
            let share = if station.max_level == 0 {
                0.0
            } else {
                level as f32 / station.max_level as f32
            };
            col.push(
                row![
                    column![
                        text(&station.name).size(15),
                        iced::widget::progress_bar(0.0..=1.0, share)
                            .girth(4)
                            .style(|_| iced::widget::progress_bar::Style {
                                background: with_alpha(palette::TEXT, 0.08).into(),
                                bar: palette::verdict(arclens_core::Verdict::Keep).into(),
                                border: Border {
                                    radius: 2.0.into(),
                                    ..Border::default()
                                },
                            }),
                    ]
                    .spacing(6)
                    .width(Length::Fill),
                    button(text("−").size(15))
                        .on_press_maybe((level > 0).then(|| step(level - 1)))
                        .padding([2, 12]),
                    text(format!("{level} / {}", station.max_level))
                        .size(15)
                        .width(70)
                        .align_x(Alignment::Center),
                    button(text("+").size(15))
                        .on_press_maybe((level < station.max_level).then(|| step(level + 1)))
                        .padding([2, 12]),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
        });

    let status = if page.progress.is_some() {
        "Advice uses these levels."
    } else {
        "Not set yet: advice is based on value only."
    };
    let mut col = column![
        text("Workshop").size(24).font(BOLD),
        explainer,
        autofill,
        rows,
        text(status).size(12).color(palette::TEXT_MUTED),
    ]
    .spacing(16)
    .max_width(640);
    if page.progress.is_some() {
        col = col.push(row![
            Space::new().width(Length::Fill),
            button(text("Forget my progress").size(13)).on_press(Message::ClearProgress)
        ]);
    }
    col.into()
}

fn section_button<'a>(section: Section, active: bool, summary: String) -> Element<'a, Message> {
    button(
        column![
            text(section.title()).size(14).font(BOLD),
            text(summary).size(12).color(palette::TEXT_MUTED),
        ]
        .spacing(2),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .on_press(Message::SetProgressSection(section))
    .style(move |_, status| button::Style {
        background: Some(
            with_alpha(
                palette::TEXT,
                if active {
                    0.12
                } else if matches!(status, button::Status::Hovered) {
                    0.07
                } else {
                    0.03
                },
            )
            .into(),
        ),
        text_color: palette::TEXT,
        border: Border {
            color: palette::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    })
    .into()
}
