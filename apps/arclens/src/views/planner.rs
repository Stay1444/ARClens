//! The Progress tab's Planner: a goal (a station's next level, or crafting
//! an item) broken down to materials, against the latest stash scan: what
//! the stash covers, what to craft from parts, what is missing, and which
//! spare items would recycle into the missing parts.

use crate::app::Message;
use crate::icons::Icons;
use arclens_core::{Goal, Plan, PlanLine};
use arclens_data::Catalog;
use arclens_i18n::t;
use arclens_ui::detail::{self, titled};
use arclens_ui::palette;
use arclens_ui::theme::{self, size, space};
use iced::widget::{column, pick_list, row, text};
use iced::{Alignment, Element, Length};
use std::fmt;

/// A goal in the picker, with its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalChoice {
    pub goal: Goal,
    pub label: String,
}

impl fmt::Display for GoalChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

pub struct PlannerView<'a> {
    /// Each station's next level, and the craft goal if one is set.
    pub choices: Vec<GoalChoice>,
    pub goal: Option<&'a Goal>,
    /// The plan for `goal`.
    pub plan: Option<Plan>,
    pub catalog: &'a Catalog,
    pub icons: &'a Icons,
    /// The latest stash scan's date, if any.
    pub scanned: Option<String>,
}

pub fn summary(page: &PlannerView<'_>) -> String {
    match (&page.goal, &page.plan) {
        (Some(_), Some(plan)) if plan.missing() == 0 => t!("planner-summary-ready"),
        (Some(_), Some(plan)) => t!("planner-summary-missing", count = plan.missing()),
        _ => t!("planner-summary-none"),
    }
}

pub fn view<'a>(page: &PlannerView<'a>, header: Element<'a, Message>) -> Element<'a, Message> {
    let selected = page
        .goal
        .and_then(|g| page.choices.iter().find(|c| &c.goal == g))
        .cloned();
    let picker = pick_list(page.choices.clone(), selected, |c: GoalChoice| {
        Message::SetPlanGoal(c.goal)
    })
    .placeholder(t!("planner-pick"))
    .text_size(size::BODY)
    .padding([8, 12])
    .width(Length::Fill);
    let mut picker_row = row![picker].spacing(8).align_y(Alignment::Center);
    if let Some(Goal::Craft { item, count }) = page.goal {
        let step = |n: u32| {
            Message::SetPlanGoal(Goal::Craft {
                item: item.clone(),
                count: n.max(1),
            })
        };
        picker_row = picker_row
            .push(theme::secondary_button(
                "−",
                Some(step(count.saturating_sub(1))),
            ))
            .push(
                text(format!("×{count}"))
                    .size(size::BODY)
                    .font(theme::DISPLAY),
            )
            .push(theme::secondary_button("+", Some(step(count + 1))));
    }

    let mut body = column![
        header,
        picker_row,
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
    body.max_width(760.0).into()
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
