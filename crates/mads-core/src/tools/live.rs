//! What `mads optimize` shows the agents: the account as it ran and what the reports say.

use serde_json::{Value, json};

use super::output::cents_to_f64;
use crate::{
    google::{Keyword, MatchType, normalize},
    perf::{
        AdGroupPerf, CampaignPerf, KeywordPerf, Live, Metrics, Signal, THIN_CLICKS, THIN_DAYS,
        TermPerf,
    },
};

/// Most characters `live` plus `performance` may take in a brief. Claude Code saves a tool result
/// over its output limit to a file the agent cannot read, and the mission fails. A live catalog
/// campaign with 7 ad groups and 200 keywords reached 125k characters before this budget.
pub const BRIEF_BUDGET: usize = 50_000;
/// Search terms per ad group in a brief, the most expensive first.
pub const BRIEF_TERMS: usize = 20;
/// Keywords without traffic named as examples in the summary of each ad group.
const QUIET_EXAMPLES: usize = 5;

fn thin_rule() -> String {
    format!(
        "A campaign marked thin has under {THIN_DAYS} days or under {THIN_CLICKS} clicks: its numbers are noise. Fix structure only (bids under the first page, low quality, no ads, wrong grouping). Do not cut, pause or grow anything because of its results."
    )
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
        "window": p.window, "thin_rule": thin_rule(),
        "ignored_campaigns": p.ignored_campaigns, "campaigns": campaigns,
    });
    (account, perf)
}

/// For one campaign mission: the campaign as it ran and its detailed numbers. Keywords with no
/// impressions and no signal say nothing and are left out.
pub fn brief_view(live: &Live, campaign: &str) -> (Value, Value) {
    let key = normalize(campaign);
    let ran = live
        .baseline
        .campaigns
        .iter()
        .find(|c| normalize(&c.name) == key);
    let ran = ran.map_or(Value::Null, |c| {
        json!({
            "daily_budget": cents_to_f64(c.daily_budget),
            "negatives": c.negatives,
            "ad_groups": c.ad_groups.iter().map(|g| json!({
                "name": g.name, "default_cpc": cents_to_f64(g.default_cpc),
                "final_url": g.final_url, "keywords": notation(&g.keywords),
                "negatives": notation(&g.negatives), "rsa": g.rsa,
            })).collect::<Vec<_>>(),
            "asset_groups": c.asset_groups,
        })
    });
    let Some(c) = live.performance.campaign(campaign) else {
        return (ran, Value::Null);
    };
    // Fewer search terms until the brief fits. Terms are the part that grows with the account.
    let room = BRIEF_BUDGET.saturating_sub(ran.to_string().len());
    let mut perf = Value::Null;
    for terms in [BRIEF_TERMS, 10, 5, 0] {
        perf = perf_of(c, live, terms);
        if terms < BRIEF_TERMS {
            perf["search_terms_cut_to"] = json!(terms);
        }
        if perf.to_string().len() < room {
            break;
        }
    }
    (ran, perf)
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

fn perf_of(c: &CampaignPerf, live: &Live, terms: usize) -> Value {
    let mut head = c.clone();
    head.ad_groups.clear();
    let mut v = serde_json::to_value(&head).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.insert("metrics".into(), metrics(&c.metrics));
        o.insert(
            "ad_groups".into(),
            c.ad_groups
                .iter()
                .map(|g| group(g, terms))
                .collect::<Vec<_>>()
                .into(),
        );
        o.insert("window".into(), json!(live.performance.window));
        o.insert("thin_rule".into(), json!(thin_rule()));
    }
    v
}

/// One ad group: keywords with traffic in full, the silent ones as counts and examples.
fn group(g: &AdGroupPerf, terms: usize) -> Value {
    let (busy, quiet): (Vec<&KeywordPerf>, Vec<&KeywordPerf>) =
        g.keywords.iter().partition(|k| k.metrics.impressions > 0);
    let count = |s: Signal| quiet.iter().filter(|k| k.signals.contains(&s)).count();
    let mut v = json!({
        "name": g.name,
        "keywords": busy.iter().map(|k| keyword(k)).collect::<Vec<_>>(),
        "keywords_without_traffic": {
            "count": quiet.len(),
            "below_first_page": count(Signal::BelowFirstPage),
            "rarely_shown": count(Signal::RarelyShown),
            "low_quality": count(Signal::LowQuality),
            "examples": quiet.iter().take(QUIET_EXAMPLES).map(|k| k.text.clone()).collect::<Vec<_>>(),
        },
        "search_terms": g.search_terms.iter().take(terms).map(term).collect::<Vec<_>>(),
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
        perf::{AdGroupPerf, KeywordPerf, Metrics, Performance, Signal, TermPerf, TermState},
    };

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
    fn a_large_campaign_brief_stays_well_under_agent_cli_output_limits() {
        let (ran, perf) = brief_view(&big(), "Catalogo");
        let size = ran.to_string().len() + perf.to_string().len();
        assert!(size < BRIEF_BUDGET, "{size} chars");
    }

    #[test]
    fn a_brief_over_budget_lists_fewer_search_terms() {
        let mut live = big();
        for g in &mut live.performance.campaigns[0].ad_groups {
            for t in &mut g.search_terms {
                t.text = format!("{} {}", t.text, "vinho tinto ".repeat(25));
            }
        }
        let (ran, perf) = brief_view(&live, "Catalogo");
        let size = ran.to_string().len() + perf.to_string().len();
        assert!(size < BRIEF_BUDGET, "{size} chars");
        let terms = perf["ad_groups"][0]["search_terms"]
            .as_array()
            .unwrap()
            .len();
        assert!(terms < BRIEF_TERMS, "{terms} terms");
        assert_eq!(perf["search_terms_cut_to"], terms);
    }

    #[test]
    fn keywords_without_traffic_become_a_summary() {
        let (_, perf) = brief_view(&big(), "Catalogo");
        let g = &perf["ad_groups"][0];
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
        assert_eq!(g["search_terms"].as_array().unwrap().len(), BRIEF_TERMS);
        let kw = &g["keywords"][0];
        assert!(kw.get("quality_score").is_none(), "no nulls: {kw}");
        assert!(kw["metrics"].get("conversions").is_none(), "no zeros: {kw}");
        assert_eq!(kw["metrics"]["clicks"], 2);
    }
}
