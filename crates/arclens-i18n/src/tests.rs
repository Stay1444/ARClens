use super::*;
use fluent_syntax::ast;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Message id → the variables it uses, per language.
fn messages(lang: Lang) -> BTreeMap<String, BTreeSet<String>> {
    let (_, files) = RESOURCES.iter().find(|(l, _)| *l == lang).unwrap();
    let mut out = BTreeMap::new();
    for source in *files {
        let resource = fluent_syntax::parser::parse(*source)
            .unwrap_or_else(|(_, errors)| panic!("{}: {errors:?}", lang.code()));
        for entry in resource.body {
            if let ast::Entry::Message(message) = entry {
                let mut vars = BTreeSet::new();
                if let Some(value) = &message.value {
                    pattern_vars(value, &mut vars);
                }
                let id = message.id.name.to_owned();
                assert!(
                    out.insert(id.clone(), vars).is_none(),
                    "{}: {id} defined twice",
                    lang.code()
                );
            }
        }
    }
    out
}

fn pattern_vars(pattern: &ast::Pattern<&str>, vars: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_vars(expression, vars);
        }
    }
}

fn expression_vars(expression: &ast::Expression<&str>, vars: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, vars);
            for variant in variants {
                pattern_vars(&variant.value, vars);
            }
        }
        ast::Expression::Inline(inline) => inline_vars(inline, vars),
    }
}

fn inline_vars(inline: &ast::InlineExpression<&str>, vars: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            vars.insert(id.name.to_owned());
        }
        ast::InlineExpression::Placeable { expression } => expression_vars(expression, vars),
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for arg in &arguments.positional {
                inline_vars(arg, vars);
            }
        }
        _ => {}
    }
}

#[test]
fn every_language_has_the_same_messages_and_variables() {
    let en = messages(Lang::En);
    assert!(en.len() > 100, "only {} messages", en.len());
    for lang in Lang::ALL {
        let other = messages(lang);
        let missing: Vec<_> = en.keys().filter(|id| !other.contains_key(*id)).collect();
        let extra: Vec<_> = other.keys().filter(|id| !en.contains_key(*id)).collect();
        assert!(missing.is_empty(), "{} lacks {missing:?}", lang.code());
        assert!(extra.is_empty(), "{} has extra {extra:?}", lang.code());
        for (id, vars) in &en {
            assert_eq!(&other[id], vars, "{}: variables of {id}", lang.code());
        }
    }
}

/// Every `t!("…")` / `tr("…")` in the sources names a message.
#[test]
fn every_message_used_in_the_code_exists() {
    let en = messages(Lang::En);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut missing = Vec::new();
    let mut used = 0;
    for dir in ["apps", "crates"] {
        for file in rust_files(&root.join(dir)) {
            let source = std::fs::read_to_string(&file).unwrap();
            for id in literal_ids(&source) {
                used += 1;
                if !en.contains_key(&id) {
                    missing.push(format!("{}: {id}", file.display()));
                }
            }
        }
    }
    assert!(used > 100, "only {used} uses found");
    assert!(missing.is_empty(), "unknown messages: {missing:#?}");
}

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // This crate's docs and tests use placeholder ids.
            let skip = ["target", "fixtures", "arclens-i18n"];
            if !skip.iter().any(|s| path.ends_with(s)) {
                out.extend(rust_files(&path));
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// The ids in `t!("id"`, `tr("id"` and `tr_with("id"` calls.
fn literal_ids(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for pattern in ["t!(", "tr(", "tr_with("] {
        let mut rest = source;
        while let Some(at) = rest.find(pattern) {
            rest = rest[at + pattern.len()..].trim_start();
            let Some(after) = rest.strip_prefix('"') else {
                continue;
            };
            let id: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            if after[id.len()..].starts_with('"') && !id.is_empty() {
                out.push(id);
            }
        }
    }
    out
}

#[test]
fn formats_with_arguments_and_plurals() {
    let args = |n: u32| {
        let mut a = FluentArgs::new();
        a.set("count", n);
        a
    };
    assert_eq!(
        format_in(Lang::En, "item-count", Some(&args(1))).unwrap(),
        "1 item"
    );
    assert_eq!(
        format_in(Lang::En, "item-count", Some(&args(3))).unwrap(),
        "3 items"
    );
    assert_eq!(
        format_in(Lang::EsEs, "item-count", Some(&args(1))).unwrap(),
        "1 objeto"
    );
    assert_eq!(
        format_in(Lang::EsEs, "item-count", Some(&args(3))).unwrap(),
        "3 objetos"
    );
}

#[test]
fn reads_locale_tags() {
    assert_eq!(Lang::from_tag("es-ES"), Some(Lang::EsEs));
    assert_eq!(Lang::from_tag("es_MX.UTF-8"), Some(Lang::EsEs));
    assert_eq!(Lang::from_tag("en-GB"), Some(Lang::En));
    assert_eq!(Lang::from_tag("de-DE"), None);
    assert_eq!(Lang::from_tag("C"), None);
}

#[test]
fn slugs_data_ids() {
    assert_eq!(slug("weapon_case"), "weapon-case");
    assert_eq!(slug("Night Raid"), "night-raid");
    assert_eq!(slug("hornet "), "hornet");
}

/// The only test that sets the process-wide language.
#[test]
fn numbers_group_per_language() {
    set(Lang::En);
    assert_eq!(number(18_431), "18,431");
    assert_eq!(number(1_234_567), "1,234,567");
    assert_eq!(number(999), "999");
    set(Lang::EsEs);
    assert_eq!(number(18_431), "18.431");
    assert_eq!(number(1500), "1500");
    assert_eq!(current(), Lang::EsEs);
    set(Lang::En);
}
