//! The report sections of a `mads optimize` run: the data it learned from and what it changed.

use std::fmt::Write as _;

use crate::{
    google::{Account, AdGroup, Campaign, Keyword, normalize},
    perf::{Live, LiveStatus},
};

const LISTED: usize = 10;

pub(crate) fn performance(md: &mut String, live: &Live) {
    let p = &live.performance;
    md.push_str("## Performance data\n\n");
    if let Some(w) = &p.window {
        let _ = writeln!(
            md,
            "Reports from {} to {} ({} days).\n",
            w.start, w.end, w.days
        );
    }
    for r in &p.reports {
        let _ = writeln!(md, "- `{}`: {:?}, {} rows", r.file, r.kind, r.rows);
    }
    for u in &p.unknown_files {
        let _ = writeln!(md, "- skipped `{u}`");
    }
    md.push('\n');
    if p.mixed_windows {
        md.push_str("The reports cover different dates, so their numbers do not add up: the cost hidden from search terms is left out. Export them again for the same dates.\n\n");
    }
    if !p.ignored_campaigns.is_empty() {
        let _ = writeln!(
            md,
            "Not part of this run, left as they are: {}.\n",
            p.ignored_campaigns.join(", ")
        );
    }
    let thin: Vec<&str> = p
        .campaigns
        .iter()
        .filter(|c| c.thin)
        .map(|c| c.name.as_str())
        .collect();
    if !thin.is_empty() {
        let _ = writeln!(
            md,
            "Too little data to judge results (under 14 days or 100 clicks), so only their structure changed: {}.\n",
            thin.join(", ")
        );
    }
    let findings = p.findings();
    if !findings.is_empty() {
        md.push_str("What the numbers say:\n\n");
        for f in findings {
            let _ = writeln!(md, "- {f}");
        }
        md.push('\n');
    }
}

/// What the new account changes in the live one. The reasons are in Budget and bids.
pub(crate) fn changes(md: &mut String, live: &Live, account: &Account, comma: bool) {
    md.push_str("## Changes\n\nCompared with the account that ran. Each campaign rationale and CPC reason is in Budget and bids.\n\n");
    let old = &live.baseline.campaigns;
    for c in &account.campaigns {
        match old.iter().find(|o| same(&o.name, &c.name)) {
            None => {
                let _ = writeln!(md, "**{}**: new campaign.\n", c.name);
            }
            Some(o) => {
                let paused = live
                    .performance
                    .campaign(&c.name)
                    .is_some_and(|p| p.live_status == Some(LiveStatus::Paused));
                campaign(md, o, c, comma, paused);
            }
        }
    }
    for o in old
        .iter()
        .filter(|o| !account.campaigns.iter().any(|c| same(&c.name, &o.name)))
    {
        let _ = writeln!(
            md,
            "**{}**: dropped, paused by the Editor import.\n",
            o.name
        );
    }
}

fn same(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

fn campaign(md: &mut String, o: &Campaign, c: &Campaign, comma: bool, paused: bool) {
    let mut lines = Vec::new();
    if o.daily_budget != c.daily_budget {
        lines.push(format!(
            "daily budget {} to {}",
            o.daily_budget.format_cpc(comma),
            c.daily_budget.format_cpc(comma)
        ));
    }
    for g in &c.ad_groups {
        match o.ad_groups.iter().find(|x| same(&x.name, &g.name)) {
            None => lines.push(format!("new ad group {}", g.name)),
            Some(x) => lines.extend(group(x, g, comma)),
        }
    }
    for x in o
        .ad_groups
        .iter()
        .filter(|x| !c.ad_groups.iter().any(|g| same(&g.name, &x.name)))
    {
        lines.push(format!("ad group {} dropped, paused by the import", x.name));
    }
    lines.extend(asset_groups(o, c));
    lines.extend(negatives("campaign", &o.negatives, &c.negatives));
    let head = if paused {
        format!("**{}** stays paused, as it is in Google Ads", c.name)
    } else {
        format!("**{}**", c.name)
    };
    if lines.is_empty() {
        let _ = writeln!(md, "{head}: no change.\n");
        return;
    }
    let _ = writeln!(md, "{head}\n");
    for l in lines {
        let _ = writeln!(md, "- {l}");
    }
    md.push('\n');
}

/// Asset groups of image and app campaigns: Editor pauses the dropped ones like ad groups.
fn asset_groups(o: &Campaign, c: &Campaign) -> Vec<String> {
    let mut out = Vec::new();
    for g in &c.asset_groups {
        match o.asset_groups.iter().find(|x| same(&x.name, &g.name)) {
            None => out.push(format!("new asset group {}", g.name)),
            Some(x) => {
                let changed = x.headlines != g.headlines
                    || x.long_headlines != g.long_headlines
                    || x.descriptions != g.descriptions;
                if changed {
                    out.push(format!("{}: new texts", g.name));
                }
            }
        }
    }
    for x in o
        .asset_groups
        .iter()
        .filter(|x| !c.asset_groups.iter().any(|g| same(&g.name, &x.name)))
    {
        out.push(format!(
            "asset group {} dropped, paused by the import",
            x.name
        ));
    }
    out
}

fn group(o: &AdGroup, g: &AdGroup, comma: bool) -> Vec<String> {
    let mut out = Vec::new();
    if o.default_cpc != g.default_cpc {
        out.push(format!(
            "{}: CPC {} to {}",
            g.name,
            o.default_cpc.format_cpc(comma),
            g.default_cpc.format_cpc(comma)
        ));
    }
    let (added, gone) = diff(&o.keywords, &g.keywords);
    if !added.is_empty() {
        out.push(format!("{}: keywords added {}", g.name, list(&added)));
    }
    if !gone.is_empty() {
        out.push(format!(
            "{}: keywords paused by the import {}",
            g.name,
            list(&gone)
        ));
    }
    out.extend(negatives(&g.name, &o.negatives, &g.negatives));
    if o.rsa != g.rsa {
        out.push(format!(
            "{}: new ad texts. Editor adds the new ad next to the old one: pause the old ad by hand",
            g.name
        ));
    }
    out
}

fn negatives(scope: &str, old: &[Keyword], new: &[Keyword]) -> Vec<String> {
    let (added, gone) = diff(old, new);
    let mut out = Vec::new();
    if !added.is_empty() {
        out.push(format!("{scope}: negatives added {}", list(&added)));
    }
    if !gone.is_empty() {
        out.push(format!(
            "{scope}: negatives dropped, remove them by hand in Google Ads (an import does not delete) {}",
            list(&gone)
        ));
    }
    out
}

/// Keywords in `new` and not in `old`, and the other way round, as `text (match)`.
fn diff(old: &[Keyword], new: &[Keyword]) -> (Vec<String>, Vec<String>) {
    let missing = |from: &[Keyword], to: &[Keyword]| -> Vec<String> {
        from.iter()
            .filter(|k| {
                !to.iter()
                    .any(|t| t.match_type == k.match_type && same(&t.text, &k.text))
            })
            .map(|k| format!("`{}` ({:?})", k.text, k.match_type).to_lowercase())
            .collect()
    };
    (missing(new, old), missing(old, new))
}

fn list(items: &[String]) -> String {
    let shown: Vec<&str> = items.iter().take(LISTED).map(String::as_str).collect();
    let more = items.len().saturating_sub(LISTED);
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{BidStrategy, Cents, Intent, MatchType, Rsa},
        perf::{CampaignPerf, Performance, ReportFile, ReportKind, Window},
    };

    fn kw(t: &str, m: MatchType) -> Keyword {
        Keyword {
            text: t.into(),
            match_type: m,
        }
    }

    fn campaign(name: &str, budget: u64, groups: Vec<AdGroup>) -> Campaign {
        Campaign {
            kind: Default::default(),
            asset_groups: Vec::new(),
            name: name.into(),
            slug: crate::input::slugify(name),
            intent: Intent::Generic,
            daily_budget: Cents(budget),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: Vec::new(),
            ad_groups: groups,
            negatives: Vec::new(),
            assets: None,
        }
    }

    fn group(name: &str, cpc: u64, keywords: Vec<Keyword>) -> AdGroup {
        AdGroup {
            name: name.into(),
            default_cpc: Cents(cpc),
            cpc_rationale: String::new(),
            final_url: String::new(),
            keywords,
            negatives: Vec::new(),
            rsa: Rsa::default(),
        }
    }

    fn fixture() -> (Live, Account) {
        let old = Account {
            brand_kit: None,
            campaigns: vec![
                campaign(
                    "Catalogo",
                    7000,
                    vec![
                        group("tintos", 120, vec![kw("malbec", MatchType::Phrase)]),
                        group("brancos", 120, vec![]),
                    ],
                ),
                campaign("Marca", 2500, vec![group("marca", 60, vec![])]),
                campaign("Antiga", 1000, vec![]),
            ],
        };
        let mut tintos = group("tintos", 120, vec![kw("malbec", MatchType::Exact)]);
        tintos.rsa.headlines = vec!["Novo".into()];
        let mut catalogo = campaign("Catalogo", 5000, vec![tintos]);
        catalogo.negatives = vec![kw("preço", MatchType::Phrase)];
        let new = Account {
            brand_kit: None,
            campaigns: vec![
                catalogo,
                campaign("Marca", 2500, vec![group("marca", 110, vec![])]),
                campaign("Nova", 1000, vec![]),
            ],
        };
        let performance = Performance {
            window: Some(Window {
                start: "2026-10-03".into(),
                end: "2026-10-05".into(),
                days: 3,
            }),
            reports: vec![ReportFile {
                file: "termos.csv".into(),
                kind: ReportKind::SearchTerms,
                rows: 1706,
                window: None,
            }],
            unknown_files: vec!["notas.csv: no known report header".into()],
            ignored_campaigns: vec!["Rotulos antigo".into()],
            campaigns: vec![CampaignPerf {
                name: "Catalogo".into(),
                thin: true,
                ..Default::default()
            }],
            mixed_windows: false,
            ..Default::default()
        };
        (
            Live {
                baseline: old,
                performance,
            },
            new,
        )
    }

    fn asset_group(name: &str, headlines: &[&str]) -> crate::google::AssetGroup {
        crate::google::AssetGroup {
            name: name.into(),
            final_url: String::new(),
            business_name: String::new(),
            headlines: headlines.iter().map(|h| h.to_string()).collect(),
            long_headlines: vec![],
            descriptions: vec![],
            search_themes: vec![],
            images: vec![],
        }
    }

    #[test]
    fn changes_cover_asset_groups_and_campaigns_paused_in_google_ads() {
        let (mut live, mut new) = fixture();
        let mut old_feed = campaign("Feed", 1000, vec![]);
        old_feed.asset_groups = vec![
            asset_group("tintos", &["A"]),
            asset_group("brancos", &["B"]),
        ];
        live.baseline.campaigns.push(old_feed);
        let mut feed = campaign("Feed", 1000, vec![]);
        feed.asset_groups = vec![asset_group("tintos", &["A2"]), asset_group("rose", &["C"])];
        new.campaigns.push(feed);
        live.performance.campaigns.push(CampaignPerf {
            name: "Marca".into(),
            live_status: Some(crate::perf::LiveStatus::Paused),
            ..Default::default()
        });
        let mut md = String::new();
        changes(&mut md, &live, &new, false);
        for needle in [
            "new asset group rose",
            "asset group brancos dropped, paused by the import",
            "tintos: new texts",
            "**Marca** stays paused, as it is in Google Ads",
        ] {
            assert!(md.contains(needle), "missing {needle} in\n{md}");
        }
    }

    #[test]
    fn performance_section_names_the_data_and_its_limits() {
        let (live, _) = fixture();
        let mut md = String::new();
        performance(&mut md, &live);
        for needle in [
            "2026-10-03 to 2026-10-05 (3 days)",
            "`termos.csv`: SearchTerms, 1706 rows",
            "skipped `notas.csv",
            "Rotulos antigo",
            "only their structure changed: Catalogo",
        ] {
            assert!(md.contains(needle), "missing {needle} in\n{md}");
        }
    }

    #[test]
    fn reports_of_different_dates_are_called_out() {
        let (mut live, _) = fixture();
        live.performance.mixed_windows = true;
        let mut md = String::new();
        performance(&mut md, &live);
        assert!(md.contains("cover different dates"), "{md}");
    }

    #[test]
    fn changes_list_what_moved_and_what_to_do_by_hand() {
        let (live, new) = fixture();
        let mut md = String::new();
        changes(&mut md, &live, &new, false);
        for needle in [
            "daily budget 70.00 to 50.00",
            "tintos: keywords added `malbec` (exact)",
            "tintos: keywords paused by the import `malbec` (phrase)",
            "ad group brancos dropped",
            "campaign: negatives added `preço` (phrase)",
            "tintos: new ad texts",
            "marca: CPC 0.60 to 1.10",
            "**Nova**: new campaign",
            "**Antiga**: dropped, paused",
        ] {
            assert!(md.contains(needle), "missing {needle} in\n{md}");
        }
    }
}
