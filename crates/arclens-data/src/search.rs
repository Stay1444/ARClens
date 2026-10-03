//! Fuzzy item search used by the overlay's quick-lookup box.

use arclens_core::Item;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Fuzzy index over item names. Cheap to rebuild; holds no item data itself.
pub struct ItemSearch {
    matcher: Matcher,
}

impl std::fmt::Debug for ItemSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemSearch").finish_non_exhaustive()
    }
}

impl Default for ItemSearch {
    fn default() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }
}

impl ItemSearch {
    /// Returns up to `limit` items whose name matches `query`, best first.
    /// An empty query matches nothing.
    pub fn search<'a>(&mut self, items: &'a [Item], query: &str, limit: usize) -> Vec<&'a Item> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut buf = Vec::new();
        let mut scored: Vec<(u32, &Item)> = items
            .iter()
            .filter_map(|item| {
                let haystack = Utf32Str::new(&item.name, &mut buf);
                pattern
                    .score(haystack, &mut self.matcher)
                    .map(|s| (s, item))
            })
            .collect();

        // Higher score first; ties broken by shorter (more exact) name, then name.
        scored.sort_by(|(sa, a), (sb, b)| {
            sb.cmp(sa)
                .then(a.name.len().cmp(&b.name.len()))
                .then_with(|| a.name.cmp(&b.name))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, item)| item)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::ItemId;

    fn named(name: &str) -> Item {
        Item {
            id: ItemId::new(name.to_lowercase().replace(' ', "-")),
            name: name.to_owned(),
            description: None,
            rarity: None,
            category: None,
            value: None,
            weight: None,
            stack_size: None,
            recycles_into: Vec::new(),
            salvages_into: Vec::new(),
            required_for: Vec::new(),
            image_url: None,
        }
    }

    #[test]
    fn finds_items_case_insensitively_and_ranks_exact_first() {
        let items = vec![
            named("Rusted Gear"),
            named("Gear Bench Kit"),
            named("Battery"),
        ];
        let mut search = ItemSearch::default();
        let hits: Vec<_> = search
            .search(&items, "rusted gear", 5)
            .into_iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(hits.first(), Some(&"Rusted Gear"));
        assert!(!hits.contains(&"Battery"));
    }

    #[test]
    fn empty_query_returns_nothing() {
        let items = vec![named("Battery")];
        assert!(ItemSearch::default().search(&items, "  ", 5).is_empty());
    }

    #[test]
    fn respects_limit() {
        let items = vec![named("Gear A"), named("Gear B"), named("Gear C")];
        assert_eq!(ItemSearch::default().search(&items, "gear", 2).len(), 2);
    }
}
