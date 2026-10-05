//! The Progress tab's Stash section: the latest stash scan, what the
//! player still needs against what they have, and past scans.

use crate::app::Message;
use crate::stash::Snapshot;
use crate::stash_worker::ScanState;
use arclens_core::{Item, ItemId};
use arclens_i18n::t;
use arclens_ui::format::thousands;
use arclens_ui::palette;
use arclens_ui::theme::{self, size, space};
use iced::widget::{column, row, text};
use iced::{Element, Length};

/// What still-needed item the player has how many of.
pub struct Shortfall<'a> {
    pub item: &'a Item,
    pub have: u32,
    pub needed: u32,
}

pub struct StashView<'a> {
    /// Finished scans, oldest first.
    pub history: &'a [Snapshot],
    /// The scan in progress, if one runs.
    pub live: Option<&'a ScanState>,
    /// Items the remaining progress needs, missing ones first.
    pub needs: Vec<Shortfall<'a>>,
    pub catalog: &'a arclens_data::Catalog,
}

impl StashView<'_> {
    fn name(&self, id: &ItemId) -> String {
        self.catalog
            .item(id)
            .map_or_else(|| id.as_str().to_owned(), |i| i.name.clone())
    }

    /// A snapshot's value at the dataset's prices.
    fn value(&self, scan: &Snapshot) -> u64 {
        scan.value(|id| self.catalog.item(id).and_then(|i| i.value))
    }
}

/// "2026-10-05 11:37" in local time.
pub fn when(unix: i64) -> String {
    jiff::Timestamp::from_second(unix)
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// The sidebar line.
pub fn summary(page: &StashView<'_>) -> String {
    page.history.last().map_or_else(
        || t!("stash-never"),
        |s| t!("stash-summary", slots = s.slots, date = when(s.taken_at)),
    )
}

fn value_text(v: u64) -> String {
    thousands(u32::try_from(v).unwrap_or(u32::MAX))
}

pub fn view<'a>(page: &StashView<'a>, header: Element<'a, Message>) -> Element<'a, Message> {
    let mut body = column![header].spacing(space::SECTION);
    if let Some(live) = page.live.filter(|l| !l.complete) {
        let seen = live.snapshot.slots;
        body = body.push(
            text(match live.used {
                Some(used) => t!("stash-scanning", seen = seen, used = used),
                None => t!("stash-scanning-unknown", seen = seen),
            })
            .size(size::BODY)
            .color(palette::TEXT_MUTED),
        );
    }
    body = body.push(latest(page));
    body = body.push(needed(page));
    if page.history.len() > 1 {
        body = body.push(history(page));
    }
    body.max_width(720.0).into()
}

fn latest<'a>(page: &StashView<'a>) -> Element<'a, Message> {
    let Some(scan) = page.history.last() else {
        return theme::panel(
            &t!("stash-latest"),
            None,
            text(t!("stash-never-help"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED),
        );
    };
    let mut lines = column![
        text(t!(
            "stash-scan-line",
            date = when(scan.taken_at),
            slots = scan.slots,
            value = value_text(page.value(scan))
        ))
        .size(size::BODY)
    ]
    .spacing(6);
    if scan.likely > 0 {
        lines = lines.push(
            text(t!("stash-likely", count = scan.likely))
                .size(size::SMALL)
                .color(palette::TEXT_MUTED),
        );
    }
    theme::panel(&t!("stash-latest"), None, lines)
}

fn needed<'a>(page: &StashView<'a>) -> Element<'a, Message> {
    let missing = page.needs.iter().filter(|n| n.have < n.needed).count();
    let list = if page.needs.is_empty() {
        column![
            text(t!("stash-needed-none"))
                .size(size::BODY)
                .color(palette::TEXT_MUTED)
        ]
    } else {
        page.needs.iter().fold(column![].spacing(8), |col, need| {
            let short = need.have < need.needed;
            let colour = if short {
                palette::verdict(arclens_core::Verdict::Sell)
            } else {
                palette::verdict(arclens_core::Verdict::Keep)
            };
            col.push(
                row![
                    text(need.item.name.clone())
                        .size(size::BODY)
                        .width(Length::Fill),
                    text(t!("stash-have-need", have = need.have, need = need.needed))
                        .size(size::BODY)
                        .font(theme::DISPLAY)
                        .color(colour),
                ]
                .spacing(12),
            )
        })
    };
    column![
        text(t!("stash-needed-help"))
            .size(size::SMALL)
            .color(palette::TEXT_MUTED),
        theme::panel(
            &t!("stash-needed"),
            Some(
                text(t!("stash-missing", count = missing))
                    .size(size::BODY)
                    .font(theme::DISPLAY)
                    .color(theme::INK)
                    .into()
            ),
            list
        ),
    ]
    .spacing(8)
    .into()
}

/// Past scans, newest first, each with its biggest changes.
fn history<'a>(page: &StashView<'a>) -> Element<'a, Message> {
    let scans = page.history;
    let list = (1..scans.len())
        .rev()
        .take(10)
        .fold(column![].spacing(12), |col, i| {
            let (scan, before) = (&scans[i], &scans[i - 1]);
            let changes = scan.changes_since(before);
            let shown: Vec<String> = changes
                .iter()
                .take(5)
                .map(|(id, d)| format!("{}{d} {}", if *d > 0 { "+" } else { "" }, page.name(id)))
                .collect();
            let line = if shown.is_empty() {
                t!("stash-no-changes")
            } else {
                let more = changes.len().saturating_sub(shown.len());
                let mut joined = shown.join(", ");
                if more > 0 {
                    joined.push_str(&t!("stash-more", count = more));
                }
                joined
            };
            col.push(
                column![
                    text(t!(
                        "stash-scan-line",
                        date = when(scan.taken_at),
                        slots = scan.slots,
                        value = value_text(page.value(scan))
                    ))
                    .size(size::BODY),
                    text(line).size(size::SMALL).color(palette::TEXT_MUTED),
                ]
                .spacing(2),
            )
        });
    theme::panel(&t!("stash-history"), None, list)
}
