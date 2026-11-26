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
    google::{AdGroup, Campaign, CampaignKind, Cents, Issue, Keyword, MatchType, normalize},
    input::Budget,
    perf::{
        AdGroupPerf, CampaignPerf, DISPLAY_CTR_PCT, DRIFT_SOURCE, Drift, JUDGE_CONVERSIONS,
        KeywordPerf, Live, Metrics, NO_ACTION_FOR_STORE, NO_RESULT_TARGETS, Performance,
        ReportKind, STALLED_DAYS, Signal, THIN_CLICKS, THIN_DAYS, TermPerf, VsTarget, vs_target,
        vs_target_in_app,
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
        "A campaign marked thin has under {THIN_DAYS} days running or under {THIN_CLICKS} clicks, and under {JUDGE_CONVERSIONS} conversions: its numbers are noise. Fix structure only (bids under the first page, low quality, no ads, wrong grouping). Do not cut, pause or grow anything because of its results. Its budget still follows budget_use_pct: money it does not spend is not a result. stalled and untracked win over thin for the budget: cut those even when thin."
    )
}

/// What the flags beyond `thin` mean, and what the missing reports leave blind.
fn live_rules(p: &Performance) -> String {
    let mut rules = vec![
        format!(
            "stalled: enabled with no ads, or {STALLED_DAYS} days without one impression. It is broken, not short of data: fix what Google says (no ads, policy) or cut its budget. It never gets more budget (E27)."
        ),
        "untracked: clicks enough to judge and no conversion while other campaigns convert. Its result is not measured, or there is none. Do not grow it: no budget increase (E27), no new keywords or groups.".into(),
        format!(
            "budget_use_pct is the average daily cost over the daily budget. Under 100 the budget is not what limits the campaign, thin or not: cut it down to its real spend plus a tenth, which frees money without changing what it gets. Leave it higher only when this run fixes what kept it from spending (bids under the first page, no ads), and say so in the rationale. At 90 or more a converting campaign is limited by its budget: give it what the others leave unspent, in steps of about 20 percent per run, or up to 40 percent when it has {JUDGE_CONVERSIONS} conversions or more in the window, because its cost per conversion is proven."
        ),
        "bid_strategy is how the campaign bids in Google Ads today. When it is not the strategy of live_account, it was changed outside mads (by hand or by auto-applied recommendations) and the import keeps it: under an automatic strategy the ad group CPCs are not in use, so an avg_cpc under default_cpc says nothing about the bid.".into(),
        "An ad group with not_in_run serves in Google Ads from an older import, next to the groups of this run, and the import pauses it. Its numbers are searches of this campaign: read them with the group of this run that covers the same theme, and reuse its exact name only if it should keep serving.".into(),
        "A policy limit (status reasons with 'política' or 'policy') is not lifted by a higher bid.".into(),
    ];
    if p.campaigns.iter().any(|c| c.app.is_some()) {
        rules.push(APP_EVENTS_RULE.into());
    }
    rules.extend(report_rules(p));
    rules.extend(outside_rules(p));
    rules.extend(drift_rules(&p.drift));
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

/// What the conversion action and network reports add to the campaigns, when they were read.
fn report_rules(p: &Performance) -> Vec<String> {
    let any = |f: fn(&CampaignPerf) -> bool| p.campaigns.iter().any(f);
    let mut rules = Vec::new();
    if any(|c| c.untracked_reason.is_some()) {
        rules.push(format!(
            "untracked_reason '{NO_ACTION_FOR_STORE}' means no conversion action counted for the store of that App campaign, for example an iOS campaign while only an Android download action exists. Its installs are not measured: it gets no more budget (E27), and its rationale says the conversion action of that store must be added in Google Ads."
        ));
    }
    if any(|c| !c.off_goal_actions.is_empty()) {
        rules.push("off_goal_actions lists the conversion actions a Search or Demand Gen campaign counted that are not installs, in an account whose goal is installs. Its cost per conversion does not measure the goal: do not move budget to or from it on that number alone.".into());
    }
    if any(|c| !c.networks.is_empty()) {
        rules.push("networks is the share of the campaign's cost and conversions per network, and the CTR of each.".into());
    }
    if any(|c| c.accidental_display_clicks) {
        rules.push(format!(
            "accidental_display_clicks: the Display CTR of this App campaign is above {DISPLAY_CTR_PCT} percent, a line mads draws and not a Google rule. Its clicks are likely accidental: treat its install count with caution and do not raise its budget on cost per install alone."
        ));
    }
    rules
}

/// The money of campaigns the run does not manage, when some of them spend without converting.
fn outside_rules(p: &Performance) -> Vec<String> {
    if p.outside_blind().is_empty() {
        return Vec::new();
    }
    let fate = if p.pause_outside_run.is_empty() {
        "This run leaves them running."
    } else {
        "The import pauses them (--pause-outside-run)."
    };
    vec![format!(
        "outside_campaigns are enabled campaigns of the account that this run does not manage, with their share of the account cost, that spend without converting while the account converts. That money is outside the plan: the daily budget you split does not include it, and you do not plan these campaigns. {fate} Do not treat their spend as room to grow the campaigns of the run."
    )]
}

/// What changed in Google Ads since the last run, when something did.
fn drift_rules(d: &Drift) -> Vec<String> {
    let mut rules = Vec::new();
    if d.is_empty() {
        return rules;
    }
    rules.push(format!(
        "drift lists what is not as the last run exported it: {DRIFT_SOURCE}, and the reports cannot tell which. A campaign paused or removed there still comes enabled if you keep it, so keep it only if it is worth the money."
    ));
    if d.budgets_moved() {
        rules.push("Budgets were changed outside mads: the daily_budget of live_account and the budget in performance are what the campaigns run with now, so plan from that live baseline and not from the budgets the last run exported. When the live budgets add up to more than the account budget (account_budget_pct), the account spends more than it is asked to: cut what does not spend before you move money to others.".into());
    }
    rules
}

const APP_EVENTS_RULE: &str = "app_installs_seen is what the advertiser's own analytics counted for the store of an App campaign, in_app_actions what those installs led to, install_to_action_pct their share and cost_per_in_app_action what each action cost. store_vs_app_install_ratio is Google's conversions over the installs the app saw: well above 1 means Google counts installs the app never sees, so the cost per install the store reports is a weak signal. A store whose installs rarely become in-app actions should not get more budget just because its cost per install is low: weigh its App campaign by cost_per_in_app_action.";

/// What the advertiser accepts to pay for a result of this kind of campaign, in currency units.
/// An install is not a conversion on the site, so App campaigns have their own target.
fn target_for(kind: CampaignKind, b: &Budget) -> Option<f64> {
    let target = if kind == CampaignKind::AppInstalls {
        b.target_cpi
    } else {
        b.target_cpa
    };
    target.map(cents_to_f64)
}

/// How a campaign that ran did against the target of its kind. None without a target or
/// without numbers that say.
pub(super) fn verdict_of(live: &Live, b: &Budget, name: &str) -> Option<VsTarget> {
    let kind = live
        .baseline
        .campaigns
        .iter()
        .find(|c| normalize(&c.name) == normalize(name))?
        .kind;
    let perf = live.performance.campaign(name)?;
    if let Some(target) = in_app_target(kind, b, perf) {
        return vs_target_in_app(perf, target);
    }
    vs_target(perf, target_for(kind, b)?)
}

/// The target per in-app action, for an App campaign that has app events to count them with. It
/// wins over `target_cpi`: an install that never becomes an action is not a result.
fn in_app_target(kind: CampaignKind, b: &Budget, perf: &CampaignPerf) -> Option<f64> {
    if kind != CampaignKind::AppInstalls || perf.app.is_none() {
        return None;
    }
    b.target_cpa_in_app.map(cents_to_f64)
}

const CAP_RULE: &str = "budget_cap is the most daily budget that campaign may get (E27). Split the account budget with the caps in mind: a capped campaign takes less than the share its intent would suggest, and the rest goes to the others. A cap wins over any share suggested for an intent.";

/// The most daily budget a campaign may get, and why (E27). None: nothing limits it. The plan
/// sees the number in `get_business`, so it splits the budget with the caps in mind instead of
/// finding them one refused call at a time.
pub(super) fn budget_cap(live: &Live, b: &Budget, name: &str) -> Option<(&'static str, Cents)> {
    let perf = live.performance.campaign(name)?;
    let ran = live
        .baseline
        .campaigns
        .iter()
        .find(|o| normalize(&o.name) == normalize(name))?
        .daily_budget;
    if perf.stalled {
        return Some((
            "stalled: no ads, or no impression for a week. Fix it or cut its budget",
            ran,
        ));
    }
    if perf.untracked {
        // Under its budget, a cut down to what it spends changes nothing it gets.
        let spent = perf
            .avg_daily_cost
            .map(|a| Cents((a.ceil() * 100.0) as u64));
        return Some((
            "untracked: clicks and no conversion while other campaigns convert. Its result is not measured, and it gets no more than it spends",
            spent.map_or(ran, |s| s.min(ran)),
        ));
    }
    matches!(
        verdict_of(live, b, name),
        Some(VsTarget::Over | VsTarget::NoResult)
    )
    .then_some((
        "over its target: each conversion costs more than budget.target_cpa, target_cpi or target_cpa_in_app, or it spent twice that with none",
        ran,
    ))
}

fn target_rule(b: &Budget) -> Option<String> {
    if b.target_cpa.is_none() && b.target_cpi.is_none() && b.target_cpa_in_app.is_none() {
        return None;
    }
    Some(format!(
        "vs_target compares what each conversion cost with the target in `budget` (target_cpa for campaigns, target_cpi for App campaigns, and target_cpa_in_app for an App campaign with app events, where each in-app action is the result and it wins over target_cpi): within is at or under it, over is above it, no_result is {NO_RESULT_TARGETS} targets spent with no conversion. An over or no_result campaign gets no more budget than it ran with (E27), thin or not. Give the money freed to a within campaign that its budget limits. A campaign with no vs_target has no target for its kind or not enough numbers yet."
    ))
}

/// For the plan: every campaign as it ran, and campaign totals without keyword detail.
pub fn business_view(live: &Live, budget: &Budget) -> (Value, Value) {
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
                if let Some(verdict) = verdict_of(live, budget, &c.name) {
                    o.insert("vs_target".into(), json!(verdict));
                }
                if let Some((_, cap)) = budget_cap(live, budget, &c.name) {
                    o.insert("budget_cap".into(), json!(cents_to_f64(cap)));
                }
            }
            v
        })
        .collect();
    let mut rules = live_rules(p);
    if campaigns.iter().any(|c| c.get("budget_cap").is_some()) {
        rules = format!("{rules} {CAP_RULE}");
    }
    if let Some(rule) = target_rule(budget) {
        rules = format!("{rules} {rule}");
    }
    let mut perf = json!({
        "window": p.window, "thin_rule": thin_rule(), "live_rules": rules,
        "idle_budget": p.idle_budget, "missing_reports": p.missing_reports,
        "ignored_campaigns": p.ignored_campaigns, "campaigns": campaigns,
    });
    let blind = p.outside_blind();
    if !blind.is_empty() {
        perf["outside_campaigns"] = json!(blind
            .iter()
            .map(|o| json!({
                "name": o.name, "cost_share_pct": o.cost_share_pct, "clicks": o.metrics.clicks,
                "conversions": o.metrics.conversions, "paused_by_the_import": p.is_paused_outside(&o.name),
            }))
            .collect::<Vec<_>>());
    }
    if !p.drift.is_empty() {
        perf["drift"] = json!(p.drift);
    }
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
            let stray = ran_campaign(live, campaign)
                .map(|ran| crate::google::stray_groups(live, ran))
                .unwrap_or_default();
            let groups = c.ad_groups.iter().map(|g| {
                let mut s = summary(g);
                if stray.contains(&g.name.as_str()) {
                    s["not_in_run"] = json!(true);
                }
                s
            });
            v["ad_groups"] = groups.collect::<Vec<_>>().into();
            v["window"] = json!(live.performance.window);
            v["thin_rule"] = json!(thin_rule());
            v["live_rules"] = json!(live_rules(&live.performance));
            v
        });
    (ran, perf)
}

/// One ad group in a line: its totals and how many keywords carry each signal.
fn summary(g: &AdGroupPerf) -> Value {
    let count = |s: Signal| g.keywords.iter().filter(|k| k.signals.contains(&s)).count();
    json!({
        "name": g.name,
        "metrics": metrics(&g.totals()),
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
    if let Some(m) = &g.metrics {
        v["metrics"] = metrics(m);
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
        input::AppStore,
        perf::{
            AdGroupPerf, AppPerf, CampaignPerf, KeywordPerf, Metrics, Network, NetworkPerf,
            Performance, Signal, StoreEvents, TermPerf, TermState,
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
            app_store: None,
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
    fn the_rules_cover_idle_budget_of_thin_campaigns_and_bidding_changed_by_hand() {
        let thin = thin_rule();
        assert!(
            thin.contains("budget still follows budget_use_pct"),
            "{thin}"
        );
        let rules = live_rules(&Performance::default());
        for needle in [
            "thin or not",
            "At 90 or more",
            "up to 40 percent",
            "bid_strategy is how the campaign bids in Google Ads today",
            "changed outside mads (by hand or by auto-applied recommendations)",
            "not_in_run",
        ] {
            assert!(rules.contains(needle), "missing {needle} in {rules}");
        }
    }

    #[test]
    fn a_group_of_an_older_import_is_marked_in_the_brief() {
        let mut live = big();
        live.performance.campaigns[0].ad_groups[0].live_status =
            Some(crate::perf::LiveStatus::Enabled);
        live.performance.campaigns[0].ad_groups.push(AdGroupPerf {
            name: "Uvas antigas".into(),
            live_status: Some(crate::perf::LiveStatus::Enabled),
            ..Default::default()
        });
        let (_, perf) = brief_view(&live, "Catalogo");
        let groups = perf["ad_groups"].as_array().unwrap();
        assert!(groups[0].get("not_in_run").is_none(), "{}", groups[0]);
        assert_eq!(groups[7]["name"], "Uvas antigas");
        assert_eq!(groups[7]["not_in_run"], true);
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
    fn group_totals_from_the_ad_group_report_win_over_keyword_sums() {
        let mut live = big();
        let mut m = Metrics {
            impressions: 900,
            clicks: 30,
            cost: 36.0,
            conversions: 3.0,
            ..Default::default()
        };
        m.derive();
        live.performance.campaigns[0].ad_groups[0].metrics = Some(m);
        let (_, perf) = brief_view(&live, "Catalogo");
        assert_eq!(perf["ad_groups"][0]["metrics"]["clicks"], 30);
        assert_eq!(
            perf["ad_groups"][1]["metrics"]["clicks"], 2,
            "summed from keywords"
        );
        let v = ad_group_view(&live, "Catalogo", "Grupo de uvas numero 0").unwrap();
        assert_eq!(v["performance"]["metrics"]["conversions"], 3.0);
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

    /// An App campaign with cheap installs that rarely turn into an account: 300 spent, Google
    /// counts 2400 conversions, the app saw 2000 installs and 100 actions.
    fn app_live(with_events: bool) -> Live {
        let mut live = big();
        live.baseline.campaigns[0].kind = CampaignKind::AppInstalls;
        let c = &mut live.performance.campaigns[0];
        c.thin = false;
        c.metrics.cost = 300.0;
        c.metrics.conversions = 2400.0;
        if with_events {
            let seen = StoreEvents {
                installs: 2000.0,
                in_app_actions: 100.0,
            };
            c.app = Some(AppPerf::new(AppStore::GooglePlay, seen, 300.0, 2400.0));
        }
        live
    }

    fn targets(cpi: Option<u64>, in_app: Option<u64>) -> Budget {
        let mut b = crate::testutil::input().budget;
        b.target_cpi = cpi.map(Cents);
        b.target_cpa_in_app = in_app.map(Cents);
        b
    }

    fn verdict(live: &Live, b: &Budget) -> Option<VsTarget> {
        verdict_of(live, b, "Catalogo")
    }

    #[test]
    fn the_in_app_target_wins_over_the_install_target() {
        let live = app_live(true);
        // 0.125 per Google conversion is within a target of 0.20 per install.
        assert_eq!(
            verdict(&live, &targets(Some(20), None)),
            Some(VsTarget::Within)
        );
        // 3.00 per in-app action is over a target of 2.00, installs target or not.
        assert_eq!(
            verdict(&live, &targets(Some(20), Some(200))),
            Some(VsTarget::Over)
        );
        assert_eq!(
            verdict(&live, &targets(None, Some(200))),
            Some(VsTarget::Over)
        );
        // Once an in-app target judges, the install target is not what decides.
        assert_eq!(
            verdict(&live, &targets(Some(1), Some(300))),
            Some(VsTarget::Within)
        );
    }

    #[test]
    fn without_app_events_the_install_target_still_judges() {
        let live = app_live(false);
        assert_eq!(
            verdict(&live, &targets(Some(20), Some(200))),
            Some(VsTarget::Within)
        );
        assert_eq!(verdict(&live, &targets(None, Some(200))), None);
        assert_eq!(verdict(&live, &targets(None, None)), None);
    }

    #[test]
    fn the_in_app_target_does_not_judge_other_campaigns() {
        let mut live = app_live(true);
        live.baseline.campaigns[0].kind = CampaignKind::Search;
        let mut b = targets(None, Some(200));
        assert_eq!(verdict(&live, &b), None);
        b.target_cpa = Some(Cents(10_000));
        assert_eq!(verdict(&live, &b), Some(VsTarget::Within));
    }

    #[test]
    fn an_app_campaign_over_its_in_app_target_gets_no_more_budget() {
        let live = app_live(true);
        let over = targets(Some(20), Some(200));
        let (why, cap) = budget_cap(&live, &over, "Catalogo").unwrap();
        assert!(why.contains("target_cpa_in_app"), "{why}");
        assert_eq!(cap, Cents(7000), "no more than it ran with");
        let on_installs = targets(Some(20), None);
        assert!(budget_cap(&live, &on_installs, "Catalogo").is_none());
    }

    #[test]
    fn the_plan_reads_the_app_numbers_and_the_rule_about_cheap_installs() {
        let (_, perf) = business_view(&app_live(true), &targets(None, Some(200)));
        let c = &perf["campaigns"][0];
        assert_eq!(c["app"]["app_installs_seen"], 2000.0);
        assert_eq!(c["app"]["in_app_actions"], 100.0);
        assert_eq!(c["app"]["install_to_action_pct"], 5.0);
        assert_eq!(c["app"]["cost_per_in_app_action"], 3.0);
        assert_eq!(c["app"]["store_vs_app_install_ratio"], 1.2);
        assert_eq!(c["vs_target"], "over");
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(
            rules.contains("should not get more budget just because its cost per install is low"),
            "{rules}"
        );
        assert!(rules.contains("target_cpa_in_app"), "{rules}");
    }

    fn network(network: Network, cost: f64, ctr: f64) -> NetworkPerf {
        NetworkPerf {
            network,
            cost_share_pct: Some(cost),
            conversions_share_pct: None,
            ctr_pct: Some(ctr),
        }
    }

    #[test]
    fn display_clicks_that_look_accidental_reach_the_plan_with_a_rule() {
        let mut live = app_live(false);
        let c = &mut live.performance.campaigns[0];
        c.networks = vec![
            network(Network::Search, 40.0, 4.0),
            network(Network::Display, 60.0, 22.5),
        ];
        c.accidental_display_clicks = true;
        let (_, perf) = business_view(&live, &targets(Some(20), None));
        let v = &perf["campaigns"][0];
        assert_eq!(v["accidental_display_clicks"], true);
        assert_eq!(v["networks"][1]["network"], "display");
        assert_eq!(v["networks"][1]["ctr_pct"], 22.5);
        assert!(
            v["networks"][1].get("conversions_share_pct").is_none(),
            "no nulls: {v}"
        );
        let rules = perf["live_rules"].as_str().unwrap();
        for needle in [
            "treat its install count with caution",
            "do not raise its budget on cost per install alone",
            "15 percent",
            "not a Google rule",
        ] {
            assert!(rules.contains(needle), "missing {needle} in {rules}");
        }
        let (_, brief) = brief_view(&live, "Catalogo");
        assert!(
            brief["live_rules"]
                .as_str()
                .unwrap()
                .contains("accidental_display_clicks")
        );
    }

    #[test]
    fn the_plan_is_told_why_an_app_campaign_is_untracked_and_which_actions_are_off_goal() {
        let mut live = app_live(false);
        let c = &mut live.performance.campaigns[0];
        c.untracked = true;
        c.untracked_reason = Some(NO_ACTION_FOR_STORE.into());
        c.off_goal_actions = vec!["Purchase".into()];
        let (_, perf) = business_view(&live, &targets(None, None));
        let v = &perf["campaigns"][0];
        assert_eq!(v["untracked_reason"], NO_ACTION_FOR_STORE);
        assert_eq!(v["off_goal_actions"][0], "Purchase");
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(
            rules.contains("only an Android download action exists"),
            "{rules}"
        );
        assert!(rules.contains("off_goal_actions"), "{rules}");
    }

    #[test]
    fn reports_that_were_not_read_add_nothing_to_what_the_plan_sees() {
        let (_, perf) = business_view(&app_live(false), &targets(None, None));
        let v = &perf["campaigns"][0];
        for key in [
            "networks",
            "conversion_actions",
            "off_goal_actions",
            "untracked_reason",
            "accidental_display_clicks",
        ] {
            assert!(v.get(key).is_none(), "{key} in {v}");
        }
        let rules = perf["live_rules"].as_str().unwrap();
        for word in [
            "untracked_reason",
            "off_goal_actions",
            "accidental_display_clicks",
        ] {
            assert!(!rules.contains(word), "{word} in {rules}");
        }
    }

    #[test]
    fn a_run_without_app_events_shows_nothing_new_to_the_plan() {
        let (_, perf) = business_view(&app_live(false), &targets(Some(20), None));
        assert!(perf["campaigns"][0].get("app").is_none());
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(!rules.contains("cost per install is low"), "{rules}");
        assert!(!rules.contains("app_installs_seen"), "{rules}");
        assert_eq!(perf["campaigns"][0]["vs_target"], "within");
    }

    #[test]
    fn the_plan_is_told_about_money_outside_the_run_and_about_drift() {
        use crate::perf::{CampaignDrift, Drift, DriftKind, LiveStatus, OutsideCampaign};
        let mut live = app_live(false);
        let (_, perf) = business_view(&live, &targets(None, None));
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(perf.get("outside_campaigns").is_none() && perf.get("drift").is_none());
        assert!(!rules.contains("outside_campaigns") && !rules.contains("drift lists"));

        live.performance.outside = vec![OutsideCampaign {
            name: "Old".into(),
            live_status: Some(LiveStatus::Enabled),
            cost_share_pct: Some(40.0),
            spends_blind: true,
            ..Default::default()
        }];
        live.performance.drift = Drift {
            campaigns: vec![CampaignDrift {
                name: "Catalogo".into(),
                change: DriftKind::Budget {
                    was: 100.0,
                    now: 150.0,
                },
            }],
            account_budget_pct: Some(150.0),
        };
        let (_, perf) = business_view(&live, &targets(None, None));
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(rules.contains("That money is outside the plan"), "{rules}");
        assert!(rules.contains("This run leaves them running"), "{rules}");
        assert!(
            rules.contains("Budgets were changed outside mads (by hand or by auto-applied recommendations)"),
            "{rules}"
        );
        assert!(rules.contains("live baseline"), "{rules}");
        assert_eq!(perf["outside_campaigns"][0]["name"], "Old");
        assert_eq!(perf["outside_campaigns"][0]["cost_share_pct"], 40.0);
        assert_eq!(perf["outside_campaigns"][0]["paused_by_the_import"], false);
        assert_eq!(perf["drift"]["campaigns"][0]["change"]["kind"], "budget");
        assert_eq!(perf["drift"]["account_budget_pct"], 150.0);

        live.performance.pause_outside_run = vec!["Old".into()];
        let (_, perf) = business_view(&live, &targets(None, None));
        let rules = perf["live_rules"].as_str().unwrap();
        assert!(rules.contains("The import pauses them"), "{rules}");
        assert_eq!(perf["outside_campaigns"][0]["paused_by_the_import"], true);
    }
}
