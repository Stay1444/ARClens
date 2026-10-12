//! The Progress tab's Planner: goals chosen all at once and in no order
//! (workshop stations, projects and expeditions, items to craft) broken
//! down to materials together, against the latest stash scan: what the
//! stash covers, what to craft from parts, what is missing, and which spare
//! items would recycle into the missing parts.

use crate::app::Message;
use crate::icons::Icons;
use crate::plan_goals::PlanGoals;
use arclens_core::{ItemId, Plan, PlanLine};
use arclens_data::Catalog;
use arclens_i18n::t;
use arclens_ui::detail::{self, titled};
use arclens_ui::palette;
use arclens_ui::theme::{self, size, space};
use iced::widget::{column, row, text};
use iced::{Alignment, Element, Length};

/// A station or project to choose: id, name, how far along, how far it
/// goes.
pub struct Track<'a> {
    pub id: &'a str,
    pub name: std::borrow::Cow<'a, str>,
    pub done: u32,
    pub total: u32,
}

pub struct PlannerView<'a> {
    pub goals: &'a PlanGoals,
    pub stations: Vec<Track<'a>>,
    /// Projects and expeditions.
    pub projects: Vec<Track<'a>>,
    /// The plan for every goal together (`None` when there are none).
    pub plan: Option<Plan>,
    pub catalog: &'a Catalog,
    pub icons: &'a Icons,
    /// The latest stash scan's date, if any.
    pub scanned: Option<String>,
}

pub fn summary(page: &PlannerView<'_>) -> String {
    match &page.plan {
        Some(plan) if plan.missing() == 0 => t!("planner-summary-ready"),
        Some(plan) => t!("planner-summary-missing", count = plan.missing()),
        None => t!("planner-summary-none"),
    }
}

pub fn view<'a>(page: &PlannerView<'a>, header: Element<'a, Message>) -> Element<'a, Message> {
    let mut body = column![
        header,
        goal_picker(page),
        text(match &page.scanned {
            Some(date) => t!("planner-stash-from", date = date.as_str()),
            None => t!("planner-no-scan"),
        })
        .size(size::SMALL)
        .color(palette::TEXT_MUTED),
    ]
    .spacing(space::SECTION);
    if let Some(plan) = &page.plan {
        body = body.push(materials(page, plan));
        if !plan.recycle.is_empty() {
            body = body.push(recycling(page, plan));
        }
    }
    body.max_width(860.0).into()
}

/// The goals: station and project chips, how far, items to craft.
fn goal_picker<'a>(page: &PlannerView<'a>) -> Element<'a, Message> {
    let goals = page.goals;
    let set = |next: PlanGoals| Message::SetPlanGoals(next);
    let reach = row![
        theme::chip(
            &t!("planner-reach-next"),
            !goals.to_max,
            set(goals.with_to_max(false))
        ),
        theme::chip(
            &t!("planner-reach-max"),
            goals.to_max,
            set(goals.with_to_max(true))
        ),
    ]
    .spacing(4);

    // Workshop: every station not maxed, and "all".
    let open: Vec<&str> = page
        .stations
        .iter()
        .filter(|s| s.done < s.total)
        .map(|s| s.id)
        .collect();
    let all_on = !open.is_empty() && open.iter().all(|id| goals.stations.contains(*id));
    let stations = page.stations.iter().filter(|s| s.done < s.total).fold(
        row![theme::chip(
            &t!("planner-all-benches"),
            all_on,
            set(goals.toggle_all_stations(open.iter().copied())),
        )]
        .spacing(4),
        |r, s| {
            r.push(theme::chip(
                &t!(
                    "planner-station-chip",
                    station = s.name.as_ref(),
                    level = if goals.to_max { s.total } else { s.done + 1 }
                ),
                goals.stations.contains(s.id),
                set(goals.toggle_station(s.id)),
            ))
        },
    );
    let projects =
        page.projects
            .iter()
            .filter(|p| p.done < p.total)
            .fold(row![].spacing(4), |r, p| {
                r.push(theme::chip(
                    &t!(
                        "planner-project-chip",
                        project = p.name.as_ref(),
                        phase = if goals.to_max { p.total } else { p.done + 1 },
                        total = p.total
                    ),
                    goals.projects.contains(p.id),
                    set(goals.toggle_project(p.id)),
                ))
            });
    let crafts = goals
        .crafts
        .iter()
        .fold(column![].spacing(6), |col, (item, &count)| {
            col.push(craft_row(page, item, count))
        });

    let mut choose = column![
        titled(
            &t!("planner-workshop"),
            stations.wrap().vertical_spacing(4).into()
        ),
        titled(
            &t!("planner-projects"),
            projects.wrap().vertical_spacing(4).into()
        ),
        titled(
            &t!("planner-crafting"),
            if goals.crafts.is_empty() {
                text(t!("planner-crafting-help"))
                    .size(size::SMALL)
                    .color(palette::TEXT_MUTED)
                    .into()
            } else {
                crafts.into()
            }
        ),
        row![
            text(t!("planner-reach"))
                .size(size::SMALL)
                .color(palette::TEXT_MUTED),
            reach
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(16);
    if !goals.is_empty() {
        choose = choose.push(theme::secondary_button(
            &t!("planner-clear"),
            Some(set(PlanGoals::default())),
        ));
    }
    choose.into()
}

/// `Anvil I ×2  − + ✕`.
fn craft_row<'a>(page: &PlannerView<'a>, item: &ItemId, count: u32) -> Element<'a, Message> {
    let set = |n: u32| Message::SetPlanGoals(page.goals.with_craft(item, n));
    row![
        text(t!("planner-goal-craft", item = name(page.catalog, item)))
            .size(size::BODY)
            .width(Length::Fill),
        theme::secondary_button("−", Some(set(count.saturating_sub(1).max(1)))),
        text(format!("×{count}"))
            .size(size::BODY)
            .font(theme::DISPLAY),
        theme::secondary_button("+", Some(set(count + 1))),
        theme::secondary_button("✕", Some(set(0))),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn name(catalog: &Catalog, id: &arclens_core::ItemId) -> String {
    catalog
        .item(id)
        .map_or_else(|| id.to_string(), |i| i.name.clone())
}

fn materials<'a>(page: &PlannerView<'a>, plan: &Plan) -> Element<'a, Message> {
    if plan.lines.is_empty() {
        return text(t!("planner-nothing"))
            .size(size::BODY)
            .color(palette::TEXT_MUTED)
            .into();
    }
    let list = plan.lines.iter().fold(column![].spacing(4), |col, line| {
        col.push(material(page, line))
    });
    let status = if plan.missing() == 0 {
        t!("planner-ready")
    } else {
        t!("planner-missing", count = plan.missing())
    };
    theme::panel(
        &t!("planner-materials"),
        Some(
            text(status)
                .size(size::BODY)
                .font(theme::DISPLAY)
                .color(theme::INK)
                .into(),
        ),
        list,
    )
}

/// `Metal Parts ×8 · 3 in stash · craft 0 · 5 missing`, parts indented.
fn material<'a>(page: &PlannerView<'a>, line: &PlanLine) -> Element<'a, Message> {
    let mut parts = vec![t!("planner-in-stash", count = line.from_stash)];
    if line.crafted > 0 {
        parts.push(t!("planner-craft", count = line.crafted));
    }
    if line.missing > 0 {
        parts.push(t!("planner-to-find", count = line.missing));
    }
    let note = parts.join(" · ");
    let entry = detail::line(detail::Line {
        icon: page.icons.get(&line.item).map(|i| &i.thumb),
        name: &name(page.catalog, &line.item),
        quantity: Some(line.needed),
        note: Some(&note),
        done: line.missing == 0 && line.crafted == 0,
    });
    let indent = f32::from(line.depth) * 24.0;
    let entry = iced::widget::button(entry)
        .padding(iced::Padding::from([3, 6]).left(6.0 + indent))
        .width(Length::Fill)
        .on_press(Message::Select(line.item.clone()))
        .style(|_, status| iced::widget::button::Style {
            background: matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            )
            .then(|| palette::with_alpha(palette::TEXT, 0.08).into()),
            text_color: palette::TEXT,
            ..iced::widget::button::Style::default()
        });
    entry.into()
}

fn recycling<'a>(page: &PlannerView<'a>, plan: &Plan) -> Element<'a, Message> {
    let list = plan.recycle.iter().fold(column![].spacing(4), |col, hint| {
        let gives = hint
            .gives
            .iter()
            .map(|q| format!("{} ×{}", name(page.catalog, &q.item), q.quantity))
            .collect::<Vec<_>>()
            .join(", ");
        col.push(detail::line(detail::Line {
            icon: page.icons.get(&hint.from).map(|i| &i.thumb),
            name: &name(page.catalog, &hint.from),
            quantity: Some(hint.count),
            note: Some(&gives),
            done: false,
        }))
    });
    column![
        text(t!("planner-recycle-help"))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
        titled(&t!("planner-recycle"), list.into()),
    ]
    .spacing(8)
    .into()
}
