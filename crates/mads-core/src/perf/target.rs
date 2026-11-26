//! A campaign's result against what the advertiser says one conversion is worth paying.

use serde::Serialize;

use super::CampaignPerf;

/// Spend, in targets, that a campaign may reach without one conversion before it counts as a
/// loss. mads draws this line, Google has none: one target of spend with no conversion is not
/// proof yet, two is.
pub const NO_RESULT_TARGETS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VsTarget {
    /// Each conversion cost the target or less.
    Within,
    /// Each conversion cost more than the target.
    Over,
    /// It spent `NO_RESULT_TARGETS` targets or more and converted nothing.
    NoResult,
}

/// None when the numbers do not say yet: no conversion and less than `NO_RESULT_TARGETS` spent.
/// `target` is in the currency of the report.
pub fn vs_target(p: &CampaignPerf, target: f64) -> Option<VsTarget> {
    judge(p.metrics.cost, p.metrics.conversions, target)
}

/// The same verdict for an App campaign, by what each in-app action cost: the cost of the
/// campaigns of its store over the actions the app counted. None without app events.
pub fn vs_target_in_app(p: &CampaignPerf, target: f64) -> Option<VsTarget> {
    let app = p.app.as_ref()?;
    judge(app.store_cost, app.in_app_actions, target)
}

fn judge(cost: f64, results: f64, target: f64) -> Option<VsTarget> {
    if results > 0.0 {
        return Some(if cost / results <= target {
            VsTarget::Within
        } else {
            VsTarget::Over
        });
    }
    (cost >= target * NO_RESULT_TARGETS).then_some(VsTarget::NoResult)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf::{AppPerf, Metrics};

    fn perf(cost: f64, conversions: f64) -> CampaignPerf {
        CampaignPerf {
            metrics: Metrics {
                cost,
                conversions,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn a_conversion_is_judged_by_what_it_cost() {
        assert_eq!(vs_target(&perf(80.0, 2.0), 40.0), Some(VsTarget::Within));
        assert_eq!(vs_target(&perf(100.0, 2.0), 40.0), Some(VsTarget::Over));
    }

    fn app_perf(cost: f64, actions: f64) -> CampaignPerf {
        CampaignPerf {
            app: Some(AppPerf {
                store: crate::input::AppStore::GooglePlay,
                app_installs_seen: 1000.0,
                in_app_actions: actions,
                install_to_action_pct: None,
                cost_per_in_app_action: None,
                store_vs_app_install_ratio: None,
                store_cost: cost,
            }),
            // Cheap installs: Google's conversions would say within any install target.
            ..perf(cost, 1000.0)
        }
    }

    #[test]
    fn an_in_app_action_is_judged_by_what_it_cost() {
        assert_eq!(
            vs_target_in_app(&app_perf(80.0, 2.0), 40.0),
            Some(VsTarget::Within)
        );
        assert_eq!(
            vs_target_in_app(&app_perf(100.0, 2.0), 40.0),
            Some(VsTarget::Over)
        );
        assert_eq!(vs_target_in_app(&app_perf(79.0, 0.0), 40.0), None);
        assert_eq!(
            vs_target_in_app(&app_perf(80.0, 0.0), 40.0),
            Some(VsTarget::NoResult)
        );
        assert_eq!(
            vs_target(&app_perf(100.0, 2.0), 1.0),
            Some(VsTarget::Within),
            "by installs the same campaign is within a target of 1 per install"
        );
    }

    #[test]
    fn without_app_events_there_is_no_in_app_verdict() {
        assert_eq!(vs_target_in_app(&perf(80.0, 2.0), 40.0), None);
    }

    #[test]
    fn spend_without_conversion_is_a_loss_only_from_two_targets() {
        assert_eq!(vs_target(&perf(79.0, 0.0), 40.0), None);
        assert_eq!(vs_target(&perf(80.0, 0.0), 40.0), Some(VsTarget::NoResult));
        assert_eq!(vs_target(&perf(0.0, 0.0), 40.0), None);
    }
}
