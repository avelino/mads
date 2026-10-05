use std::collections::BTreeSet;

use super::{Keyword, MatchType, contains_word_sequence, normalize};

/// What an agent sends: the Rust side does the combinatorics.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeywordSpec {
    pub variants: Vec<String>,
    pub modifiers: Vec<String>,
    pub exact_heads: bool,
    /// Keep a one-word variant as a phrase keyword. Alone it matches any search with that word,
    /// so only brand campaigns want it.
    pub one_word_phrase: bool,
    pub extra: Vec<Keyword>,
}

/// Phrase keywords (variants and variants x modifiers) in byte order, then the variants
/// as exact match in input order, then explicit extras that are not already present.
/// Without `one_word_phrase`, a one-word variant that is also exact has no bare phrase keyword.
pub fn expand_keywords(spec: &KeywordSpec) -> Vec<Keyword> {
    let variants = unique_keep_first(spec.variants.iter().map(|v| normalize(v)));
    let modifiers = unique_keep_first(spec.modifiers.iter().map(|m| normalize(m)));

    let mut phrase = BTreeSet::new();
    for v in &variants {
        let one_word = !v.contains(' ');
        if spec.one_word_phrase || !one_word || !spec.exact_heads {
            phrase.insert(v.clone());
        }
        // "vinellu app" x "app" would give "vinellu app app".
        for m in modifiers.iter().filter(|m| !contains_word_sequence(v, m)) {
            phrase.insert(format!("{v} {m}"));
        }
    }

    let mut out: Vec<Keyword> = phrase
        .into_iter()
        .map(|text| Keyword {
            text,
            match_type: MatchType::Phrase,
        })
        .collect();
    if spec.exact_heads {
        out.extend(variants.into_iter().map(|text| Keyword {
            text,
            match_type: MatchType::Exact,
        }));
    }
    for extra in &spec.extra {
        let k = Keyword {
            text: normalize(&extra.text),
            match_type: extra.match_type,
        };
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

fn unique_keep_first(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    items
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::MatchType::{Exact, Phrase};

    fn kw(rows: &[(&str, MatchType)]) -> Vec<Keyword> {
        rows.iter()
            .map(|(t, m)| Keyword {
                text: (*t).into(),
                match_type: *m,
            })
            .collect()
    }

    fn spec(variants: &[&str], modifiers: &[&str], exact_heads: bool) -> KeywordSpec {
        KeywordSpec {
            variants: variants.iter().map(|s| s.to_string()).collect(),
            modifiers: modifiers.iter().map(|s| s.to_string()).collect(),
            exact_heads,
            one_word_phrase: true,
            extra: vec![],
        }
    }

    #[test]
    fn one_word_head_without_phrase_keeps_exact_and_modifier_phrases() {
        let mut s = spec(&["malbec", "vinho malbec"], &["review"], true);
        s.one_word_phrase = false;
        assert_eq!(
            expand_keywords(&s),
            kw(&[
                ("malbec review", Phrase),
                ("vinho malbec", Phrase),
                ("vinho malbec review", Phrase),
                ("malbec", Exact),
                ("vinho malbec", Exact),
            ])
        );
    }

    #[test]
    fn small_example_phrase_sorted_then_exact_in_variant_order() {
        let got = expand_keywords(&spec(&["b x", "a"], &["m"], true));
        assert_eq!(
            got,
            kw(&[
                ("a", Phrase),
                ("a m", Phrase),
                ("b x", Phrase),
                ("b x m", Phrase),
                ("b x", Exact),
                ("a", Exact)
            ])
        );
    }

    #[test]
    fn no_exact_when_disabled_and_dedupes_variants() {
        let got = expand_keywords(&spec(&["A", "a "], &[], false));
        assert_eq!(got, kw(&[("a", Phrase)]));
    }

    #[test]
    fn extra_keywords_are_appended_and_skip_duplicates() {
        let mut s = spec(&["a"], &[], true);
        s.extra = kw(&[("a", Phrase), ("zzz", Exact)]);
        let got = expand_keywords(&s);
        assert_eq!(got, kw(&[("a", Phrase), ("a", Exact), ("zzz", Exact)]));
    }

    #[test]
    fn reproduces_every_ad_group_of_the_reference_csv() {
        let csv_text = include_str!("../../../../data/3-keywords.csv");
        let mut rdr = csv::Reader::from_reader(csv_text.as_bytes());
        let mut groups: Vec<(String, Vec<Keyword>)> = Vec::new();
        for rec in rdr.records() {
            let rec = rec.unwrap();
            let (group, text, ty) = (rec[4].to_string(), rec[5].to_string(), rec[6].to_string());
            let match_type = if ty == "Exact match" { Exact } else { Phrase };
            match groups.last_mut() {
                Some((g, v)) if *g == group => v.push(Keyword { text, match_type }),
                _ => groups.push((group, vec![Keyword { text, match_type }])),
            }
        }
        assert_eq!(groups.len(), 12);
        let modifiers = ["harmonização", "review", "safra", "vale a pena", "é bom"];
        for (group, expected) in groups {
            let variants: Vec<&str> = expected
                .iter()
                .filter(|k| k.match_type == Exact)
                .map(|k| k.text.as_str())
                .collect();
            let got = expand_keywords(&spec(&variants, &modifiers, true));
            assert_eq!(got, expected, "ad group {group}");
        }
    }

    #[test]
    fn a_modifier_the_variant_already_contains_is_not_repeated() {
        let got = expand_keywords(&spec(
            &["vinellu app", "x vale a pena"],
            &["app", "vale a pena", "review"],
            false,
        ));
        let texts: Vec<&str> = got.iter().map(|k| k.text.as_str()).collect();
        assert!(!texts.contains(&"vinellu app app"), "{texts:?}");
        assert!(!texts.contains(&"x vale a pena vale a pena"), "{texts:?}");
        assert!(
            texts.contains(&"vinellu app review") && texts.contains(&"vinellu app vale a pena"),
            "{texts:?}"
        );
    }
}
