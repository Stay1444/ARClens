//! The Progress tab: where the player is in the game, so advice knows
//! what still matters. One section per part of the game's progression:
//! workshop levels, quests, and projects (expeditions among them).

use crate::app::Message;
use arclens_core::{Item, Progress, Project, Quest, Station};
use arclens_ui::palette::{self, with_alpha};
use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text};
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
    Quests,
    Projects,
    Blueprints,
}

impl Section {
    pub const ALL: [Self; 4] = [
        Self::Workshop,
        Self::Quests,
        Self::Projects,
        Self::Blueprints,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Workshop => "Workshop",
            Self::Quests => "Quests",
            Self::Projects => "Projects",
            Self::Blueprints => "Blueprints",
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
            Self::Quests => {
                let done = page
                    .quests
                    .iter()
                    .filter(|q| page.progress.is_some_and(|p| p.quest_done(&q.id)))
                    .count();
                format!("{done} of {} done", page.quests.len())
            }
            Self::Projects => {
                let (done, total) = page.projects.iter().fold((0, 0), |(d, t), project| {
                    let phases = u32::try_from(project.phases.len()).unwrap_or(u32::MAX);
                    let done = page.progress.map_or(0, |p| p.phases_done(&project.id));
                    (d + done.min(phases), t + phases)
                });
                format!("{done} of {total} phases")
            }
            Self::Blueprints => {
                let learned = page
                    .blueprints
                    .iter()
                    .filter(|b| page.progress.is_some_and(|p| p.blueprint_learned(&b.id)))
                    .count();
                format!("{learned} of {} learned", page.blueprints.len())
            }
        }
    }
}

pub struct ProgressView<'a> {
    pub section: Section,
    pub stations: &'a [Station],
    pub quests: &'a [Quest],
    pub projects: &'a [Project],
    /// Blueprint items, sorted by name.
    pub blueprints: Vec<&'a Item>,
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
        Section::Quests => quests(page),
        Section::Projects => projects(page),
        Section::Blueprints => blueprints(page),
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

/// Quests per trader, in the order the game gives them. Ticking one also
/// ticks the quests before it; unticking unticks the ones after.
fn quests<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let explainer = text(
        "Tick the quests you've finished: items they asked for stop counting as reasons \
         to keep. Ticking a quest also ticks the ones before it.",
    )
    .size(13)
    .color(palette::TEXT_MUTED);
    let mut groups = column![].spacing(20);
    let mut traders: Vec<&str> = page.quests.iter().map(|q| q.trader.as_str()).collect();
    traders.dedup();
    for trader in traders {
        let list = page.quests.iter().filter(|q| q.trader == trader).fold(
            column![].spacing(6),
            |col, quest| {
                let done = page.progress.is_some_and(|p| p.quest_done(&quest.id));
                let id = quest.id.clone();
                let mut line = row![
                    checkbox(done)
                        .label(quest.name.as_str())
                        .on_toggle(move |done| Message::SetQuestDone(id.clone(), done))
                        .size(16)
                        .text_size(14),
                ]
                .spacing(8)
                .align_y(Alignment::Center);
                if quest.needs_items {
                    line = line.push(tag("needs items"));
                }
                col.push(line)
            },
        );
        let title = if trader.is_empty() { "Other" } else { trader };
        groups = groups.push(column![text(title).size(16).font(BOLD), list].spacing(8));
    }
    column![text("Quests").size(24).font(BOLD), explainer, groups,]
        .spacing(16)
        .max_width(640)
        .into()
}

/// Projects and expeditions, one phase stepper each.
fn projects<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let explainer = text(
        "Set how many phases of each project you've delivered: items for finished \
         phases stop counting as reasons to keep.",
    )
    .size(13)
    .color(palette::TEXT_MUTED);
    let rows = page
        .projects
        .iter()
        .fold(column![].spacing(14), |col, project| {
            let total = u32::try_from(project.phases.len()).unwrap_or(u32::MAX);
            let done = page
                .progress
                .map_or(0, |p| p.phases_done(&project.id))
                .min(total);
            let step = |to: u32| Message::SetProjectPhases(project.id.clone(), to);
            let next = usize::try_from(done)
                .ok()
                .and_then(|i| project.phases.get(i))
                .map_or_else(|| "All phases done".to_owned(), |p| format!("Next: {p}"));
            col.push(
                row![
                    column![
                        text(&project.name).size(15),
                        text(next).size(12).color(palette::TEXT_MUTED),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    button(text("−").size(15))
                        .on_press_maybe((done > 0).then(|| step(done - 1)))
                        .padding([2, 12]),
                    text(format!("{done} / {total}"))
                        .size(15)
                        .width(70)
                        .align_x(Alignment::Center),
                    button(text("+").size(15))
                        .on_press_maybe((done < total).then(|| step(done + 1)))
                        .padding([2, 12]),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
        });
    column![text("Projects").size(24).font(BOLD), explainer, rows]
        .spacing(16)
        .max_width(640)
        .into()
}

/// Blueprints, ticked once learned: until then advice says LEARN.
fn blueprints<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let explainer = text(
        "Blueprints you haven't learned show LEARN instead of a price. Tick the ones you \
         know: a duplicate is then just worth its price.",
    )
    .size(13)
    .color(palette::TEXT_MUTED);
    let list = page
        .blueprints
        .iter()
        .fold(column![].spacing(6), |col, blueprint| {
            let learned = page
                .progress
                .is_some_and(|p| p.blueprint_learned(&blueprint.id));
            let id = blueprint.id.clone();
            col.push(
                checkbox(learned)
                    .label(blueprint.name.as_str())
                    .on_toggle(move |learned| Message::SetBlueprintLearned(id.clone(), learned))
                    .size(16)
                    .text_size(14),
            )
        });
    column![text("Blueprints").size(24).font(BOLD), explainer, list]
        .spacing(16)
        .max_width(640)
        .into()
}

fn tag(label: &str) -> Element<'_, Message> {
    container(text(label).size(10).color(palette::TEXT_MUTED))
        .padding([1, 6])
        .style(|_| container::Style {
            border: Border {
                color: palette::BORDER,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
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
