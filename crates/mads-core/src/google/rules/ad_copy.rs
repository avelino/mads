//! Rules for the whole responsive search ad of an ad group: its specific texts merged with the
//! brand kit, as Google receives them.

use std::collections::BTreeSet;

use super::HEADLINE;
use crate::{
    google::{
        AdGroup, BrandKit, Issue, MAX_DESCRIPTIONS, MAX_HEADLINES, MergedRsa, char_len, merge_rsa,
    },
    input::fold,
};

/// A headline shorter than this counts as short. Short headlines leave room for a second and a
/// third headline on small screens.
pub(super) const SHORT_HEADLINE: usize = 20;
/// Short headlines every ad needs.
const MIN_SHORT: usize = 3;
/// Headlines that are keywords of the group, when the group has that many keywords that fit.
const MIN_KEYWORD_HEADLINES: usize = 3;
/// Two texts that share this share of their words say the same thing.
const SAME_IDEA: f64 = 0.6;

/// Issues of the merged ad of `ag`. `path` is the ad group path.
///
/// Groups without `main_keyword` were built before these rules and only get `W04`, so the
/// export of an older run is not blocked.
pub(super) fn merged_ad(ag: &AdGroup, kit: &BrandKit, path: &str) -> Vec<Issue> {
    let mut out = Vec::new();
    let ad = merge_rsa(&ag.rsa, kit);
    let at = format!("{path}.rsa");
    if ag.main_keyword.is_empty() {
        if ad.headlines.len() < MAX_HEADLINES || ad.descriptions.len() < MAX_DESCRIPTIONS {
            let msg = format!(
                "merged ad has {} headlines and {} descriptions (15 and 4 recommended)",
                ad.headlines.len(),
                ad.descriptions.len()
            );
            out.push(Issue::warning("W04", at, msg));
        }
        return out;
    }
    complete(&mut out, &ad, &at);
    short_headlines(&mut out, &ad, &at);
    first_is_main_keyword(&mut out, ag, &at);
    keyword_headlines(&mut out, ag, &ad, &at);
    let keys = keyword_keys(ag);
    same_ideas(
        &mut out,
        &ag.rsa.headlines,
        &kit.headlines,
        &keys,
        &format!("{at}.headlines"),
    );
    let none = BTreeSet::new();
    let descriptions = &ag.rsa.descriptions;
    same_ideas(
        &mut out,
        descriptions,
        &kit.descriptions,
        &none,
        &format!("{at}.descriptions"),
    );
    out
}

/// `W15` for every pair of texts of `items` that say the same thing. `path` is the list path.
pub(super) fn same_ideas_within(out: &mut Vec<Issue>, items: &[String], path: &str) {
    same_ideas(out, items, &[], &BTreeSet::new(), path);
}

fn complete(out: &mut Vec<Issue>, ad: &MergedRsa, at: &str) {
    let (h, d) = (ad.headlines.len(), ad.descriptions.len());
    if h < MAX_HEADLINES || d < MAX_DESCRIPTIONS {
        let msg = format!(
            "merged ad has {h} headlines and {d} descriptions, it needs {MAX_HEADLINES} and \
             {MAX_DESCRIPTIONS}: add specific texts that differ from the brand kit"
        );
        out.push(Issue::error("E34", at, msg));
    }
}

fn short_headlines(out: &mut Vec<Issue>, ad: &MergedRsa, at: &str) {
    let short = ad
        .headlines
        .iter()
        .filter(|h| char_len(h.trim()) < SHORT_HEADLINE)
        .count();
    if short < MIN_SHORT {
        let msg = format!(
            "merged ad has {short} headlines under {SHORT_HEADLINE} characters, it needs \
             {MIN_SHORT}: write short specific headlines"
        );
        out.push(Issue::error("E35", format!("{at}.headlines"), msg));
    }
}

fn first_is_main_keyword(out: &mut Vec<Issue>, ag: &AdGroup, at: &str) {
    if char_len(ag.main_keyword.trim()) > HEADLINE {
        return;
    }
    let first = ag
        .rsa
        .headlines
        .first()
        .map(|h| fold(h))
        .unwrap_or_default();
    if first != fold(&ag.main_keyword) {
        let msg = format!(
            "headlines[0] must be the main keyword '{}' (the first variant), only accents and \
             capitals may differ",
            ag.main_keyword
        );
        out.push(Issue::error("E36", format!("{at}.headlines[0]"), msg));
    }
}

fn keyword_headlines(out: &mut Vec<Issue>, ag: &AdGroup, ad: &MergedRsa, at: &str) {
    let keys = keyword_keys(ag);
    let need = keys.len().min(MIN_KEYWORD_HEADLINES);
    let have: BTreeSet<String> = ad
        .headlines
        .iter()
        .map(|h| fold(h))
        .filter(|k| keys.contains(k))
        .collect();
    if have.len() < need {
        let sample: Vec<&str> = keys.iter().map(String::as_str).take(5).collect();
        let msg = format!(
            "{} headlines are keywords of the group, it needs {need}: write keywords as \
             headlines, such as {}",
            have.len(),
            sample.join(", ")
        );
        out.push(Issue::error("E37", format!("{at}.headlines"), msg));
    }
}

/// Keywords of the group that fit in a headline, folded.
fn keyword_keys(ag: &AdGroup) -> BTreeSet<String> {
    ag.keywords
        .iter()
        .filter(|k| char_len(k.text.trim()) <= HEADLINE)
        .map(|k| fold(&k.text))
        .filter(|k| !k.is_empty())
        .collect()
}

/// `W15` for each text of `own` that says the same as a later text of `own` or a text of `other`.
/// Texts that are keywords (`exempt`) are variations by design and are skipped.
fn same_ideas(
    out: &mut Vec<Issue>,
    own: &[String],
    other: &[String],
    exempt: &BTreeSet<String>,
    path: &str,
) {
    let skip = |t: &String| exempt.contains(&fold(t));
    for (i, a) in own.iter().enumerate().filter(|(_, t)| !skip(t)) {
        let later = own.iter().skip(i + 1).chain(other).filter(|t| !skip(t));
        if let Some(b) = later.into_iter().find(|b| same_idea(a, b)) {
            let msg = format!("'{a}' repeats the words of '{b}': give it another idea");
            out.push(Issue::warning("W15", format!("{path}[{i}]"), msg));
        }
    }
}

/// Words of three letters or more, without accents. Short words carry no idea.
fn idea_words(text: &str) -> BTreeSet<String> {
    fold(text)
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_string)
        .collect()
}

fn same_idea(a: &str, b: &str) -> bool {
    let (wa, wb) = (idea_words(a), idea_words(b));
    if wa.len() < 2 || wb.len() < 2 {
        return false;
    }
    let shared = wa.intersection(&wb).count() as f64;
    let all = wa.union(&wb).count() as f64;
    shared / all >= SAME_IDEA
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::{Cents, Keyword, MatchType, Rsa};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|t| t.to_string()).collect()
    }

    fn kw(t: &str, m: MatchType) -> Keyword {
        Keyword {
            text: t.into(),
            match_type: m,
        }
    }

    fn kit() -> BrandKit {
        BrandKit {
            headlines: s(&[
                "Cancelamento grátis",
                "Viaje com segurança",
                "Reserve online",
                "Escolha onde embarcar",
                "Mais de 800 cidades",
                "Compre no site ou no app",
                "Motoristas monitorados",
                "Ônibus com câmera anti-fadiga",
                "Suporte em toda a viagem",
                "Passagens de ônibus na Buser",
            ]),
            descriptions: s(&[
                "Ônibus monitorados por câmera e telemetria. Reserve já.",
                "Cancele grátis e compre pelo site ou app. Busque seu trecho.",
                "Escolha onde embarcar e desembarcar. Compre sua passagem online.",
            ]),
            differentiators: s(&["Cancelamento grátis", "Escolha onde embarcar"]),
        }
    }

    fn group() -> AdGroup {
        AdGroup {
            name: "Ônibus SP RJ".into(),
            main_keyword: "onibus sp rj".into(),
            default_cpc: Cents(150),
            cpc_rationale: String::new(),
            final_url: "https://www.buser.com.br/onibus/sao-paulo-sp/rio-de-janeiro-rj".into(),
            keywords: vec![
                kw("passagem onibus sp rj", MatchType::Phrase),
                kw("onibus sp rj barato", MatchType::Phrase),
                kw("onibus sp rj", MatchType::Exact),
                kw("onibus sao paulo rio de janeiro", MatchType::Exact),
            ],
            negatives: vec![],
            rsa: Rsa {
                headlines: s(&[
                    "Ônibus SP RJ",
                    "Passagem ônibus SP RJ",
                    "Ônibus SP RJ barato",
                    "Viaje de SP ao Rio",
                    "Embarque perto de casa",
                ]),
                descriptions: s(&["Viaje de São Paulo ao Rio com cancelamento grátis. Reserve."]),
                path1: None,
                path2: None,
            },
        }
    }

    fn codes(ag: &AdGroup, kit: &BrandKit) -> Vec<String> {
        merged_ad(ag, kit, "ad_groups[0]")
            .into_iter()
            .map(|i| i.code)
            .collect()
    }

    type Case = (
        &'static str,
        fn(&mut AdGroup, &mut BrandKit),
        &'static [&'static str],
    );

    #[test]
    fn merged_ad_rules() {
        let cases: &[Case] = &[
            ("complete and varied", |_, _| {}, &[]),
            (
                "E34 fewer than 15 headlines",
                |_, k| k.headlines.truncate(8),
                &["E34"],
            ),
            (
                "E34 fewer than 4 descriptions",
                |_, k| k.descriptions.truncate(2),
                &["E34"],
            ),
            (
                "E35 fewer than 3 short headlines",
                |g, k| {
                    g.rsa.headlines[3] = "Viaje de São Paulo ao Rio hoje".into();
                    k.headlines = (1..=10)
                        .map(|i| format!("Titulo longo da marca numero {i}"))
                        .collect()
                },
                &["E35"],
            ),
            (
                "E36 first headline is not the main keyword",
                |g, _| g.rsa.headlines.swap(0, 3),
                &["E36"],
            ),
            (
                "E36 skipped when the main keyword does not fit a headline",
                |g, _| {
                    g.main_keyword = "passagem de onibus sao paulo rio de janeiro".into();
                    g.rsa.headlines[0] = "Ônibus SP RJ".into();
                },
                &[],
            ),
            (
                "E37 fewer than 3 headlines are keywords",
                |g, _| {
                    g.rsa.headlines[1] = "Saída da Barra Funda".into();
                    g.rsa.headlines[2] = "Chegada na Rodoviária Novo Rio".into();
                },
                &["E37"],
            ),
            (
                "E37 asks only for the keywords that exist",
                |g, _| {
                    g.keywords.truncate(1);
                    g.keywords[0].text = "onibus sp rj".into();
                    g.rsa.headlines[1] = "Saída da Barra Funda".into();
                    g.rsa.headlines[2] = "Chegada no Novo Rio".into();
                },
                &[],
            ),
            (
                "W15 a specific headline repeats a kit headline",
                |g, _| g.rsa.headlines[3] = "Cancelamento grátis hoje".into(),
                &["W15"],
            ),
            (
                "W15 ignores keyword headlines",
                |g, _| g.keywords.push(kw("onibus sp rj hoje", MatchType::Phrase)),
                &[],
            ),
            (
                "legacy group without main keyword keeps W04",
                |g, k| {
                    g.main_keyword.clear();
                    k.headlines.truncate(8);
                    g.rsa.headlines.swap(0, 3);
                },
                &["W04"],
            ),
        ];
        for (name, change, want) in cases {
            let (mut ag, mut k) = (group(), kit());
            change(&mut ag, &mut k);
            let mut got = codes(&ag, &k);
            got.dedup();
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn issues_name_the_limit_and_the_value() {
        let (mut ag, mut k) = (group(), kit());
        k.headlines.truncate(8);
        ag.rsa.headlines.swap(0, 3);
        let issues = merged_ad(&ag, &k, "ad_groups[0]");
        let e34 = issues.iter().find(|i| i.code == "E34").unwrap();
        assert!(e34.message.contains("13 headlines") && e34.message.contains("15"));
        let e36 = issues.iter().find(|i| i.code == "E36").unwrap();
        assert_eq!(e36.path, "ad_groups[0].rsa.headlines[0]");
        assert!(e36.message.contains("onibus sp rj"));
    }
}
