//! The Progress tab: where the player is in the game, so advice knows
//! what still matters. One section per part of the game's progression:
//! workshop levels, quests, and projects (expeditions among them).

use crate::app::Message;
use arclens_core::{Item, Progress, Project, Quest, Station};
use arclens_ui::palette::{self, with_alpha};
use arclens_ui::theme::{self, size, space};
use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length};

/// Width of a section's content column.
const CONTENT_WIDTH: f32 = 720.0;
/// Width of the section sidebar.
const SIDEBAR_WIDTH: f32 = 260.0;

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
        .fold(column![theme::label("Progress")].spacing(10), |col, &s| {
            col.push(section_button(s, page.section == s, s.summary(page)))
        })
        .width(SIDEBAR_WIDTH);
    let body = match page.section {
        Section::Workshop => workshop(page),
        Section::Quests => quests(page),
        Section::Projects => projects(page),
        Section::Blueprints => blueprints(page),
    };
    row![
        container(sections).padding(theme::PAGE_PADDING),
        scrollable(container(body).padding(theme::PAGE_PADDING)).height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

/// A section's title and the line explaining it.
fn header<'a>(title: &str, explainer: &'a str) -> Element<'a, Message> {
    column![
        theme::heading(title, size::TITLE),
        text(explainer).size(size::BODY).color(palette::TEXT_MUTED),
    ]
    .spacing(8)
    .into()
}

/// A count for a panel's header bar ("3 / 7").
fn count<'a>(value: String) -> Element<'a, Message> {
    text(value)
        .size(size::BODY)
        .font(theme::DISPLAY)
        .color(theme::INK)
        .into()
}

fn workshop<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let autofill = container(
        column![
            text("FILLS IN FROM THE GAME")
                .size(size::BODY)
                .font(theme::DISPLAY)
                .color(theme::ACCENT),
            text(
                "With game capture on, open a station in the game's Workshop: ARClens reads \
                 its level from the page title and updates it here.",
            )
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
        ]
        .spacing(4),
    )
    .padding([12.0, space::PANEL])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(with_alpha(theme::ACCENT, 0.06).into()),
        border: Border {
            color: with_alpha(theme::ACCENT, 0.4),
            width: 1.0,
            radius: theme::RADIUS.into(),
        },
        ..container::Style::default()
    });

    let rows = page
        .stations
        .iter()
        .fold(column![].spacing(space::GAP), |col, station| {
            col.push(station_row(page, station))
        });

    let status = if page.progress.is_some() {
        "Advice uses these levels."
    } else {
        "Not set yet: advice is based on value only."
    };
    let mut col = column![
        header(
            "Workshop",
            "Your workshop levels tell advice what you still need: upgrades you've built \
             stop counting as reasons to keep an item, and parts for the next ones make \
             recycling worth it.",
        ),
        autofill,
        theme::panel(
            "Stations",
            Some(count(Section::Workshop.summary(page))),
            rows
        ),
        text(status).size(size::SMALL).color(palette::TEXT_MUTED),
    ]
    .spacing(space::SECTION)
    .max_width(CONTENT_WIDTH);
    if page.progress.is_some() {
        col = col.push(row![
            Space::new().width(Length::Fill),
            theme::secondary_button("Forget my progress", Some(Message::ClearProgress)),
        ]);
    }
    col.into()
}

/// One station: its name, a level bar and a stepper.
fn station_row<'a>(page: &ProgressView<'a>, station: &'a Station) -> Element<'a, Message> {
    let level = page.progress.map_or(0, |p| p.level(&station.id));
    let step = |to: u32| Message::SetStationLevel(station.id.clone(), to);
    #[allow(clippy::cast_precision_loss, reason = "levels are small")]
    let share = if station.max_level == 0 {
        0.0
    } else {
        level as f32 / station.max_level as f32
    };
    row![
        column![
            text(&station.name).size(size::BODY).font(theme::STRONG),
            iced::widget::progress_bar(0.0..=1.0, share)
                .girth(6)
                .style(|_| iced::widget::progress_bar::Style {
                    background: with_alpha(palette::TEXT, 0.08).into(),
                    bar: theme::ACCENT.into(),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                }),
        ]
        .spacing(8)
        .width(Length::Fill),
        stepper(
            format!("{level} / {}", station.max_level),
            (level > 0).then(|| step(level - 1)),
            (level < station.max_level).then(|| step(level + 1)),
        ),
    ]
    .spacing(space::GAP)
    .align_y(Alignment::Center)
    .into()
}

/// − value +, the buttons disabled at the ends.
fn stepper<'a>(
    value: String,
    minus: Option<Message>,
    plus: Option<Message>,
) -> Element<'a, Message> {
    row![
        step_button("−", minus),
        text(value)
            .size(size::BODY)
            .font(theme::STRONG)
            .width(76)
            .align_x(Alignment::Center),
        step_button("+", plus),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// A square outlined button for a stepper.
fn step_button(label: &str, on_press: Option<Message>) -> Element<'_, Message> {
    button(
        text(label)
            .size(size::H2)
            .font(theme::STRONG)
            .center()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(36)
    .height(36)
    .padding(0)
    .on_press_maybe(on_press)
    .style(|_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let disabled = matches!(status, button::Status::Disabled);
        button::Style {
            background: Some(with_alpha(palette::TEXT, if hovered { 0.12 } else { 0.04 }).into()),
            text_color: if disabled {
                with_alpha(palette::TEXT, 0.3)
            } else {
                palette::TEXT
            },
            border: Border {
                color: with_alpha(palette::TEXT, if disabled { 0.12 } else { 0.25 }),
                width: 1.0,
                radius: theme::RADIUS.into(),
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// A larger checkbox, in body text.
fn tick<'a>(
    checked: bool,
    label: &'a str,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> iced::widget::Checkbox<'a, Message> {
    checkbox(checked)
        .label(label)
        .on_toggle(on_toggle)
        .size(20)
        .spacing(12)
        .text_size(size::BODY)
}

/// Quests per trader, in the order the game gives them. Ticking one also
/// ticks the quests before it; unticking unticks the ones after.
fn quests<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let mut groups = column![].spacing(space::SECTION);
    let mut traders: Vec<&str> = page.quests.iter().map(|q| q.trader.as_str()).collect();
    traders.dedup();
    for trader in traders {
        let (mut done_count, mut total) = (0, 0);
        let list = page.quests.iter().filter(|q| q.trader == trader).fold(
            column![].spacing(10),
            |col, quest| {
                let done = page.progress.is_some_and(|p| p.quest_done(&quest.id));
                total += 1;
                done_count += usize::from(done);
                let id = quest.id.clone();
                let mut line = row![tick(done, quest.name.as_str(), move |done| {
                    Message::SetQuestDone(id.clone(), done)
                })]
                .spacing(10)
                .align_y(Alignment::Center);
                if quest.needs_items {
                    line = line.push(tag("needs items"));
                }
                col.push(line)
            },
        );
        let title = if trader.is_empty() { "Other" } else { trader };
        groups = groups.push(theme::panel(
            title,
            Some(count(format!("{done_count} / {total}"))),
            list,
        ));
    }
    column![
        header(
            "Quests",
            "Tick the quests you've finished: items they asked for stop counting as reasons \
             to keep. Ticking a quest also ticks the ones before it.",
        ),
        groups,
    ]
    .spacing(space::SECTION)
    .max_width(CONTENT_WIDTH)
    .into()
}

/// Projects and expeditions, one phase stepper each.
fn projects<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let rows = page
        .projects
        .iter()
        .fold(column![].spacing(space::GAP + 4.0), |col, project| {
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
                        text(&project.name).size(size::BODY).font(theme::STRONG),
                        text(next).size(size::SMALL).color(palette::TEXT_MUTED),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    stepper(
                        format!("{done} / {total}"),
                        (done > 0).then(|| step(done - 1)),
                        (done < total).then(|| step(done + 1)),
                    ),
                ]
                .spacing(space::GAP)
                .align_y(Alignment::Center),
            )
        });
    column![
        header(
            "Projects",
            "Set how many phases of each project you've delivered: items for finished \
             phases stop counting as reasons to keep.",
        ),
        theme::panel(
            "Projects",
            Some(count(Section::Projects.summary(page))),
            rows
        ),
    ]
    .spacing(space::SECTION)
    .max_width(CONTENT_WIDTH)
    .into()
}

/// Blueprints, ticked once learned: until then advice says LEARN.
fn blueprints<'a>(page: &ProgressView<'a>) -> Element<'a, Message> {
    let list = page
        .blueprints
        .iter()
        .fold(column![].spacing(10), |col, blueprint| {
            let learned = page
                .progress
                .is_some_and(|p| p.blueprint_learned(&blueprint.id));
            let id = blueprint.id.clone();
            col.push(tick(learned, blueprint.name.as_str(), move |learned| {
                Message::SetBlueprintLearned(id.clone(), learned)
            }))
        });
    column![
        header(
            "Blueprints",
            "Blueprints you haven't learned show LEARN instead of a price. Tick the ones you \
             know: a duplicate is then just worth its price.",
        ),
        theme::panel(
            "Blueprints",
            Some(count(Section::Blueprints.summary(page))),
            list
        ),
    ]
    .spacing(space::SECTION)
    .max_width(CONTENT_WIDTH)
    .into()
}

fn tag(label: &str) -> Element<'_, Message> {
    container(
        text(label.to_uppercase())
            .size(size::TINY)
            .font(theme::DISPLAY_SEMI)
            .color(palette::TEXT_MUTED),
    )
    .padding([2, 8])
    .style(|_| container::Style {
        border: Border {
            color: palette::BORDER,
            width: 1.0,
            radius: theme::RADIUS.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A sidebar tile; the selected one cream, like the game's selected tiles.
fn section_button<'a>(section: Section, active: bool, summary: String) -> Element<'a, Message> {
    let (title_color, summary_color) = if active {
        (theme::INK, with_alpha(theme::INK, 0.7))
    } else {
        (palette::TEXT, palette::TEXT_MUTED)
    };
    button(
        column![
            text(section.title().to_uppercase())
                .size(size::H2)
                .font(theme::DISPLAY)
                .color(title_color),
            text(summary).size(size::SMALL).color(summary_color),
        ]
        .spacing(2),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .on_press(Message::SetProgressSection(section))
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(if active {
                theme::CREAM.into()
            } else if hovered {
                theme::PANEL_RAISED.into()
            } else {
                theme::PANEL.into()
            }),
            text_color: if active { theme::INK } else { palette::TEXT },
            border: Border {
                color: if active {
                    theme::CREAM
                } else {
                    palette::BORDER
                },
                width: 1.0,
                radius: theme::RADIUS.into(),
            },
            ..button::Style::default()
        }
    })
    .into()
}
