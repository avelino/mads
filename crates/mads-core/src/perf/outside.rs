//! Campaigns of the reports that the run does not manage, and the ones among them that spend
//! without converting.

use super::model::{CampaignPerf, LiveStatus, Metrics, OutsideCampaign, Performance, THIN_CLICKS};

/// An enabled campaign outside the run that spent more than this share of the account cost
/// with no conversion is flagged, even under `THIN_CLICKS` clicks. A line mads draws, not a
/// Google rule.
pub const OUTSIDE_COST_SHARE_PCT: f64 = 10.0;

/// What the digest read of one campaign outside the run.
#[derive(Default)]
pub(super) struct OutsideRows {
    pub perf: CampaignPerf,
    /// A campaigns report gave the totals: day rows no longer add to them.
    pub has_totals: bool,
    /// Latest day read: its row gives the status and the budget.
    pub latest_day: String,
}

/// The campaigns outside the run with their share of the account cost. `in_run` are the
/// campaigns of the run: together they are the account.
pub(super) fn summarize(rows: Vec<CampaignPerf>, in_run: &[CampaignPerf]) -> Vec<OutsideCampaign> {
    let mut total: f64 = in_run.iter().map(|c| c.metrics.cost).sum();
    total += rows.iter().map(|c| c.metrics.cost).sum::<f64>();
    let converts = in_run
        .iter()
        .chain(&rows)
        .any(|c| c.metrics.conversions > 0.0);
    rows.into_iter()
        .map(|c| {
            let share = (total > 0.0).then(|| c.metrics.cost * 100.0 / total);
            let spends_blind = converts && is_blind(&c, share);
            OutsideCampaign {
                name: c.name,
                live_status: c.live_status,
                budget: c.budget,
                metrics: c.metrics,
                cost_share_pct: share.map(|s| (s * 100.0).round() / 100.0),
                spends_blind,
            }
        })
        .collect()
}

/// Enabled, no conversion, and either clicks enough to judge or a cost share above the line.
/// A removed or paused campaign costs nothing from now on.
fn is_blind(c: &CampaignPerf, share: Option<f64>) -> bool {
    let m: &Metrics = &c.metrics;
    c.live_status == Some(LiveStatus::Enabled)
        && m.conversions == 0.0
        && (m.clicks >= THIN_CLICKS || share.is_some_and(|s| s > OUTSIDE_COST_SHARE_PCT))
}

impl Performance {
    /// Campaigns outside the run that spend without converting.
    pub fn outside_blind(&self) -> Vec<&OutsideCampaign> {
        self.outside.iter().filter(|o| o.spends_blind).collect()
    }

    /// Campaigns `--pause-outside-run` writes a `Paused` row for.
    pub fn is_paused_outside(&self, name: &str) -> bool {
        let key = crate::google::normalize(name);
        self.pause_outside_run
            .iter()
            .any(|n| crate::google::normalize(n) == key)
    }

    /// The campaigns outside the run that spend without converting, and what happens to them.
    pub(super) fn outside_lines(&self) -> Vec<String> {
        let blind = self.outside_blind();
        if blind.is_empty() {
            return Vec::new();
        }
        let listed: Vec<String> = blind
            .iter()
            .map(|o| {
                let share = o
                    .cost_share_pct
                    .map_or_else(String::new, |s| format!("{s}% of the account cost, "));
                format!("{} ({share}{} clicks)", o.name, o.metrics.clicks)
            })
            .collect();
        let mut out = vec![format!(
            "enabled outside this run and spending without conversions while the account converts, money the plan does not manage: {}",
            listed.join(", ")
        )];
        out.push(if self.pause_outside_run.is_empty() {
            "left running: pass --pause-outside-run to pause them in the import".to_string()
        } else {
            format!(
                "paused by the import (--pause-outside-run): {}",
                self.pause_outside_run.join(", ")
            )
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign(name: &str, status: LiveStatus, cost: f64, clicks: u64, conv: f64) -> CampaignPerf {
        CampaignPerf {
            name: name.into(),
            live_status: Some(status),
            metrics: Metrics {
                cost,
                clicks,
                conversions: conv,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn converting(cost: f64) -> CampaignPerf {
        campaign("In run", LiveStatus::Enabled, cost, 500, 20.0)
    }

    fn flagged(rows: Vec<CampaignPerf>, in_run: &[CampaignPerf]) -> Vec<bool> {
        summarize(rows, in_run)
            .iter()
            .map(|o| o.spends_blind)
            .collect()
    }

    #[test]
    fn the_click_bar_is_the_one_of_an_untracked_campaign() {
        let run = [converting(1000.0)];
        let at = |clicks| campaign("Old", LiveStatus::Enabled, 1.0, clicks, 0.0);
        assert_eq!(flagged(vec![at(THIN_CLICKS)], &run), [true]);
        assert_eq!(flagged(vec![at(THIN_CLICKS - 1)], &run), [false]);
    }

    #[test]
    fn the_cost_share_bar_is_strictly_above_the_line() {
        let run = [converting(900.0)];
        // 100 of 1000 is exactly 10 percent: not above it.
        let at = |cost| campaign("Old", LiveStatus::Enabled, cost, 5, 0.0);
        assert_eq!(flagged(vec![at(100.0)], &run), [false]);
        assert_eq!(flagged(vec![at(101.0)], &run), [true]);
        let found = summarize(vec![at(100.0)], &run);
        assert_eq!(found[0].cost_share_pct, Some(10.0));
    }

    #[test]
    fn a_converting_campaign_is_never_flagged() {
        let run = [converting(10.0)];
        let big = campaign("Old", LiveStatus::Enabled, 990.0, 900, 0.5);
        assert_eq!(flagged(vec![big], &run), [false]);
    }

    #[test]
    fn only_enabled_campaigns_are_flagged() {
        let run = [converting(10.0)];
        for status in [LiveStatus::Paused, LiveStatus::Removed] {
            let c = campaign("Old", status, 990.0, 900, 0.0);
            assert_eq!(flagged(vec![c], &run), [false], "{status:?}");
        }
        let mut unknown = campaign("Old", LiveStatus::Enabled, 990.0, 900, 0.0);
        unknown.live_status = None;
        assert_eq!(flagged(vec![unknown], &run), [false]);
    }

    #[test]
    fn nothing_is_flagged_when_no_campaign_of_the_account_converts() {
        let run = [campaign("In run", LiveStatus::Enabled, 10.0, 500, 0.0)];
        let old = campaign("Old", LiveStatus::Enabled, 990.0, 900, 0.0);
        assert_eq!(flagged(vec![old], &run), [false]);
    }

    #[test]
    fn a_converting_campaign_outside_the_run_counts_as_the_account_converting() {
        let run = [campaign("In run", LiveStatus::Enabled, 10.0, 500, 0.0)];
        let rows = vec![
            campaign("Good", LiveStatus::Enabled, 10.0, 50, 4.0),
            campaign("Old", LiveStatus::Enabled, 10.0, 150, 0.0),
        ];
        assert_eq!(flagged(rows, &run), [false, true]);
    }

    #[test]
    fn the_findings_say_what_happens_to_them() {
        let run = [converting(10.0)];
        let outside = summarize(
            vec![campaign("Old", LiveStatus::Enabled, 90.0, 150, 0.0)],
            &run,
        );
        let mut p = Performance {
            outside,
            ..Default::default()
        };
        let lines = p.outside_lines();
        assert!(
            lines[0].contains("Old (90% of the account cost, 150 clicks)"),
            "{lines:?}"
        );
        assert!(
            lines[1].contains("--pause-outside-run to pause them"),
            "{lines:?}"
        );
        p.pause_outside_run = vec!["Old".into()];
        let lines = p.outside_lines();
        assert!(
            lines[1].contains("paused by the import (--pause-outside-run): Old"),
            "{lines:?}"
        );
        assert!(p.is_paused_outside("old"));
        p.outside.clear();
        assert!(p.outside_lines().is_empty());
    }
}
