//! Reports plus the run's account give the performance digest the agents read.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    model::*,
    table::ReportKind,
    table::{AD_GROUP, ASSET, CAMPAIGN, DAY, KEYWORD, MATCH, Row, SEARCH_TERM, Table},
};
use crate::{
    google::{Account, AdGroup, Campaign, MatchType, contains_word_sequence, normalize},
    input::fold,
};

const ADDED: &[&str] = &["adicionada excluida", "added excluded"];
const REASONS: &[&str] = &["motivos do status", "status reasons"];
const LIVE: &[&str] = &["status da campanha", "campaign status"];
const BUDGET: &[&str] = &["orcamento", "budget"];
const MAX_CPC: &[&str] = &["cpc max", "max cpc"];
const QUALITY: &[&str] = &["indice de qualidade", "quality score", "qual score"];
const ASSET_KIND: &[&str] = &["tipo de recurso", "asset type"];

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
    let mut pass = Pass::default();
    for (file, table) in files {
        match table {
            Err(why) => p.unknown_files.push(format!("{file}: {why}")),
            Ok(t) => {
                p.window = p.window.take().or_else(|| t.window.clone());
                p.reports.push(ReportFile {
                    file: file.clone(),
                    kind: t.kind,
                    rows: t.rows().count(),
                    window: t.window.clone(),
                });
                read(&mut p, &mut pass, t, account);
            }
        }
    }
    p.ignored_campaigns = pass.ignored.iter().cloned().collect();
    let windows: BTreeSet<_> = p.reports.iter().filter_map(|r| r.window.as_ref()).collect();
    p.mixed_windows = windows.len() > 1;
    let days = p.window.as_ref().map(|w| w.days);
    let end = p.window.as_ref().and_then(|w| iso_date(&w.end));
    for (i, c) in p.campaigns.iter_mut().enumerate() {
        if !pass.totals.contains(&i)
            && let Some((budget, rank)) = pass.day_lost.get(&i)
        {
            c.lost_to_budget_pct = mean(budget);
            c.lost_to_rank_pct = mean(rank);
        }
        c.days_running = pass
            .first_active
            .get(&i)
            .and_then(|first| days_between(iso_date(first)?, end?));
        finish(c, pass.totals.contains(&i), days, p.mixed_windows);
    }
    mark_untracked(&mut p.campaigns);
    p.idle_budget = idle_budget(&p.campaigns);
    p.missing_reports = missing_reports(&p.reports);
    p
}

/// `2026-10-09`, as day rows and `Window` write a date.
fn iso_date(text: &str) -> Option<time::Date> {
    let mut parts = text.trim().splitn(3, '-').map(str::parse::<u32>);
    let (y, m, d) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    let month = time::Month::try_from(u8::try_from(m).ok()?).ok()?;
    time::Date::from_calendar_date(i32::try_from(y).ok()?, month, u8::try_from(d).ok()?).ok()
}

/// Both days included.
fn days_between(first: time::Date, last: time::Date) -> Option<u32> {
    u32::try_from((last - first).whole_days() + 1).ok()
}

/// Clicks enough to judge and no conversion, while another campaign converts.
fn mark_untracked(campaigns: &mut [CampaignPerf]) {
    if !campaigns.iter().any(|c| c.metrics.conversions > 0.0) {
        return;
    }
    for c in campaigns {
        c.untracked = c.metrics.clicks >= THIN_CLICKS && c.metrics.conversions == 0.0;
    }
}

fn idle_budget(campaigns: &[CampaignPerf]) -> Option<f64> {
    let idle: Vec<f64> = campaigns
        .iter()
        .filter(|c| c.live_status == Some(LiveStatus::Enabled))
        .filter_map(|c| Some((c.budget? - c.avg_daily_cost?).max(0.0)))
        .collect();
    (!idle.is_empty()).then(|| round2(idle.iter().sum()))
}

fn missing_reports(reports: &[ReportFile]) -> Vec<ReportKind> {
    use ReportKind::*;
    let has = |kinds: &[ReportKind]| reports.iter().any(|r| kinds.contains(&r.kind));
    [
        (Campaigns, has(&[Campaigns, CampaignsByDay])),
        (Keywords, has(&[Keywords])),
        (SearchTerms, has(&[SearchTerms])),
        (Assets, has(&[Assets])),
    ]
    .into_iter()
    .filter(|(_, present)| !present)
    .map(|(kind, _)| kind)
    .collect()
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// What the rows say beyond the digest itself, kept while the files are read.
#[derive(Default)]
struct Pass {
    ignored: BTreeSet<String>,
    /// Campaigns whose totals came from a campaign report, not summed from other rows.
    totals: BTreeSet<usize>,
    /// Lost impression share of every day row, per campaign: budget, then rank.
    day_lost: BTreeMap<usize, (Vec<f64>, Vec<f64>)>,
    /// Latest day read per campaign: its row gives the budget and status.
    latest_day: BTreeMap<usize, String>,
    /// First day with impressions per campaign.
    first_active: BTreeMap<usize, String>,
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty())
        .then(|| (values.iter().sum::<f64>() / values.len() as f64 * 100.0).round() / 100.0)
}

fn read(p: &mut Performance, pass: &mut Pass, t: &Table, account: &Account) {
    use super::table::ReportKind::*;
    for row in t.rows() {
        let name = row.text(CAMPAIGN);
        let Some(ci) = p
            .campaigns
            .iter()
            .position(|c| normalize(&c.name) == normalize(&name))
        else {
            if !name.is_empty() {
                pass.ignored.insert(name);
            }
            continue;
        };
        let campaign = &account.campaigns[ci];
        let c = &mut p.campaigns[ci];
        match t.kind {
            Campaigns => {
                campaign_row(c, &row);
                pass.totals.insert(ci);
            }
            CampaignsByDay => {
                let has_totals = pass.totals.contains(&ci);
                day_row(c, &row, has_totals, pass.day_lost.entry(ci).or_default());
                let day = row.text(DAY);
                if day.is_empty() {
                    continue;
                }
                if !has_totals && pass.latest_day.get(&ci).is_none_or(|d| *d <= day) {
                    status(c, &row);
                    pass.latest_day.insert(ci, day.clone());
                }
                if metrics(&row).impressions > 0 {
                    let first = pass.first_active.entry(ci).or_insert_with(|| day.clone());
                    if day < *first {
                        *first = day;
                    }
                }
            }
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
    status(c, row);
    c.metrics = metrics(row);
    c.lost_to_budget_pct = lost_budget(row).or(c.lost_to_budget_pct);
    c.lost_to_rank_pct = lost_rank(row).or(c.lost_to_rank_pct);
}

fn status(c: &mut CampaignPerf, row: &Row) {
    c.live_status = live_status(&row.text(LIVE));
    c.status_reasons = row.text(REASONS);
    c.budget = row.number(BUDGET);
}

/// A day row adds to the totals only when no campaign report gave them. Its lost share is kept
/// to average over the days: the last day alone would say nothing about the period.
fn day_row(c: &mut CampaignPerf, row: &Row, has_totals: bool, lost: &mut (Vec<f64>, Vec<f64>)) {
    if !has_totals {
        c.metrics.add(&metrics(row));
    }
    if row.text(DAY).is_empty() {
        return;
    }
    lost.0.extend(lost_budget(row));
    lost.1.extend(lost_rank(row));
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

/// Enabled and going nowhere: Google says it has no ads, or a week passed without one impression.
fn stalled(c: &CampaignPerf, days: Option<u32>) -> bool {
    if c.live_status != Some(LiveStatus::Enabled) {
        return false;
    }
    let reasons = fold(&c.status_reasons);
    let no_ads = reasons.contains("nenhum anuncio") || reasons.contains("no ads");
    let silent = c.metrics.impressions == 0 && days.is_some_and(|d| d >= STALLED_DAYS);
    no_ads || silent
}

/// Totals, the thin flag, hidden term cost, and the term list cut to the most expensive.
fn finish(c: &mut CampaignPerf, has_totals: bool, days: Option<u32>, mixed_windows: bool) {
    let empty = c.metrics.impressions == 0 && c.metrics.clicks == 0 && c.metrics.cost == 0.0;
    if !has_totals && empty {
        let mut sum = Metrics::default();
        for k in c.ad_groups.iter().flat_map(|g| &g.keywords) {
            sum.add(&k.metrics);
        }
        c.metrics = sum;
    }
    let days = c.days_running.or(days);
    if let Some(d) = days.filter(|d| *d > 0) {
        let avg = round2(c.metrics.cost / f64::from(d));
        c.avg_daily_cost = Some(avg);
        c.budget_use_pct = c
            .budget
            .filter(|b| *b > 0.0)
            .map(|b| round2(avg / b * 100.0));
    }
    c.stalled = stalled(c, days);
    let short = days.is_some_and(|d| d < THIN_DAYS) || c.metrics.clicks < THIN_CLICKS;
    c.thin = !c.stalled && c.metrics.conversions < JUDGE_CONVERSIONS && short;
    let listed: f64 = c
        .ad_groups
        .iter()
        .flat_map(|g| &g.search_terms)
        .map(|t| t.metrics.cost)
        .sum();
    let any_terms = c.ad_groups.iter().any(|g| !g.search_terms.is_empty());
    // Campaign cost minus term cost only means something when both cover the same days.
    if has_totals && any_terms && !mixed_windows {
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
