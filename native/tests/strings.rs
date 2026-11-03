//! The words, `test/manifest.test.js`'s "English and French hold the same keys": every string in
//! both languages, with the same placeholders, accents and glossary, and none the code asks for
//! that is not there.

use std::collections::{BTreeMap, BTreeSet};

use kit_db_tab::strings::{EN, FR, strings};

/// A key without its plural form.
fn stem(key: &str) -> &str {
    key.strip_suffix("_one")
        .or_else(|| key.strip_suffix("_other"))
        .unwrap_or(key)
}

fn dictionary(
    entries: &'static [(&'static str, &'static str)],
) -> BTreeMap<&'static str, &'static str> {
    let map: BTreeMap<_, _> = entries.iter().copied().collect();
    assert_eq!(map.len(), entries.len(), "a key is written twice");
    map
}

fn placeholders(template: &str) -> Vec<&str> {
    let mut found: Vec<&str> = template
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
        .collect();
    found.sort_unstable();
    found
}

#[test]
fn english_and_french_hold_the_same_keys() {
    let (en, fr) = (dictionary(EN), dictionary(FR));
    let keys = |dictionary: &BTreeMap<&str, &str>| -> BTreeSet<String> {
        dictionary.keys().map(|key| stem(key).to_owned()).collect()
    };
    assert_eq!(keys(&fr), keys(&en));
    for key in en.keys() {
        assert!(fr.contains_key(key), "{key} has no French");
        assert_eq!(placeholders(en[key]), placeholders(fr[key]), "{key}");
    }
    for key in fr.keys() {
        assert!(en.contains_key(key), "{key} has no English");
    }
}

#[test]
fn a_count_has_both_its_forms() {
    for entries in [EN, FR] {
        let dictionary = dictionary(entries);
        for key in dictionary.keys() {
            if let Some(stem) = key.strip_suffix("_one") {
                assert!(
                    dictionary.contains_key(format!("{stem}_other").as_str()),
                    "{stem}"
                );
            }
            if let Some(stem) = key.strip_suffix("_other") {
                assert!(
                    dictionary.contains_key(format!("{stem}_one").as_str()),
                    "{stem}"
                );
            }
        }
    }
}

#[test]
fn french_has_its_accents_and_its_glossary() {
    let fr = dictionary(FR);
    let en = dictionary(EN);
    assert_eq!(fr["tab"], "Base de données");
    assert!(fr["deniedText"].contains("Réglages › Kits"));
    assert!(en["deniedText"].contains("Settings › Kits"));
    assert_eq!(fr["handOver"], "Confier à un agent");
    assert!(fr["secretNote"].contains("« Confier à un agent »"));
    assert!(fr["noDbText"].contains("espace de travail"));
    assert!(fr["panesDenied"].contains("Réglages › Kits"));
    let all = FR
        .iter()
        .map(|(_, text)| *text)
        .collect::<Vec<_>>()
        .join(" ");
    for unaccented in [
        "Reglages",
        "echec",
        "donnees",
        "verrouille",
        "Base de donnees",
        "a un agent",
    ] {
        let word = |c: char| !c.is_alphanumeric();
        let found = all.match_indices(unaccented).any(|(at, hit)| {
            all[..at].chars().next_back().is_none_or(word)
                && all[at + hit.len()..].chars().next().is_none_or(word)
        });
        assert!(!found, "{unaccented} is missing its accents");
    }
}

#[test]
fn every_string_the_code_asks_for_is_there_and_none_is_left_unused() {
    let source = [
        include_str!("../src/lib.rs"),
        include_str!("../src/view.rs"),
    ]
    .join("\n");
    let mut asked = BTreeSet::new();
    for call in [".t(", ".tv(", ".tn("] {
        for (at, _) in source.match_indices(call) {
            // The key is the first argument, on the same line or the next.
            let rest = source[at + call.len()..].trim_start();
            if let Some(rest) = rest.strip_prefix('"') {
                asked.insert(rest[..rest.find('"').unwrap()].to_owned());
            }
        }
    }
    assert!(
        asked.len() > 40,
        "the words are found in the code: {asked:?}"
    );
    let en: BTreeSet<String> = EN.iter().map(|(key, _)| stem(key).to_owned()).collect();
    for key in &asked {
        assert!(en.contains(key), "{key} is asked for and not written");
    }
    // Those the code builds a key for: a command's failure, a send's.
    let dynamic = |key: &str| key.starts_with("error.") || key.starts_with("problem.");
    for key in &en {
        assert!(
            asked.contains(key) || dynamic(key),
            "{key} is written and never used"
        );
    }
    for code in [
        "denied",
        "not_found",
        "rate_limited",
        "unavailable",
        "failed",
    ] {
        assert!(en.contains(&format!("problem.{code}")), "{code}");
    }
}

#[test]
fn the_language_is_the_one_asked_for_and_english_fills_the_gaps() {
    let fr = strings("fr-FR");
    assert_eq!(fr.t("readOnly"), "Lecture seule");
    assert_eq!(fr.tn("rowsCount", 1, &[]), "1 ligne");
    assert_eq!(
        fr.tn("rowsCount", 0, &[]),
        "0 ligne",
        "French counts none as one"
    );
    assert_eq!(fr.tn("rowsCount", 120, &[]), "120 lignes");
    let en = strings("en");
    assert_eq!(en.tn("rowsCount", 1, &[]), "1 row");
    assert_eq!(en.tn("rowsCount", 0, &[]), "0 rows");
    assert_eq!(en.tn("blob", 8, &[]), "8 bytes (binary)");
    assert_eq!(strings("de").t("readOnly"), "Read-only");
    assert_eq!(en.t("no such key"), "no such key");
}
