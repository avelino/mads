//! What `mads optimize` shows the agents: the account as it ran and what the reports say.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    MissionTools, ToolOutput, ToolSpec,
    output::{cents_to_f64, parse_args},
    schema::schema_for,
};
use crate::{
    google::{AdGroup, Campaign, Issue, Keyword, MatchType, normalize},
    perf::{
        AdGroupPerf, JUDGE_CONVERSIONS, KeywordPerf, Live, Metrics, Performance, ReportKind,
        STALLED_DAYS, Signal, THIN_CLICKS, THIN_DAYS, TermPerf,
    },
};

// The brief of a live campaign carries a summary per ad group, and `get_ad_group_performance`
// gives one group in detail. One call with every group reached 56k characters for 7 groups and
// 200 keywords: Claude Code saves a result that large to a file the agent cannot read.

/// Search terms listed per ad group, the most expensive first.
pub const GROUP_TERMS: usize = 20;
/// Keywords without traffic named as examples in the detail of an ad group.
const QUIET_EXAMPLES: usize = 5;

pub const DETAIL_HINT: &str = "live and performance are summaries. Call get_ad_group_performance with each ad group name before you rebuild it: it has the keywords, negatives and texts as they ran, and their numbers.";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct AdGroupArgs {
    /// Name of an ad group or asset group of this campaign as it ran.
    ad_group: String,
}

pub(super) fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_ad_group_performance".into(),
        description: "One ad group of this campaign as it ran in Google Ads, with its numbers: keywords, negatives, ad texts, signals, search terms and asset labels.".into(),
        input_schema: schema_for::<AdGroupArgs>(),
    }
}

pub(super) async fn call(t: &MissionTools, slug: &str, args: Value) -> ToolOutput {
    let a: AdGroupArgs = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let ws = t.ws.lock().await;
    let (Some(live), Some(c)) = (
        &ws.live,
        ws.account.campaigns.iter().find(|c| c.slug == slug),
    ) else {
        return ToolOutput::fail("NOT_FOUND", format!("campaign '{slug}' has no live data"));
    };
    match ad_group_view(live, &c.name, &a.ad_group) {
        Some(v) => {
            if let Ok(mut seen) = t.detailed.lock() {
                seen.insert(normalize(&a.ad_group));
            }
            ToolOutput::ok(v, &[], format!("get_ad_group_performance {}", a.ad_group))
        }
        None => {
            let known = known_groups(live, &c.name).join(", ");
            let msg = format!(
                "'{}' did not run in this campaign. Groups that ran: {known}",
                a.ad_group
            );
            ToolOutput::fail("NOT_FOUND", msg)
        }
    }
}

fn thin_rule() -> String {
    format!(
        "A campaign marked thin has under {THIN_DAYS} days running or under {THIN_CLICKS} clicks, and under {JUDGE_CONVERSIONS} conversions: its numbers are noise. Fix structure only (bids under the first page, low quality, no ads, wrong grouping). Do not cut, pause or grow anything because of its results. stalled and untracked win over thin for the budget: cut those even when thin."
    )
}

/// What the flags beyond `thin` mean, and what the missing reports leave blind.
fn live_rules(p: &Performance) -> String {
    let mut rules = vec![
        format!(
            "stalled: enabled with no ads, or {STALLED_DAYS} days without one impression. It is broken, not short of data: fix what Google says (no ads, policy) or cut its budget. It never gets more budget (E27)."
        ),
        "untracked: clicks enough to judge and no conversion while other campaigns convert. Its result is not measured, or there is none. Do not grow it: no budget increase (E27), no new keywords or groups.".into(),
        "budget_use_pct is the average daily cost over the daily budget. Under 100 the budget is not what limits the campaign, so cutting it down to its real spend frees money without changing what it gets. At 100 or more a converting campaign is limited by its budget: give it what the others leave unspent, in steps of about 20 percent per run.".into(),
        "A policy limit (status reasons with 'política' or 'policy') is not lifted by a higher bid.".into(),
    ];
    if let Some(idle) = p.idle_budget {
        rules.push(format!(
            "idle_budget: {idle:.2} a day of the budget goes unspent. Move it to campaigns that convert at their budget limit."
        ));
    }
    let blind = p.blind_reports();
    if !blind.is_empty() {
        rules.push(format!(
            "No {} report: keyword numbers are unknown. Keep the keywords that ran, change bids and texts only.",
            blind.join(" or ")
        ));
    }
    rules.join(" ")
}

/// For the plan: every campaign as it ran, and campaign totals without keyword detail.
pub fn business_view(live: &Live) -> (Value, Value) {
    let account = json!({"campaigns": live.baseline.campaigns.iter().map(|c| json!({
        "name": c.name, "kind": c.kind, "intent": c.intent,
        "daily_budget": cents_to_f64(c.daily_budget), "bid_strategy": c.bid_strategy,
        "ad_groups": c.planned_ad_groups.iter().map(|g| g.name.clone()).collect::<Vec<_>>(),
    })).collect::<Vec<_>>()});
    let p = &live.performance;
    let campaigns: Vec<Value> = p
        .campaigns
        .iter()
        .map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                o.remove("ad_groups");
            }
            v
        })
        .collect();
    let perf = json!({
        "window": p.window, "thin_rule": thin_rule(), "live_rules": live_rules(p),
        "idle_budget": p.idle_budget, "missing_reports": p.missing_reports,
        "ignored_campaigns": p.ignored_campaigns, "campaigns": campaigns,
    });
    (account, perf)
}

/// LIVE_DETAIL: a group that ran, rebuilt before this mission read its detail. None: go ahead.
pub(super) fn detail_first(
    t: &MissionTools,
    live: &Live,
    campaign: &str,
    group: &str,
) -> Option<ToolOutput> {
    ad_group_view(live, campaign, group)?;
    let seen = t
        .detailed
        .lock()
        .map(|s| s.contains(&normalize(group)))
        .unwrap_or(false);
    (!seen).then(|| {
        let msg = format!(
            "'{group}' ran in Google Ads: call get_ad_group_performance with it first, then rebuild it from what ran and its numbers"
        );
        ToolOutput::fail("LIVE_DETAIL", msg)
    })
}

/// What a live ad group may not change: W10 puts back the keywords that ran when the keywords
/// report that would justify dropping them is missing, E28 refuses a higher bid in an untracked
/// campaign. One optimize run dropped the keyword with 43% of the Search cost without a keyword
/// report, and raised every bid of a campaign whose clicks converted into nothing measurable.
pub(super) fn guard_ran_group(live: &Live, campaign: &str, g: &mut AdGroup) -> Vec<Issue> {
    let key = normalize(&g.name);
    let Some(ran) = ran_campaign(live, campaign)
        .and_then(|c| c.ad_groups.iter().find(|r| normalize(&r.name) == key))
    else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    if live
        .performance
        .missing_reports
        .contains(&ReportKind::Keywords)
    {
        let back: Vec<Keyword> = ran
            .keywords
            .iter()
            .filter(|k| !g.keywords.contains(k))
            .cloned()
            .collect();
        if !back.is_empty() {
            let msg = format!(
                "kept {} keywords that ran, no keywords report to judge them: {}",
                back.len(),
                notation(&back[..back.len().min(QUIET_EXAMPLES)]).join(", ")
            );
            issues.push(Issue::warning("W10", "keywords", msg));
            g.keywords.extend(back);
        }
    }
    let untracked = live
        .performance
        .campaign(campaign)
        .is_some_and(|p| p.untracked);
    if untracked && g.default_cpc > ran.default_cpc {
        let msg = format!(
            "this campaign is untracked: its clicks bring no conversion anyone can see, so a higher bid buys more of them. Keep default_cpc at {} or lower",
            cents_to_f64(ran.default_cpc)
        );
        issues.push(Issue::error("E28", "default_cpc", msg));
    }
    issues
}

fn ran_campaign<'a>(live: &'a Live, campaign: &str) -> Option<&'a Campaign> {
    let key = normalize(campaign);
    live.baseline
        .campaigns
        .iter()
        .find(|c| normalize(&c.name) == key)
}

fn known_groups(live: &Live, campaign: &str) -> Vec<String> {
    let mut names: Vec<String> = ran_campaign(live, campaign)
        .map(|c| {
            c.ad_groups
                .iter()
                .map(|g| g.name.clone())
                .chain(c.asset_groups.iter().map(|g| g.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    if let Some(p) = live.performance.campaign(campaign) {
        for g in &p.ad_groups {
            if !names.iter().any(|n| normalize(n) == normalize(&g.name)) {
                names.push(g.name.clone());
            }
        }
    }
    names
}

/// For one campaign mission: the campaign as it ran and its numbers, one line per ad group.
pub fn brief_view(live: &Live, campaign: &str) -> (Value, Value) {
    let ran = ran_campaign(live, campaign).map_or(Value::Null, |c| {
        json!({
            "daily_budget": cents_to_f64(c.daily_budget),
            "negatives": notation(&c.negatives),
            "ad_groups": c.ad_groups.iter().map(|g| json!({
                "name": g.name, "default_cpc": cents_to_f64(g.default_cpc),
                "keywords": g.keywords.len(), "negatives": g.negatives.len(),
            })).collect::<Vec<_>>(),
            "asset_groups": c.asset_groups.iter().map(|g| g.name.clone()).collect::<Vec<_>>(),
        })
    });
    let perf = live
        .performance
        .campaign(campaign)
        .map_or(Value::Null, |c| {
            let mut head = c.clone();
            head.ad_groups.clear();
            let mut v = serde_json::to_value(&head).unwrap_or(Value::Null);
            v["metrics"] = metrics(&c.metrics);
            v["ad_groups"] = c.ad_groups.iter().map(summary).collect::<Vec<_>>().into();
            v["window"] = json!(live.performance.window);
            v["thin_rule"] = json!(thin_rule());
            v["live_rules"] = json!(live_rules(&live.performance));
            v
        });
    (ran, perf)
}

/// One ad group in a line: its totals and how many keywords carry each signal.
fn summary(g: &AdGroupPerf) -> Value {
    let mut total = Metrics::default();
    for k in &g.keywords {
        total.add(&k.metrics);
    }
    let count = |s: Signal| g.keywords.iter().filter(|k| k.signals.contains(&s)).count();
    json!({
        "name": g.name,
        "metrics": metrics(&total),
        "keywords": g.keywords.len(),
        "keywords_with_traffic": g.keywords.iter().filter(|k| k.metrics.impressions > 0).count(),
        "signals": {
            "below_first_page": count(Signal::BelowFirstPage),
            "rarely_shown": count(Signal::RarelyShown),
            "low_quality": count(Signal::LowQuality),
        },
        "search_terms_total": g.search_terms_total,
        "cost_without_conversions": round(g.cost_without_conversions),
    })
}

/// One ad group or asset group as it ran and its numbers. None when neither the run nor the
/// reports know it.
pub fn ad_group_view(live: &Live, campaign: &str, name: &str) -> Option<Value> {
    let key = normalize(name);
    let c = ran_campaign(live, campaign);
    let search = c.and_then(|c| c.ad_groups.iter().find(|g| normalize(&g.name) == key));
    let asset = c.and_then(|c| c.asset_groups.iter().find(|g| normalize(&g.name) == key));
    let perf = live
        .performance
        .campaign(campaign)
        .and_then(|p| p.ad_groups.iter().find(|g| normalize(&g.name) == key));
    let ran = match (search, asset) {
        (Some(g), _) => json!({
            "name": g.name, "default_cpc": cents_to_f64(g.default_cpc), "final_url": g.final_url,
            "keywords": notation(&g.keywords), "negatives": notation(&g.negatives), "rsa": g.rsa,
        }),
        (None, Some(g)) => json!(g),
        (None, None) if perf.is_none() => return None,
        (None, None) => Value::Null,
    };
    let shown = search
        .map(|g| g.name.clone())
        .or(asset.map(|g| g.name.clone()))
        .or(perf.map(|g| g.name.clone()))
        .unwrap_or_default();
    Some(json!({"ad_group": shown, "live": ran, "performance": perf.map_or(Value::Null, detail)}))
}

/// Keywords as Google writes them: `"phrase"` and `[exact]`. A third of the size of objects.
fn notation(keywords: &[Keyword]) -> Vec<String> {
    keywords
        .iter()
        .map(|k| match k.match_type {
            MatchType::Phrase => format!("\"{}\"", k.text),
            MatchType::Exact => format!("[{}]", k.text),
        })
        .collect()
}

/// One ad group: keywords with traffic in full, the silent ones as counts and examples.
fn detail(g: &AdGroupPerf) -> Value {
    let (busy, quiet): (Vec<&KeywordPerf>, Vec<&KeywordPerf>) =
        g.keywords.iter().partition(|k| k.metrics.impressions > 0);
    let count = |s: Signal| quiet.iter().filter(|k| k.signals.contains(&s)).count();
    let mut v = json!({
        "keywords": busy.iter().map(|k| keyword(k)).collect::<Vec<_>>(),
        "keywords_without_traffic": {
            "count": quiet.len(),
            "below_first_page": count(Signal::BelowFirstPage),
            "rarely_shown": count(Signal::RarelyShown),
            "low_quality": count(Signal::LowQuality),
            "examples": quiet.iter().take(QUIET_EXAMPLES).map(|k| k.text.clone()).collect::<Vec<_>>(),
        },
        "search_terms": g.search_terms.iter().take(GROUP_TERMS).map(term).collect::<Vec<_>>(),
        "search_terms_total": g.search_terms_total,
        "cost_without_conversions": round(g.cost_without_conversions),
    });
    if !g.assets.is_empty() {
        v["assets"] = json!(g.assets);
    }
    v
}

fn keyword(k: &KeywordPerf) -> Value {
    let mut v = json!({"text": k.text, "match_type": k.match_type, "metrics": metrics(&k.metrics)});
    if !k.signals.is_empty() {
        v["signals"] = json!(k.signals);
    }
    for (key, value) in [
        ("max_cpc", k.max_cpc),
        ("quality_score", k.quality_score),
        ("first_page_bid", k.first_page_bid),
        ("top_of_page_bid", k.top_of_page_bid),
    ] {
        if let Some(x) = value {
            v[key] = json!(x);
        }
    }
    v
}

fn term(t: &TermPerf) -> Value {
    json!({"text": t.text, "match_type": t.match_type, "state": t.state, "metrics": metrics(&t.metrics)})
}

/// Only the numbers that are there: zeros and empty ratios say nothing and cost characters.
fn metrics(m: &Metrics) -> Value {
    let mut v = json!({});
    for (key, value) in [
        ("impressions", m.impressions as f64),
        ("clicks", m.clicks as f64),
        ("cost", m.cost),
        ("conversions", m.conversions),
        ("conversion_value", m.conversion_value),
    ] {
        if value != 0.0 {
            v[key] = if value.fract() == 0.0 {
                json!(value as u64)
            } else {
                json!(round(value))
            };
        }
    }
    for (key, value) in [
        ("ctr_pct", m.ctr_pct),
        ("avg_cpc", m.avg_cpc),
        ("cost_per_conversion", m.cost_per_conversion),
    ] {
        if let Some(x) = value {
            v[key] = json!(x);
        }
    }
    v
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{Account, AdGroup, BidStrategy, Campaign, Cents, Intent, Keyword, MatchType, Rsa},
        perf::{
            AdGroupPerf, CampaignPerf, KeywordPerf, Metrics, Performance, Signal, TermPerf,
            TermState,
        },
    };

    /// What the tests hold the live parts of a brief and one group detail to.
    const BRIEF_BUDGET: usize = 15_000;
    const GROUP_BUDGET: usize = 15_000;

    /// The size of the live catalog campaign that broke Claude Code's tool output limit:
    /// 7 ad groups, 30 keywords each with a signal and no traffic, 30 terms each.
    fn big() -> Live {
        let kws: Vec<Keyword> = (0..30)
            .map(|i| Keyword {
                text: format!("vinho tinto suave numero {i}"),
                match_type: MatchType::Phrase,
            })
            .collect();
        let groups: Vec<AdGroup> = (0..7)
            .map(|g| AdGroup {
                name: format!("Grupo de uvas numero {g}"),
                default_cpc: Cents(120),
                cpc_rationale: String::new(),
                final_url: "https://vinellu.com/uvas".into(),
                keywords: kws.clone(),
                negatives: vec![],
                rsa: Rsa::default(),
            })
            .collect();
        let campaign = Campaign {
            kind: Default::default(),
            asset_groups: vec![],
            name: "Catalogo".into(),
            slug: "catalogo".into(),
            intent: Intent::Catalog,
            daily_budget: Cents(7000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: groups,
            negatives: vec![],
            assets: None,
        };
        let mut traffic = Metrics {
            impressions: 40,
            clicks: 2,
            cost: 2.4,
            ..Default::default()
        };
        traffic.derive();
        let perf_groups = (0..7)
            .map(|g| AdGroupPerf {
                name: format!("Grupo de uvas numero {g}"),
                keywords: (0..30)
                    .map(|i| KeywordPerf {
                        text: format!("vinho tinto suave numero {i}"),
                        match_type: Some(MatchType::Phrase),
                        status_reasons: "raramente exibido; abaixo do lance de primeira página"
                            .into(),
                        signals: vec![Signal::RarelyShown, Signal::BelowFirstPage],
                        max_cpc: Some(1.2),
                        quality_score: None,
                        first_page_bid: None,
                        top_of_page_bid: None,
                        metrics: if i == 0 {
                            traffic.clone()
                        } else {
                            Metrics::default()
                        },
                    })
                    .collect(),
                search_terms: (0..30)
                    .map(|i| TermPerf {
                        text: format!("melhor vinho tinto para jantar numero {i}"),
                        match_type: "Correspondência de frase (variação aproximada)".into(),
                        state: TermState::New,
                        metrics: traffic.clone(),
                    })
                    .collect(),
                search_terms_total: 160,
                ..Default::default()
            })
            .collect();
        Live {
            performance: Performance {
                campaigns: vec![CampaignPerf {
                    name: "Catalogo".into(),
                    thin: true,
                    ad_groups: perf_groups,
                    ..Default::default()
                }],
                ..Default::default()
            },
            baseline: Account {
                brand_kit: None,
                campaigns: vec![campaign],
            },
        }
    }

    #[test]
    fn a_large_campaign_brief_is_a_small_summary() {
        let (ran, perf) = brief_view(&big(), "Catalogo");
        let size = ran.to_string().len() + perf.to_string().len();
        assert!(size < BRIEF_BUDGET, "{size} chars");
        let g = &perf["ad_groups"][0];
        assert_eq!(g["name"], "Grupo de uvas numero 0");
        assert_eq!(g["keywords"], 30);
        assert_eq!(g["keywords_with_traffic"], 1);
        assert_eq!(g["signals"]["below_first_page"], 30);
        assert_eq!(g["search_terms_total"], 160);
        assert!(
            g.get("search_terms").is_none(),
            "detail is in get_ad_group_performance"
        );
        let r = &ran["ad_groups"][0];
        assert_eq!(
            (r["default_cpc"].as_f64(), r["keywords"].as_u64()),
            (Some(1.2), Some(30))
        );
    }

    #[test]
    fn one_ad_group_detail_stays_small_even_with_long_terms() {
        let mut live = big();
        for g in &mut live.performance.campaigns[0].ad_groups {
            for t in &mut g.search_terms {
                t.text = format!("{} {}", t.text, "vinho tinto ".repeat(25));
            }
        }
        let v = ad_group_view(&live, "Catalogo", "grupo de UVAS numero 3").unwrap();
        assert!(
            v.to_string().len() < GROUP_BUDGET,
            "{} chars",
            v.to_string().len()
        );
        assert_eq!(v["ad_group"], "Grupo de uvas numero 3");
        assert_eq!(v["live"]["keywords"].as_array().unwrap().len(), 30);
        assert_eq!(v["live"]["keywords"][0], "\"vinho tinto suave numero 0\"");
        assert!(ad_group_view(&live, "Catalogo", "nope").is_none());
        assert!(ad_group_view(&live, "Outra", "Grupo de uvas numero 3").is_none());
    }

    #[test]
    fn keywords_without_traffic_become_a_summary() {
        let v = ad_group_view(&big(), "Catalogo", "Grupo de uvas numero 0").unwrap();
        let g = &v["performance"];
        assert_eq!(
            g["keywords"].as_array().unwrap().len(),
            1,
            "only the one with traffic"
        );
        let quiet = &g["keywords_without_traffic"];
        assert_eq!(quiet["count"], 29);
        assert_eq!(quiet["below_first_page"], 29);
        assert_eq!(quiet["rarely_shown"], 29);
        assert_eq!(quiet["examples"].as_array().unwrap().len(), 5);
        assert_eq!(g["search_terms"].as_array().unwrap().len(), GROUP_TERMS);
        let kw = &g["keywords"][0];
        assert!(kw.get("quality_score").is_none(), "no nulls: {kw}");
        assert!(kw["metrics"].get("conversions").is_none(), "no zeros: {kw}");
        assert_eq!(kw["metrics"]["clicks"], 2);
    }
}
