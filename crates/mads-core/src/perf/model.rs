//! What `mads optimize` learned from the reports, as the agents read it.

use serde::{Deserialize, Serialize};

use super::table::{ReportKind, Window};
use crate::google::{Account, MatchType};

/// Below this many days or clicks a campaign's numbers are noise: agents fix structure only.
pub const THIN_DAYS: u32 = 14;
pub const THIN_CLICKS: u64 = 100;
/// Search terms kept per ad group, by cost. The rest are summed, not listed.
pub const TERMS_PER_GROUP: usize = 30;

/// The account as it ran, and what the reports say about it. Stored in `workspace.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Live {
    /// The account of the run being optimized. Anything in it that the new account drops is
    /// paused at export.
    pub baseline: Account,
    pub performance: Performance,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Performance {
    pub window: Option<Window>,
    pub reports: Vec<ReportFile>,
    /// Files that are not a report mads understands, with the reason.
    pub unknown_files: Vec<String>,
    /// Campaigns in the reports that are not in the run, such as older campaigns of the account.
    pub ignored_campaigns: Vec<String>,
    pub campaigns: Vec<CampaignPerf>,
    /// The reports cover different dates: totals of one and rows of another do not add up.
    #[serde(default)]
    pub mixed_windows: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportFile {
    pub file: String,
    pub kind: ReportKind,
    pub rows: usize,
    #[serde(default)]
    pub window: Option<Window>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub impressions: u64,
    pub clicks: u64,
    /// Account currency units.
    pub cost: f64,
    pub conversions: f64,
    pub conversion_value: f64,
    pub ctr_pct: Option<f64>,
    pub avg_cpc: Option<f64>,
    pub cost_per_conversion: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveStatus {
    Enabled,
    Paused,
    Removed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CampaignPerf {
    pub name: String,
    pub live_status: Option<LiveStatus>,
    /// Google's status reasons, such as "limited by policy" or "no ads".
    pub status_reasons: String,
    pub budget: Option<f64>,
    pub metrics: Metrics,
    pub lost_to_budget_pct: Option<f64>,
    pub lost_to_rank_pct: Option<f64>,
    /// Too little data to judge performance: fix structure only.
    pub thin: bool,
    /// Cost Google does not show by search term: the campaign cost minus the listed terms.
    pub hidden_terms_cost: Option<f64>,
    pub ad_groups: Vec<AdGroupPerf>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdGroupPerf {
    pub name: String,
    pub keywords: Vec<KeywordPerf>,
    /// The most expensive terms, at most `TERMS_PER_GROUP`.
    pub search_terms: Vec<TermPerf>,
    pub search_terms_total: usize,
    /// Cost of every term of the group with zero conversions.
    pub cost_without_conversions: f64,
    pub assets: Vec<AssetPerf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// The CPC is under what the auction asks to show on the first page.
    BelowFirstPage,
    RarelyShown,
    LowQuality,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordPerf {
    pub text: String,
    pub match_type: Option<MatchType>,
    pub status_reasons: String,
    pub signals: Vec<Signal>,
    pub max_cpc: Option<f64>,
    pub quality_score: Option<f64>,
    pub first_page_bid: Option<f64>,
    pub top_of_page_bid: Option<f64>,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TermState {
    /// Already a keyword of the ad group.
    Keyword,
    /// Blocked by a negative of the run.
    Negative,
    /// Excluded in Google Ads by hand.
    Excluded,
    New,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TermPerf {
    pub text: String,
    pub match_type: String,
    pub state: TermState,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetPerf {
    pub text: String,
    pub kind: String,
    /// Google's performance label, such as Best, Good or Low.
    pub label: String,
    pub impressions: u64,
}

impl Metrics {
    pub fn add(&mut self, o: &Metrics) {
        self.impressions += o.impressions;
        self.clicks += o.clicks;
        self.cost += o.cost;
        self.conversions += o.conversions;
        self.conversion_value += o.conversion_value;
        self.derive();
    }

    /// Fills the ratios from the totals. None when the divisor is zero.
    pub fn derive(&mut self) {
        let ratio = |a: f64, b: f64| (b > 0.0).then(|| (a / b * 100.0).round() / 100.0);
        self.ctr_pct = ratio(self.clicks as f64 * 100.0, self.impressions as f64);
        self.avg_cpc = ratio(self.cost, self.clicks as f64);
        self.cost_per_conversion = ratio(self.cost, self.conversions);
    }
}

impl Performance {
    pub fn campaign(&self, name: &str) -> Option<&CampaignPerf> {
        let key = crate::google::normalize(name);
        self.campaigns
            .iter()
            .find(|c| crate::google::normalize(&c.name) == key)
    }
}
