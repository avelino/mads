//! Reports plus the run's account give the performance digest the agents read.

use std::collections::BTreeSet;

use super::{
    model::*,
    table::{Row, Table},
};
use crate::{
    google::{Account, AdGroup, Campaign, MatchType, contains_word_sequence, normalize},
    input::fold,
};

const CAMPAIGN: &[&str] = &["campanha", "campaign"];
const AD_GROUP: &[&str] = &["grupo de anuncios", "ad group"];
const KEYWORD: &[&str] = &["palavra chave", "keyword", "search keyword"];
const MATCH: &[&str] = &["tipo de corresp", "match type"];
const SEARCH_TERM: &[&str] = &["termo de pesquisa", "search term"];
const ADDED: &[&str] = &["adicionada excluida", "added excluded"];
const REASONS: &[&str] = &["motivos do status", "status reasons"];
const LIVE: &[&str] = &["status da campanha", "campaign status"];
const BUDGET: &[&str] = &["orcamento", "budget"];
const MAX_CPC: &[&str] = &["cpc max", "max cpc"];
const QUALITY: &[&str] = &["indice de qualidade", "quality score", "qual score"];
const ASSET: &[&str] = &["recurso", "asset"];
const ASSET_KIND: &[&str] = &["tipo de recurso", "asset type"];
const DAY: &[&str] = &["dia", "day"];

/// One file of the reports folder: its name and the table, or why it is not a report.
pub type ReportInput = (String, Result<Table, String>);

pub fn digest(files: &[ReportInput], account: &Account) -> Performance {
    let mut p = Performance {
        campaigns: account
            .campaigns
            .iter()
            .map(|c| CampaignPerf {
                name: c.name.clone(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut ignored = BTreeSet::new();
    let mut acc = Accumulated::default();
    for (file, table) in files {
        match table {
            Err(why) => p.unknown_files.push(format!("{file}: {why}")),
            Ok(t) => {
                p.window = p.window.take().or_else(|| t.window.clone());
                p.reports.push(ReportFile {
                    file: file.clone(),
                    kind: t.kind,
                    rows: t.rows().count(),
                });
                read(&mut p, &mut acc, &mut ignored, t, account);
            }
        }
    }
    p.ignored_campaigns = ignored.into_iter().collect();
    let days = p.window.as_ref().map(|w| w.days);
    for (i, c) in p.campaigns.iter_mut().enumerate() {
        finish(c, acc.campaign_metrics.contains(&i), days);
    }
    p
}

/// What the rows say beyond the digest itself.
#[derive(Default)]
struct Accumulated {
    /// Campaigns whose totals came from a campaign report, not summed from keywords.
    campaign_metrics: BTreeSet<usize>,
}

fn read(
    p: &mut Performance,
    acc: &mut Accumulated,
    ignored: &mut BTreeSet<String>,
    t: &Table,
    account: &Account,
) {
    use super::table::ReportKind::*;
    for row in t.rows() {
        let name = row.text(CAMPAIGN);
        let Some(ci) = p
            .campaigns
            .iter()
            .position(|c| normalize(&c.name) == normalize(&name))
        else {
            if !name.is_empty() {
                ignored.insert(name);
            }
            continue;
        };
        let campaign = &account.campaigns[ci];
        let c = &mut p.campaigns[ci];
        match t.kind {
            Campaigns => {
                campaign_row(c, &row);
                acc.campaign_metrics.insert(ci);
            }
            CampaignsByDay => day_row(c, &row, acc.campaign_metrics.contains(&ci)),
            Keywords => group(c, &row.text(AD_GROUP)).keywords.push(keyword(&row)),
            SearchTerms => {
                let g = row.text(AD_GROUP);
                let term = term(&row, campaign, &g);
                group(c, &g).search_terms.push(term);
            }
            Assets => group(c, &row.text(AD_GROUP)).assets.push(asset(&row)),
        }
    }
}

fn group<'a>(c: &'a mut CampaignPerf, name: &str) -> &'a mut AdGroupPerf {
    let key = normalize(name);
    let at = match c.ad_groups.iter().position(|g| normalize(&g.name) == key) {
        Some(i) => i,
        None => {
            c.ad_groups.push(AdGroupPerf {
                name: name.to_string(),
                ..Default::default()
            });
            c.ad_groups.len() - 1
        }
    };
    &mut c.ad_groups[at]
}

fn metrics(row: &Row) -> Metrics {
    let n = |names: &[&str]| row.number(names).unwrap_or(0.0);
    let mut m = Metrics {
        impressions: n(&["impr", "impressions", "impressoes"]) as u64,
        clicks: n(&["cliques", "clicks"]) as u64,
        cost: n(&["custo", "cost"]),
        conversions: n(&["conversoes", "conversions"]),
        conversion_value: n(&["valor conv", "conv value", "conversion value"]),
        ..Default::default()
    };
    m.derive();
    m
}

fn campaign_row(c: &mut CampaignPerf, row: &Row) {
    c.live_status = live_status(&row.text(LIVE));
    c.status_reasons = row.text(REASONS);
    c.budget = row.number(BUDGET);
    c.metrics = metrics(row);
    c.lost_to_budget_pct = lost_budget(row).or(c.lost_to_budget_pct);
    c.lost_to_rank_pct = lost_rank(row).or(c.lost_to_rank_pct);
}

/// A day row adds to the totals only when no campaign report gave them.
fn day_row(c: &mut CampaignPerf, row: &Row, has_totals: bool) {
    if !has_totals {
        c.metrics.add(&metrics(row));
    }
    if row.text(DAY).is_empty() {
        return;
    }
    c.lost_to_budget_pct = lost_budget(row).or(c.lost_to_budget_pct);
    c.lost_to_rank_pct = lost_rank(row).or(c.lost_to_rank_pct);
}

fn lost_budget(row: &Row) -> Option<f64> {
    row.number_words(&[
        &["perdidas", "orcamento"],
        &["perdida", "orcamento"],
        &["lost", "budget"],
    ])
}

fn lost_rank(row: &Row) -> Option<f64> {
    row.number_words(&[
        &["perdidas", "classificacao"],
        &["perdida", "classificacao"],
        &["lost", "rank"],
    ])
}

fn live_status(text: &str) -> Option<LiveStatus> {
    let t = fold(text);
    if t.starts_with("ativ") || t.starts_with("enabled") || t.starts_with("active") {
        Some(LiveStatus::Enabled)
    } else if t.starts_with("paus") {
        Some(LiveStatus::Paused)
    } else if t.starts_with("remov") {
        Some(LiveStatus::Removed)
    } else {
        None
    }
}

fn match_type(text: &str) -> Option<MatchType> {
    let t = fold(text);
    if t.contains("frase") || t.contains("phrase") {
        Some(MatchType::Phrase)
    } else if t.contains("exat") || t.contains("exact") {
        Some(MatchType::Exact)
    } else {
        None
    }
}

fn keyword(row: &Row) -> KeywordPerf {
    let reasons = row.text(REASONS);
    let quality_score = row.number(QUALITY);
    KeywordPerf {
        // Google writes phrase keywords in quotes and exact ones in brackets.
        text: normalize(row.text(KEYWORD).trim_matches(['"', '[', ']'])),
        match_type: match_type(&row.text(MATCH)),
        signals: signals(&reasons, quality_score),
        status_reasons: reasons,
        max_cpc: row.number(MAX_CPC),
        quality_score,
        first_page_bid: row.number_words(&[&["primeira", "pagina"], &["first", "page"]]),
        top_of_page_bid: row.number_words(&[&["topo", "pagina"], &["top", "page"]]),
        metrics: metrics(row),
    }
}

fn signals(reasons: &str, quality_score: Option<f64>) -> Vec<Signal> {
    let r = fold(reasons);
    let mut out = Vec::new();
    if r.contains("primeira pagina") || r.contains("first page") {
        out.push(Signal::BelowFirstPage);
    }
    if r.contains("raramente") || r.contains("rarely") {
        out.push(Signal::RarelyShown);
    }
    let low = r.contains("baixa qualidade") || r.contains("low quality");
    if low || quality_score.is_some_and(|q| q <= 4.0) {
        out.push(Signal::LowQuality);
    }
    out
}

fn term(row: &Row, campaign: &Campaign, group_name: &str) -> TermPerf {
    let text = normalize(&row.text(SEARCH_TERM));
    let ag = campaign
        .ad_groups
        .iter()
        .find(|g| normalize(&g.name) == normalize(group_name));
    let added = fold(&row.text(ADDED));
    let state = if ag.is_some_and(|g| g.keywords.iter().any(|k| k.text == text)) {
        TermState::Keyword
    } else if blocked(campaign, ag, &text) {
        TermState::Negative
    } else if added.contains("exclu") {
        TermState::Excluded
    } else if added.contains("adicion") || added.contains("added") {
        TermState::Keyword
    } else {
        TermState::New
    };
    TermPerf {
        text,
        match_type: row.text(MATCH),
        state,
        metrics: metrics(row),
    }
}

fn blocked(c: &Campaign, ag: Option<&AdGroup>, term: &str) -> bool {
    c.negatives
        .iter()
        .chain(ag.map(|g| g.negatives.as_slice()).unwrap_or_default())
        .any(|n| match n.match_type {
            MatchType::Phrase => contains_word_sequence(term, &normalize(&n.text)),
            MatchType::Exact => normalize(&n.text) == term,
        })
}

fn asset(row: &Row) -> AssetPerf {
    AssetPerf {
        text: row.text(ASSET),
        kind: row.text(ASSET_KIND),
        label: row.text_words(&[&["desempenho"], &["performance"]]),
        impressions: row
            .number(&["impr", "impressions", "impressoes"])
            .unwrap_or(0.0) as u64,
    }
}

/// Totals, the thin flag, hidden term cost, and the term list cut to the most expensive.
fn finish(c: &mut CampaignPerf, has_totals: bool, days: Option<u32>) {
    if !has_totals && c.metrics.impressions == 0 {
        let mut sum = Metrics::default();
        for k in c.ad_groups.iter().flat_map(|g| &g.keywords) {
            sum.add(&k.metrics);
        }
        c.metrics = sum;
    }
    c.thin = days.is_some_and(|d| d < THIN_DAYS) || c.metrics.clicks < THIN_CLICKS;
    let listed: f64 = c
        .ad_groups
        .iter()
        .flat_map(|g| &g.search_terms)
        .map(|t| t.metrics.cost)
        .sum();
    let any_terms = c.ad_groups.iter().any(|g| !g.search_terms.is_empty());
    if has_totals && any_terms {
        c.hidden_terms_cost = Some(((c.metrics.cost - listed).max(0.0) * 100.0).round() / 100.0);
    }
    for g in &mut c.ad_groups {
        g.search_terms_total = g.search_terms.len();
        g.cost_without_conversions = g
            .search_terms
            .iter()
            .filter(|t| t.metrics.conversions == 0.0)
            .map(|t| t.metrics.cost)
            .sum();
        g.search_terms.sort_by(|a, b| {
            b.metrics
                .cost
                .total_cmp(&a.metrics.cost)
                .then(b.metrics.impressions.cmp(&a.metrics.impressions))
        });
        g.search_terms.truncate(TERMS_PER_GROUP);
    }
}
