//! What `mads optimize` shows the agents: the account as it ran and what the reports say.

use serde_json::{Value, json};

use super::output::cents_to_f64;
use crate::{
    google::normalize,
    perf::{CampaignPerf, Live, THIN_CLICKS, THIN_DAYS},
};

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
                "final_url": g.final_url, "keywords": g.keywords, "negatives": g.negatives,
                "rsa": g.rsa,
            })).collect::<Vec<_>>(),
            "asset_groups": c.asset_groups,
        })
    });
    let perf = live
        .performance
        .campaign(campaign)
        .map_or(Value::Null, |c| perf_of(c, live));
    (ran, perf)
}

fn perf_of(c: &CampaignPerf, live: &Live) -> Value {
    let mut c = c.clone();
    for g in &mut c.ad_groups {
        g.keywords
            .retain(|k| k.metrics.impressions > 0 || !k.signals.is_empty());
    }
    let mut v = serde_json::to_value(&c).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.insert("window".into(), json!(live.performance.window));
        o.insert("thin_rule".into(), json!(thin_rule()));
    }
    v
}
