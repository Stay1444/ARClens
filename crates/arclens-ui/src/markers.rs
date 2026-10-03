//! Map-marker icons: our own glyphs (in `assets/markers/`), picked by
//! keywords in the marker's subcategory or category, drawn white on the
//! category colour.

use crate::palette;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{container, svg};
use iced::{Border, Element, Length, Point, Rectangle, Size};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Boss,
    Arc,
    Container,
    Weapon,
    Medical,
    Nature,
    Quest,
    Extraction,
    Hatch,
    Event,
    Place,
    Key,
    Spawn,
    Other,
}

/// Keywords per glyph, most specific first: the first glyph with a keyword
/// contained in the name wins.
const KEYWORDS: &[(Glyph, &[&str])] = &[
    (
        Glyph::Boss,
        &["queen", "matriarch", "boss", "bastion", "bombardier"],
    ),
    (Glyph::Hatch, &["hatch"]),
    (
        Glyph::Extraction,
        &["extract", "exit", "elevator", "train", "metro", "lift"],
    ),
    (Glyph::Key, &["key", "door", "lock"]),
    (
        Glyph::Medical,
        &["medic", "med_", "med-", "health", "first_aid", "first aid"],
    ),
    (
        Glyph::Weapon,
        &["weapon", "gun", "ammo", "armory", "armoury"],
    ),
    (
        Glyph::Nature,
        &[
            "nature", "plant", "mushroom", "fruit", "apricot", "herb", "mullein", "pear", "berry",
            "flower", "tree",
        ],
    ),
    (Glyph::Quest, &["quest", "objective", "mission", "task"]),
    (
        Glyph::Event,
        &["event", "harvester", "graveyard", "snow", "assessor"],
    ),
    (
        Glyph::Container,
        &[
            "container",
            "crate",
            "box",
            "cache",
            "loot",
            "locker",
            "chest",
            "case",
            "bag",
            "safe",
            "stash",
        ],
    ),
    (
        Glyph::Arc,
        &[
            "arc",
            "tick",
            "wasp",
            "hornet",
            "leaper",
            "rocketeer",
            "sentinel",
            "turret",
            "snitch",
            "surveyor",
            "pop",
            "fireball",
            "enemy",
            "robot",
        ],
    ),
    (Glyph::Spawn, &["spawn", "player"]),
    (
        Glyph::Place,
        &[
            "label", "location", "zone", "poi", "area", "landmark", "station", "depot", "camp",
        ],
    ),
];

/// The glyph for a marker kind. The subcategory decides when it is
/// recognised (`weapon_case` under `containers`), else the category.
pub fn glyph(category: &str, subcategory: Option<&str>) -> Glyph {
    let find = |name: &str| {
        let name = name.to_lowercase();
        KEYWORDS
            .iter()
            .find(|(_, words)| words.iter().any(|w| name.contains(w)))
            .map(|(glyph, _)| *glyph)
    };
    subcategory
        .and_then(find)
        .or_else(|| find(category))
        .unwrap_or(Glyph::Other)
}

/// The glyph's SVG handle. Built once: iced caches rasterisations per
/// handle, so reusing it keeps drawing cheap.
pub fn handle(glyph: Glyph) -> svg::Handle {
    static HANDLES: OnceLock<Vec<svg::Handle>> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        ALL.iter()
            .map(|g| svg::Handle::from_memory(source(*g)))
            .collect()
    });
    handles[ALL
        .iter()
        .position(|g| *g == glyph)
        .unwrap_or(ALL.len() - 1)]
    .clone()
}

const ALL: [Glyph; 14] = [
    Glyph::Boss,
    Glyph::Arc,
    Glyph::Container,
    Glyph::Weapon,
    Glyph::Medical,
    Glyph::Nature,
    Glyph::Quest,
    Glyph::Extraction,
    Glyph::Hatch,
    Glyph::Event,
    Glyph::Place,
    Glyph::Key,
    Glyph::Spawn,
    Glyph::Other,
];

fn source(glyph: Glyph) -> &'static [u8] {
    match glyph {
        Glyph::Boss => include_bytes!("../assets/markers/boss.svg"),
        Glyph::Arc => include_bytes!("../assets/markers/arc.svg"),
        Glyph::Container => include_bytes!("../assets/markers/container.svg"),
        Glyph::Weapon => include_bytes!("../assets/markers/weapon.svg"),
        Glyph::Medical => include_bytes!("../assets/markers/medical.svg"),
        Glyph::Nature => include_bytes!("../assets/markers/nature.svg"),
        Glyph::Quest => include_bytes!("../assets/markers/quest.svg"),
        Glyph::Extraction => include_bytes!("../assets/markers/extraction.svg"),
        Glyph::Hatch => include_bytes!("../assets/markers/hatch.svg"),
        Glyph::Event => include_bytes!("../assets/markers/event.svg"),
        Glyph::Place => include_bytes!("../assets/markers/place.svg"),
        Glyph::Key => include_bytes!("../assets/markers/key.svg"),
        Glyph::Spawn => include_bytes!("../assets/markers/spawn.svg"),
        Glyph::Other => include_bytes!("../assets/markers/other.svg"),
    }
}

/// A round badge: the marker's glyph on its category colour, `size` px.
pub fn badge<'a, M: 'a>(category: &str, subcategory: Option<&str>, size: f32) -> Element<'a, M> {
    let color = palette::marker(category);
    let inner = size * 0.62;
    container(
        svg(handle(glyph(category, subcategory)))
            .width(inner)
            .height(inner),
    )
    .center_x(size)
    .center_y(size)
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(color.into()),
        border: Border {
            radius: (size / 2.0).into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// A marker kind's glyph on its category colour, `size` px across.
pub fn draw_badge(
    frame: &mut Frame,
    category: &str,
    subcategory: Option<&str>,
    at: Point,
    size: f32,
) {
    frame.fill(
        &Path::circle(at, size / 2.0),
        crate::palette::marker(category),
    );
    let inner = size * 0.62;
    frame.draw_svg(
        Rectangle::new(
            Point::new(at.x - inner / 2.0, at.y - inner / 2.0),
            Size::new(inner, inner),
        ),
        &handle(glyph(category, subcategory)),
    );
}

/// A dense group: its outline (padded so edge markers sit inside), shaded
/// in the kind's colour, with one badge and the count at the centre.
pub fn draw_area(
    frame: &mut Frame,
    area: &arclens_core::MarkerArea,
    to_screen: impl Fn(arclens_core::MapPoint) -> Point,
    icon: f32,
) {
    const PAD: f32 = 9.0;
    let color = crate::palette::marker(&area.category);
    let center = to_screen(area.center);
    let outline: Vec<Point> = area
        .hull
        .iter()
        .map(|&p| {
            let p = to_screen(p);
            let (dx, dy) = (p.x - center.x, p.y - center.y);
            let len = dx.hypot(dy).max(1.0);
            Point::new(p.x + dx / len * PAD, p.y + dy / len * PAD)
        })
        .collect();
    let shape = if outline.len() >= 3 {
        Path::new(|b| {
            b.move_to(outline[0]);
            for p in &outline[1..] {
                b.line_to(*p);
            }
            b.close();
        })
    } else {
        let radius = outline
            .iter()
            .map(|p| p.distance(center))
            .fold(PAD, f32::max);
        Path::circle(center, radius)
    };
    frame.fill(&shape, crate::palette::with_alpha(color, 0.18));
    frame.stroke(
        &shape,
        Stroke::default()
            .with_color(crate::palette::with_alpha(color, 0.7))
            .with_width(1.5),
    );
    draw_badge(
        frame,
        &area.category,
        area.subcategory.as_deref(),
        center,
        icon,
    );
    frame.fill_text(canvas::Text {
        content: format!("×{}", area.count),
        position: Point::new(center.x + icon / 2.0 + 3.0, center.y),
        color: crate::palette::TEXT,
        size: 11.0.into(),
        font: iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::DEFAULT
        },
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_glyphs_by_keyword() {
        assert_eq!(glyph("arc", Some("queen")), Glyph::Boss);
        assert_eq!(glyph("arc", Some("tick")), Glyph::Arc);
        assert_eq!(glyph("containers", Some("weapon_case")), Glyph::Weapon);
        assert_eq!(glyph("containers", Some("medical_bag")), Glyph::Medical);
        assert_eq!(glyph("containers", Some("utility_box")), Glyph::Container);
        assert_eq!(glyph("locations", Some("raider_hatch")), Glyph::Hatch);
        assert_eq!(glyph("locations", Some("extraction")), Glyph::Extraction);
        assert_eq!(glyph("nature", Some("prickly_pear")), Glyph::Nature);
        assert_eq!(glyph("labels", Some("poi")), Glyph::Place);
        // Real MetaForge subcategories (Dam, 2026-10-03).
        assert_eq!(glyph("locations", Some("supply_station")), Glyph::Place);
        assert_eq!(glyph("containers", Some("arc_probe")), Glyph::Arc);
        assert_eq!(glyph("containers", Some("raider_cache")), Glyph::Container);
        assert_eq!(glyph("containers", Some("med_crate")), Glyph::Medical);
        assert_eq!(glyph("arc", Some("rollbot")), Glyph::Arc);
        assert_eq!(glyph("events", Some("snow_pile")), Glyph::Event);
        assert_eq!(glyph("locations", Some("locked_room")), Glyph::Key);
        assert_eq!(glyph("locations", Some("player_spawn")), Glyph::Spawn);
        assert_eq!(glyph("nature", Some("candleberries")), Glyph::Nature);
        // Unknown subcategory: the category decides.
        assert_eq!(glyph("quests", Some("xyz")), Glyph::Quest);
        assert_eq!(glyph("mystery", None), Glyph::Other);
    }

    #[test]
    fn every_glyph_is_valid_svg() {
        for g in ALL {
            let text = std::str::from_utf8(source(g)).unwrap();
            assert!(text.starts_with("<svg") && text.trim_end().ends_with("</svg>"));
        }
    }
}
