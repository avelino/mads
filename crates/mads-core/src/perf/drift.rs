//! What changed in Google Ads since the run being optimized was exported. The reports cannot say
//! who changed it: by hand and auto-applied recommendations look the same.

use serde::{Deserialize, Serialize};

use super::model::{CampaignPerf, Live, LiveStatus, Performance};
use crate::{
    google::{Account, Campaign, Cents, normalize},
    input::ExportStatus,
};

/// A budget that moved by more than this share of what the last run exported is drift. Also the
/// share the live account budgets may stand above the budget of the run. A line mads draws.
pub const DRIFT_BUDGET_PCT: f64 = 20.0;

/// Who may have made a change the reports show. They cannot tell.
pub const DRIFT_SOURCE: &str = "changed outside mads (by hand or by auto-applied recommendations)";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriftKind {
    /// Daily budget, in account currency units, as the last run exported it and as it is now.
    Budget { was: f64, now: f64 },
    /// Bid strategy as the last run exported it and as the report writes it.
    BidStrategy { was: String, now: String },
    /// Exported enabled, paused now.
    Paused,
    /// Removed in Google Ads.
    Removed,
    /// Dropped by the last run, which exported it paused, and enabled now.
    EnabledAgain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignDrift {
    pub name: String,
    pub change: DriftKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Drift {
    pub campaigns: Vec<CampaignDrift>,
    /// The daily budgets of the enabled campaigns of the account add up to this percent of the
    /// daily budget of the run. Set only when it is above `DRIFT_BUDGET_PCT` over 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_budget_pct: Option<f64>,
}

/// What the run being optimized exported.
pub struct Base<'a> {
    pub account: &'a Account,
    /// The reports the base run was optimized with, when it was an optimize run.
    pub live: Option<&'a Live>,
    pub export_status: ExportStatus,
}

impl Drift {
    pub fn is_empty(&self) -> bool {
        self.campaigns.is_empty() && self.account_budget_pct.is_none()
    }

    /// Some budget of the live account is not the one the last run exported.
    pub fn budgets_moved(&self) -> bool {
        self.account_budget_pct.is_some()
            || self
                .campaigns
                .iter()
                .any(|c| matches!(c.change, DriftKind::Budget { .. }))
    }

    /// One line per change, for the terminal and `report.md`. Shares only: the lines go to
    /// places the advertiser's reports do not.
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self.campaigns.iter().map(CampaignDrift::line).collect();
        if let Some(pct) = self.account_budget_pct {
            out.push(format!(
                "the daily budgets of the enabled campaigns add up to {pct}% of the daily budget of this run"
            ));
        }
        out
    }
}

impl CampaignDrift {
    fn line(&self) -> String {
        let name = &self.name;
        match &self.change {
            DriftKind::Budget { was, now } => {
                let pct = ((now - was).abs() * 100.0 / was).round();
                let way = if now > was { "higher" } else { "lower" };
                format!("{name}: daily budget is {pct}% {way} than the last run exported")
            }
            DriftKind::BidStrategy { was, now } => {
                format!("{name}: bids with `{now}`, the last run exported {was}")
            }
            DriftKind::Paused => format!("{name}: paused, the last run exported it enabled"),
            DriftKind::Removed => format!("{name}: removed in Google Ads"),
            DriftKind::EnabledAgain => {
                format!("{name}: enabled, the last run dropped it and exported it paused")
            }
        }
    }
}

/// The live account against the base run. `daily` is the daily budget of the new run.
pub fn drift_since(base: &Base, p: &Performance, daily: Cents) -> Drift {
    let mut campaigns = Vec::new();
    for c in &base.account.campaigns {
        let Some(perf) = p.campaign(&c.name) else {
            continue;
        };
        let changes = [
            budget_drift(c, perf),
            bid_drift(c, perf),
            status_drift(base, c, perf),
        ];
        campaigns.extend(changes.into_iter().flatten().map(|change| CampaignDrift {
            name: c.name.clone(),
            change,
        }));
    }
    campaigns.extend(enabled_again(base, p));
    Drift {
        campaigns,
        account_budget_pct: account_over(p, daily),
    }
}

fn budget_drift(c: &Campaign, perf: &CampaignPerf) -> Option<DriftKind> {
    if perf.live_status == Some(LiveStatus::Removed) {
        return None;
    }
    let was = c.daily_budget.0 as f64 / 100.0;
    let now = perf.budget?;
    let moved = was > 0.0 && (now - was).abs() * 100.0 / was > DRIFT_BUDGET_PCT;
    moved.then_some(DriftKind::Budget { was, now })
}

fn bid_drift(c: &Campaign, perf: &CampaignPerf) -> Option<DriftKind> {
    let label = c.bid_strategy.label();
    perf.bids_unlike(label).then(|| DriftKind::BidStrategy {
        was: label.to_string(),
        now: perf.bid_strategy.clone(),
    })
}

/// A campaign the base run exported paused is expected to be enabled after the import, so only
/// what it exported enabled counts as paused by somebody.
fn status_drift(base: &Base, c: &Campaign, perf: &CampaignPerf) -> Option<DriftKind> {
    match perf.live_status? {
        LiveStatus::Removed => Some(DriftKind::Removed),
        LiveStatus::Paused if exported_enabled(base, c) => Some(DriftKind::Paused),
        LiveStatus::Paused | LiveStatus::Enabled => None,
    }
}

/// A campaign that was live when the base run was optimized came out enabled, whatever it was.
/// A new one follows `export.status`.
fn exported_enabled(base: &Base, c: &Campaign) -> bool {
    let was_live = base
        .live
        .and_then(|l| l.performance.campaign(&c.name))
        .is_some_and(|p| {
            matches!(
                p.live_status,
                Some(LiveStatus::Enabled | LiveStatus::Paused)
            )
        });
    was_live || base.export_status == ExportStatus::Enabled
}

/// Campaigns the base run dropped, so it exported them paused, that are enabled again.
fn enabled_again(base: &Base, p: &Performance) -> Vec<CampaignDrift> {
    let Some(live) = base.live else {
        return Vec::new();
    };
    let kept = |name: &str| {
        base.account
            .campaigns
            .iter()
            .any(|c| normalize(&c.name) == normalize(name))
    };
    let enabled_outside = |name: &str| {
        let key = normalize(name);
        p.outside
            .iter()
            .any(|x| normalize(&x.name) == key && x.live_status == Some(LiveStatus::Enabled))
    };
    live.baseline
        .campaigns
        .iter()
        .filter(|o| !kept(&o.name) && enabled_outside(&o.name))
        .map(|o| CampaignDrift {
            name: o.name.clone(),
            change: DriftKind::EnabledAgain,
        })
        .collect()
}

/// The live daily budgets of every enabled campaign, in the run or not, as a percent of `daily`.
/// None when they stay within `DRIFT_BUDGET_PCT` over it.
fn account_over(p: &Performance, daily: Cents) -> Option<f64> {
    if daily.0 == 0 {
        return None;
    }
    let enabled = |status: Option<LiveStatus>| status == Some(LiveStatus::Enabled);
    let in_run = p
        .campaigns
        .iter()
        .filter(|c| enabled(c.live_status))
        .filter_map(|c| c.budget);
    let outside = p
        .outside
        .iter()
        .filter(|c| enabled(c.live_status))
        .filter_map(|c| c.budget);
    let total: f64 = in_run.chain(outside).sum();
    let pct = total * 100.0 / (daily.0 as f64 / 100.0);
    (pct > 100.0 + DRIFT_BUDGET_PCT).then(|| pct.round())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{BidStrategy, Intent},
        perf::OutsideCampaign,
    };

    const DAILY: Cents = Cents(1_000_000);

    fn campaign(name: &str, budget: u64) -> Campaign {
        Campaign {
            app_store: None,
            kind: Default::default(),
            asset_groups: Vec::new(),
            name: name.into(),
            slug: crate::input::slugify(name),
            intent: Intent::Generic,
            daily_budget: Cents(budget),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: Vec::new(),
            ad_groups: Vec::new(),
            negatives: Vec::new(),
            assets: None,
        }
    }

    fn account(campaigns: Vec<Campaign>) -> Account {
        Account {
            brand_kit: None,
            campaigns,
        }
    }

    fn perf(name: &str, status: LiveStatus, budget: f64) -> CampaignPerf {
        CampaignPerf {
            name: name.into(),
            live_status: Some(status),
            budget: Some(budget),
            ..Default::default()
        }
    }

    fn running(campaigns: Vec<CampaignPerf>) -> Performance {
        Performance {
            campaigns,
            ..Default::default()
        }
    }

    fn outside(name: &str, status: LiveStatus, budget: Option<f64>) -> OutsideCampaign {
        OutsideCampaign {
            name: name.into(),
            live_status: Some(status),
            budget,
            ..Default::default()
        }
    }

    /// The drift of a base run that was not optimized: nothing was live before it.
    fn drift(base: &Account, status: ExportStatus, p: &Performance) -> Vec<DriftKind> {
        let base = Base {
            account: base,
            live: None,
            export_status: status,
        };
        drift_since(&base, p, DAILY)
            .campaigns
            .into_iter()
            .map(|c| c.change)
            .collect()
    }

    #[test]
    fn a_budget_is_drift_only_above_the_threshold() {
        let base = account(vec![campaign("Search", 10_000)]);
        let at = |budget| {
            let p = running(vec![perf("Search", LiveStatus::Enabled, budget)]);
            drift(&base, ExportStatus::Enabled, &p)
        };
        assert!(at(120.0).is_empty(), "20 percent more is not above 20");
        assert!(at(80.0).is_empty(), "20 percent less is not above 20");
        assert!(at(100.0).is_empty());
        let moved = |now| [DriftKind::Budget { was: 100.0, now }];
        assert_eq!(at(120.5), moved(120.5));
        assert_eq!(at(79.0), moved(79.0));
    }

    #[test]
    fn a_report_without_budgets_has_no_budget_drift() {
        let base = account(vec![campaign("Search", 10_000)]);
        let mut c = perf("Search", LiveStatus::Enabled, 0.0);
        c.budget = None;
        assert!(drift(&base, ExportStatus::Enabled, &running(vec![c])).is_empty());
    }

    #[test]
    fn a_bid_strategy_that_is_not_the_exported_one_is_drift() {
        let base = account(vec![campaign("Search", 10_000), campaign("Brand", 10_000)]);
        let mut search = perf("Search", LiveStatus::Enabled, 100.0);
        search.bid_strategy = "Maximize conversions".into();
        let mut brand = perf("Brand", LiveStatus::Enabled, 100.0);
        brand.bid_strategy = "Manual CPC".into();
        let found = drift(&base, ExportStatus::Enabled, &running(vec![search, brand]));
        assert_eq!(
            found,
            [DriftKind::BidStrategy {
                was: "Manual CPC".into(),
                now: "Maximize conversions".into()
            }]
        );
    }

    #[test]
    fn paused_and_removed_campaigns_are_drift_when_the_base_run_exported_them_enabled() {
        let base = account(vec![campaign("Search", 10_000), campaign("Brand", 10_000)]);
        let p = running(vec![
            perf("Search", LiveStatus::Paused, 100.0),
            perf("Brand", LiveStatus::Removed, 100.0),
        ]);
        assert_eq!(
            drift(&base, ExportStatus::Enabled, &p),
            [DriftKind::Paused, DriftKind::Removed]
        );
    }

    #[test]
    fn a_campaign_the_base_run_exported_paused_is_not_drift_while_it_stays_paused() {
        let base = account(vec![campaign("Search", 10_000)]);
        let p = running(vec![perf("Search", LiveStatus::Paused, 100.0)]);
        assert!(drift(&base, ExportStatus::Paused, &p).is_empty());
        let on = running(vec![perf("Search", LiveStatus::Enabled, 100.0)]);
        assert!(drift(&base, ExportStatus::Paused, &on).is_empty());
    }

    #[test]
    fn a_campaign_that_was_live_for_the_base_run_came_out_enabled() {
        let base_account = account(vec![campaign("Search", 10_000)]);
        let before = Live {
            baseline: base_account.clone(),
            performance: running(vec![perf("Search", LiveStatus::Paused, 100.0)]),
        };
        let base = Base {
            account: &base_account,
            live: Some(&before),
            export_status: ExportStatus::Paused,
        };
        let p = running(vec![perf("Search", LiveStatus::Paused, 100.0)]);
        let found = drift_since(&base, &p, DAILY);
        assert_eq!(found.campaigns[0].change, DriftKind::Paused);
    }

    #[test]
    fn a_campaign_the_base_run_dropped_that_is_enabled_again_is_drift() {
        let base_account = account(vec![campaign("Search", 10_000)]);
        let before = Live {
            baseline: account(vec![campaign("Search", 10_000), campaign("Old", 5_000)]),
            performance: Performance::default(),
        };
        let base = Base {
            account: &base_account,
            live: Some(&before),
            export_status: ExportStatus::Paused,
        };
        let mut p = running(vec![perf("Search", LiveStatus::Enabled, 100.0)]);
        p.outside = vec![outside("Old", LiveStatus::Paused, None)];
        assert!(drift_since(&base, &p, DAILY).is_empty());
        p.outside = vec![outside("Old", LiveStatus::Enabled, None)];
        let found = drift_since(&base, &p, DAILY);
        assert_eq!(found.campaigns[0].name, "Old");
        assert_eq!(found.campaigns[0].change, DriftKind::EnabledAgain);
    }

    #[test]
    fn the_live_total_is_compared_with_the_daily_budget_at_the_threshold() {
        let base_account = account(vec![]);
        let base = Base {
            account: &base_account,
            live: None,
            export_status: ExportStatus::Enabled,
        };
        let daily = Cents(10_000);
        let mut p = running(vec![perf("Search", LiveStatus::Enabled, 100.0)]);
        p.outside = vec![outside("Old", LiveStatus::Enabled, Some(20.0))];
        // 120 live against 100: at the threshold, not above it.
        assert_eq!(drift_since(&base, &p, daily).account_budget_pct, None);
        p.outside[0].budget = Some(21.0);
        let over = drift_since(&base, &p, daily);
        assert_eq!(over.account_budget_pct, Some(121.0));
        assert!(over.budgets_moved());
        // A paused campaign adds nothing.
        p.outside[0].live_status = Some(LiveStatus::Paused);
        assert!(drift_since(&base, &p, daily).is_empty());
    }

    #[test]
    fn the_lines_use_shares_and_name_what_changed() {
        let budget = |name: &str, now| CampaignDrift {
            name: name.into(),
            change: DriftKind::Budget { was: 100.0, now },
        };
        let d = Drift {
            campaigns: vec![budget("Search", 135.0), budget("Brand", 50.0)],
            account_budget_pct: Some(150.0),
        };
        let lines = d.lines();
        assert_eq!(
            lines[0],
            "Search: daily budget is 35% higher than the last run exported"
        );
        assert_eq!(
            lines[1],
            "Brand: daily budget is 50% lower than the last run exported"
        );
        assert!(lines[2].contains("add up to 150% of the daily budget of this run"));
    }
}
