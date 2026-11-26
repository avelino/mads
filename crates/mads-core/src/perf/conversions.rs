//! The conversion actions each campaign counted, from the Campaigns report segmented by
//! conversion action. They say which App campaign has nothing counting for its store, and which
//! Search or Demand Gen campaign counts something other than what the account is after.

use serde::{Deserialize, Serialize};

use super::{
    model::{CampaignPerf, Performance, THIN_CLICKS},
    table::{CONVERSION_ACTION, Row},
};
use crate::{
    google::{Account, CampaignKind},
    input::{AppStore, fold},
};

/// Why an App campaign is untracked when none of the conversion actions it counted is for its
/// store, such as an iOS campaign next to an Android download action.
pub const NO_ACTION_FOR_STORE: &str = "no conversion action for this store";
/// Conversion actions kept per campaign, the ones with most conversions. The rest are dropped.
pub const ACTIONS_PER_CAMPAIGN: usize = 5;

const CATEGORY: &[&str] = &[
    "categoria de conversao",
    "categoria da acao de conversao",
    "conversion category",
    "conversion action category",
];
const SOURCE: &[&str] = &["origem da conversao", "conversion source"];
const ALL_CONVERSIONS: &[&str] = &[
    "todas as conv",
    "todas as conversoes",
    "all conv",
    "all conversions",
];
/// Words that make an action an install: its category or its name. Folded, so `instalação` fits.
const INSTALL_WORDS: &[&str] = &["download", "instal", "first open", "primeira abertura"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConversionActionPerf {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// What the action counted as a conversion, the number bidding learns from.
    pub conversions: f64,
    /// Everything the action counted, including what is not a conversion for bidding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_conversions: Option<f64>,
}

/// Adds one row of the report to its campaign. Rows of the same action add up. A row without
/// an action (Google writes `--` for a campaign that counted none) is left out.
pub(super) fn add_row(c: &mut CampaignPerf, row: &Row) {
    let name = row.text(CONVERSION_ACTION);
    if name.is_empty() {
        return;
    }
    let conversions = row.number(&["conversoes", "conversions"]).unwrap_or(0.0);
    let all = row.number(ALL_CONVERSIONS);
    match c.conversion_actions.iter_mut().find(|a| a.name == name) {
        Some(a) => {
            a.conversions += conversions;
            a.all_conversions = sum_options(a.all_conversions, all);
        }
        None => c.conversion_actions.push(ConversionActionPerf {
            name,
            category: row.text(CATEGORY),
            source: row.text(SOURCE),
            conversions,
            all_conversions: all,
        }),
    }
}

fn sum_options(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
    }
}

/// Reads what the actions say about each campaign. `account` has the campaigns of `campaigns`,
/// in the same order. Call it only when a conversion action report was read: without one, a
/// campaign with no action is a campaign nobody asked about.
pub(super) fn analyse(campaigns: &mut [CampaignPerf], account: &Account) {
    let installs_goal = account
        .campaigns
        .iter()
        .any(|c| c.kind == CampaignKind::AppInstalls);
    for (p, c) in campaigns.iter_mut().zip(&account.campaigns) {
        p.conversion_actions.sort_by(|a, b| {
            b.conversions
                .total_cmp(&a.conversions)
                .then_with(|| a.name.cmp(&b.name))
        });
        match (c.kind, c.app_store) {
            (CampaignKind::AppInstalls, Some(store)) if lacks_store_action(p, store) => {
                p.untracked = true;
                p.untracked_reason = Some(NO_ACTION_FOR_STORE.to_string());
            }
            (CampaignKind::Search | CampaignKind::DemandGen, _) if installs_goal => {
                p.off_goal_actions = off_goal(p);
            }
            _ => {}
        }
        p.conversion_actions.truncate(ACTIONS_PER_CAMPAIGN);
    }
}

/// Clicks enough to judge, and not one conversion of an action that is for this store. A
/// campaign whose report row says it converted while the actions list is empty is a mismatch
/// between reports, not a finding.
fn lacks_store_action(p: &CampaignPerf, store: AppStore) -> bool {
    let counted = p
        .conversion_actions
        .iter()
        .any(|a| a.conversions > 0.0 && counts_for(a, store));
    let known = !p.conversion_actions.is_empty() || p.metrics.conversions == 0.0;
    p.metrics.clicks >= THIN_CLICKS && !counted && known
}

/// An action counts for a store unless its name, category or source names the other one. An
/// action that names no store is taken as the campaign's own.
fn counts_for(a: &ConversionActionPerf, store: AppStore) -> bool {
    let text = format!("{} {} {}", a.name, a.category, a.source);
    named_store(&text).is_none_or(|named| named == store)
}

/// The store a text names: `Android` and `Google Play`, or `iOS`, `iPhone` and `App Store`.
/// None when it names neither, or both.
fn named_store(text: &str) -> Option<AppStore> {
    let folded = fold(text);
    let has_word = |w: &str| folded.split_whitespace().any(|x| x == w);
    let android = has_word("android") || folded.contains("google play");
    let ios = has_word("ios") || has_word("iphone") || folded.contains("app store");
    match (android, ios) {
        (true, false) => Some(AppStore::GooglePlay),
        (false, true) => Some(AppStore::AppStore),
        _ => None,
    }
}

fn is_install(a: &ConversionActionPerf) -> bool {
    let text = fold(&format!("{} {}", a.category, a.name));
    INSTALL_WORDS.iter().any(|w| text.contains(w))
}

/// Names of the actions that converted and are not installs.
fn off_goal(p: &CampaignPerf) -> Vec<String> {
    p.conversion_actions
        .iter()
        .filter(|a| a.conversions > 0.0 && !is_install(a))
        .map(|a| a.name.clone())
        .collect()
}

/// `12` or `12.50`: whole numbers without decimals.
fn count(x: f64) -> String {
    if x.fract() == 0.0 {
        format!("{x:.0}")
    } else {
        format!("{x:.2}")
    }
}

impl Performance {
    /// One line per campaign that counted actions, one for the App campaigns whose store has
    /// none, and one per Search or Demand Gen campaign that counts what is not the goal.
    pub(super) fn conversion_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in self
            .campaigns
            .iter()
            .filter(|c| !c.conversion_actions.is_empty())
        {
            let actions: Vec<String> = c
                .conversion_actions
                .iter()
                .map(|a| format!("{} ({})", a.name, count(a.conversions)))
                .collect();
            out.push(format!(
                "conversion actions of {}: {}",
                c.name,
                actions.join(", ")
            ));
        }
        let no_store: Vec<&str> = self
            .campaigns
            .iter()
            .filter(|c| c.untracked_reason.as_deref() == Some(NO_ACTION_FOR_STORE))
            .map(|c| c.name.as_str())
            .collect();
        if !no_store.is_empty() {
            out.push(format!(
                "untracked, {NO_ACTION_FOR_STORE}: no conversion action counting for the store of these App campaigns, so their installs are not measured and they get no more budget: {}",
                no_store.join(", ")
            ));
        }
        for c in self
            .campaigns
            .iter()
            .filter(|c| !c.off_goal_actions.is_empty())
        {
            out.push(format!(
                "{} optimizes on conversions that are not the goal of this account (installs): {}",
                c.name,
                c.off_goal_actions.join(", ")
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(name: &str, conversions: f64) -> ConversionActionPerf {
        ConversionActionPerf {
            name: name.into(),
            conversions,
            ..Default::default()
        }
    }

    #[test]
    fn a_text_names_one_store_or_none() {
        assert_eq!(
            named_store("App (Android) first open"),
            Some(AppStore::GooglePlay)
        );
        assert_eq!(
            named_store("Google Play installs"),
            Some(AppStore::GooglePlay)
        );
        assert_eq!(
            named_store("App (iOS) first open"),
            Some(AppStore::AppStore)
        );
        assert_eq!(
            named_store("Apple App Store download"),
            Some(AppStore::AppStore)
        );
        assert_eq!(named_store("first open"), None);
        assert_eq!(named_store("Android and iOS"), None, "both is none");
        assert_eq!(named_store("Radios"), None, "ios must be a word");
    }

    #[test]
    fn an_action_that_names_no_store_counts_for_both() {
        let a = action("first open", 3.0);
        assert!(counts_for(&a, AppStore::GooglePlay) && counts_for(&a, AppStore::AppStore));
        let android = action("App (Android) first open", 3.0);
        assert!(counts_for(&android, AppStore::GooglePlay));
        assert!(!counts_for(&android, AppStore::AppStore));
        let by_source = ConversionActionPerf {
            source: "Google Play".into(),
            ..action("first open", 3.0)
        };
        assert!(!counts_for(&by_source, AppStore::AppStore));
    }

    #[test]
    fn installs_are_told_from_other_goals_by_category_or_name() {
        assert!(is_install(&action("App (Android) first open", 1.0)));
        assert!(is_install(&action("Primeira abertura do app", 1.0)));
        let by_category = ConversionActionPerf {
            category: "Download".into(),
            ..action("event 1", 1.0)
        };
        assert!(is_install(&by_category));
        assert!(!is_install(&action("Purchase", 1.0)));
        assert!(!is_install(&action("Contato pelo site", 1.0)));
    }

    #[test]
    fn off_goal_names_only_actions_that_converted() {
        let p = CampaignPerf {
            conversion_actions: vec![
                action("Purchase", 4.0),
                action("Contact", 0.0),
                action("App (Android) first open", 9.0),
            ],
            ..Default::default()
        };
        assert_eq!(off_goal(&p), ["Purchase"]);
    }

    #[test]
    fn counts_print_whole_or_with_two_decimals() {
        assert_eq!(count(12.0), "12");
        assert_eq!(count(12.5), "12.50");
    }
}
