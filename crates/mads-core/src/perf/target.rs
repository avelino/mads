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
    let m = &p.metrics;
    if m.conversions > 0.0 {
        return Some(if m.cost / m.conversions <= target {
            VsTarget::Within
        } else {
            VsTarget::Over
        });
    }
    (m.cost >= target * NO_RESULT_TARGETS).then_some(VsTarget::NoResult)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf::Metrics;

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

    #[test]
    fn spend_without_conversion_is_a_loss_only_from_two_targets() {
        assert_eq!(vs_target(&perf(79.0, 0.0), 40.0), None);
        assert_eq!(vs_target(&perf(80.0, 0.0), 40.0), Some(VsTarget::NoResult));
        assert_eq!(vs_target(&perf(0.0, 0.0), 40.0), None);
    }
}
