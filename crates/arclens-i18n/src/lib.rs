//! Translations of the app and overlay: [Project Fluent] messages per
//! language, compiled in, and one process-wide current language.
//!
//! ```
//! arclens_i18n::set(arclens_i18n::Lang::EsEs);
//! assert_eq!(arclens_i18n::t!("tab-map"), "Mapa");
//! assert_eq!(arclens_i18n::t!("map-shown", shown = 3, total = 40), "3 / 40 visibles");
//! ```
//!
//! Messages live in `locales/<lang>/*.ftl`; every language has the same
//! ids (checked by the tests). A missing message falls back to English,
//! then to its id.
//!
//! Game data (item, quest and station names) is translated by the data
//! source, not here: [`Lang::data_code`] picks it.
//!
//! [Project Fluent]: https://projectfluent.org

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

pub use fluent_bundle::FluentValue;

/// A language the interface is translated into.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    #[default]
    En,
    /// Spanish as written in Spain.
    EsEs,
}

impl Lang {
    pub const ALL: [Self; 2] = [Self::En, Self::EsEs];

    /// BCP 47 tag, as saved in the settings: `en`, `es-ES`.
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::EsEs => "es-ES",
        }
    }

    /// The language's name in itself, for the language picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::EsEs => "Español (España)",
        }
    }

    /// Key of the language in RaidTheory's localised strings.
    pub fn data_code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::EsEs => "es",
        }
    }

    /// The language a locale tag (`es-ES`, `es_MX.UTF-8`, `en-GB`, `C`)
    /// is best shown in, if we have it. Any Spanish gets Spain's.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Self::En),
            "es" => Some(Self::EsEs),
            _ => None,
        }
    }

    /// The system's language if translated, else English.
    pub fn system() -> Self {
        sys_locale::get_locales()
            .find_map(|tag| Self::from_tag(&tag))
            .unwrap_or_default()
    }

    fn index(self) -> u8 {
        match self {
            Self::En => 0,
            Self::EsEs => 1,
        }
    }
}

/// `(language, its .ftl files)`.
const RESOURCES: [(Lang, &[&str]); 2] = [
    (
        Lang::En,
        &[
            include_str!("../locales/en/common.ftl"),
            include_str!("../locales/en/app.ftl"),
            include_str!("../locales/en/overlay.ftl"),
            include_str!("../locales/en/game.ftl"),
        ],
    ),
    (
        Lang::EsEs,
        &[
            include_str!("../locales/es-ES/common.ftl"),
            include_str!("../locales/es-ES/app.ftl"),
            include_str!("../locales/es-ES/overlay.ftl"),
            include_str!("../locales/es-ES/game.ftl"),
        ],
    ),
];

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Sets the language every later lookup uses.
pub fn set(lang: Lang) {
    CURRENT.store(lang.index(), Ordering::Relaxed);
}

/// The language lookups use now.
pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::EsEs,
        _ => Lang::En,
    }
}

fn bundles() -> &'static [FluentBundle<FluentResource>] {
    static BUNDLES: OnceLock<Vec<FluentBundle<FluentResource>>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        RESOURCES
            .iter()
            .map(|(lang, files)| {
                let id = lang.code().parse().unwrap_or_default();
                let mut bundle = FluentBundle::new_concurrent(vec![id]);
                // No Unicode isolation marks around arguments: the UI font
                // would draw them as boxes.
                bundle.set_use_isolating(false);
                for source in *files {
                    // Syntax errors are caught by the tests; a broken entry
                    // is skipped, the rest of the file still loads.
                    let resource = FluentResource::try_new((*source).to_owned())
                        .unwrap_or_else(|(partial, _)| partial);
                    let _ = bundle.add_resource(resource);
                }
                bundle
            })
            .collect()
    })
}

fn format_in(lang: Lang, id: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let bundle = bundles().get(usize::from(lang.index()))?;
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned(),
    )
}

/// The message `id` in the current language, if any language has it.
pub fn try_tr_with(id: &str, args: &[(&str, FluentValue<'_>)]) -> Option<String> {
    let args = (!args.is_empty()).then(|| {
        args.iter()
            .map(|(name, value)| (*name, value.clone()))
            .collect::<FluentArgs<'_>>()
    });
    let lang = current();
    format_in(lang, id, args.as_ref()).or_else(|| {
        (lang != Lang::En)
            .then(|| format_in(Lang::En, id, args.as_ref()))
            .flatten()
    })
}

/// The message `id` in the current language, or `None` if no language
/// has it (for keys built from data, e.g. a marker kind).
pub fn try_tr(id: &str) -> Option<String> {
    try_tr_with(id, &[])
}

/// The message `id` with `args`; its id if no language has it.
pub fn tr_with(id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
    try_tr_with(id, args).unwrap_or_else(|| id.to_owned())
}

/// The message `id`; its id if no language has it.
pub fn tr(id: &str) -> String {
    tr_with(id, &[])
}

/// `t!("id")` or `t!("id", name = value, ...)`: the message in the
/// current language. Values are anything [`FluentValue`] converts from
/// (strings, numbers).
#[macro_export]
macro_rules! t {
    ($id:literal) => {
        $crate::tr($id)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::tr_with(
            $id,
            &[$((stringify!($name), $crate::FluentValue::from($value))),+],
        )
    };
}

/// `n` with the current language's thousands separator: `18,431` in
/// English, `18.431` in Spanish (which leaves 4-digit numbers alone).
pub fn number(n: u32) -> String {
    let digits = n.to_string();
    let sep = match current() {
        Lang::En => ',',
        Lang::EsEs if digits.len() <= 4 => return digits,
        Lang::EsEs => '.',
    };
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// A data id (`weapon_case`, `night-raid`) as a message-id suffix:
/// `weapon-case`, `night-raid`.
pub fn slug(id: &str) -> String {
    id.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests;
