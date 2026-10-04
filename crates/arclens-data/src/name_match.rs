//! Mapping recognised tooltip text to a catalogue item.

use arclens_core::Item;

/// Minimum similarity (normalised Levenshtein, 0–1) for a non-exact match.
/// High on purpose: names like "Osprey I" / "Osprey II" differ by one
/// character, so a loose threshold would confidently pick the wrong tier.
const MIN_SIMILARITY: f64 = 0.9;

/// Uppercases, drops accents ("ESCÁNER" → "ESCANER": OCR may or may not
/// keep them), drops a leading trader quantity ("x25", "×25"), and
/// collapses whitespace.
pub fn normalize_name(text: &str) -> String {
    let upper: String = text
        .trim()
        .to_uppercase()
        .chars()
        .map(fold_accent)
        .collect();
    let mut words: Vec<&str> = upper.split_whitespace().collect();
    if let Some(first) = words.first()
        && let Some(rest) = first.strip_prefix('X').or_else(|| first.strip_prefix('×'))
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        words.remove(0);
    }
    words.join(" ")
}

/// Spanish (and other Latin) accented capitals to their plain letter.
fn fold_accent(c: char) -> char {
    match c {
        'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ñ' => 'N',
        'Ç' => 'C',
        c => c,
    }
}

/// The catalogue item whose name (in any language it has, see
/// [`Item::aliases`]) best matches recognised `text`, with a confidence in
/// `0.0..=1.0`. `None` when nothing is close enough.
pub fn match_name<'a>(text: &str, items: &'a [Item]) -> Option<(&'a Item, f64)> {
    let wanted = normalize_name(text);
    if wanted.is_empty() {
        return None;
    }
    let mut best: Option<(&Item, f64)> = None;
    for item in items {
        for name in item.names() {
            let name = normalize_name(name);
            if name == wanted {
                return Some((item, 1.0));
            }
            let score = strsim::normalized_levenshtein(&name, &wanted);
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((item, score));
            }
        }
    }
    best.filter(|&(_, score)| score >= MIN_SIMILARITY)
}

/// The best of `candidates` for recognised `text` by any of its names
/// (normalised like item names), if at least `min` similar.
pub fn best_named<'a, T, N>(text: &str, candidates: &'a [T], names: N, min: f64) -> Option<&'a T>
where
    N: Fn(&'a T) -> Vec<&'a str>,
{
    let wanted = normalize_name(text);
    candidates
        .iter()
        .filter_map(|c| {
            names(c)
                .into_iter()
                .map(|n| strsim::normalized_levenshtein(&wanted, &normalize_name(n)))
                .max_by(f64::total_cmp)
                .map(|score| (c, score))
        })
        .filter(|&(_, score)| score >= min)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::ItemId;

    fn named(name: &str) -> Item {
        Item {
            id: ItemId::new(name.to_lowercase().replace(' ', "_")),
            name: name.to_owned(),
            aliases: Vec::new(),
            description: None,
            rarity: None,
            category: None,
            value: None,
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            ingredient_of: Vec::new(),
            image_url: None,
            details: arclens_core::ItemDetails::default(),
        }
    }

    fn catalogue() -> Vec<Item> {
        [
            "Osprey I",
            "Osprey II",
            "Light Ammo",
            "Combat Mk. 3 (Aggressive)",
            "Shield Recharger",
        ]
        .into_iter()
        .map(named)
        .collect()
    }

    #[test]
    fn exact_matches_ignore_case_and_trader_quantity() {
        let items = catalogue();
        assert_eq!(
            match_name("OSPREY II", &items).map(|(i, _)| i.name.as_str()),
            Some("Osprey II")
        );
        assert_eq!(
            match_name("x25 LIGHT AMMO", &items).map(|(i, _)| i.name.as_str()),
            Some("Light Ammo")
        );
        assert_eq!(
            match_name("×25 LIGHT AMMO", &items).map(|(i, s)| (i.name.as_str(), s)),
            Some(("Light Ammo", 1.0))
        );
        assert_eq!(
            match_name("COMBAT MK. 3 (AGGRESSIVE)", &items).map(|(i, _)| i.name.as_str()),
            Some("Combat Mk. 3 (Aggressive)")
        );
    }

    #[test]
    fn tolerates_a_small_misread() {
        let items = catalogue();
        assert_eq!(
            match_name("SHIELD RECHARGFR", &items).map(|(i, _)| i.name.as_str()),
            Some("Shield Recharger")
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(match_name("REQUEST AMMO", &catalogue()).is_none());
        assert!(match_name("   ", &catalogue()).is_none());
    }

    #[test]
    fn does_not_strip_real_words_starting_with_x() {
        assert_eq!(normalize_name("XENON LAMP"), "XENON LAMP");
        assert_eq!(normalize_name("x25 Light   Ammo"), "LIGHT AMMO");
    }
}
