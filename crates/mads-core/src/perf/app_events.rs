//! Installs and in-app actions the advertiser's own product analytics counted, read from a CSV
//! the advertiser exports. Google Ads counts installs and conversions its own way: this is the
//! number to hold them against.
//!
//! Columns, in any order and any letter case: `date` (`YYYY-MM-DD`), `store` (`google_play` or
//! `app_store`), `installs` and `in_app_actions`. A comma, a semicolon or a tab separates them.
//! Numbers may be `1020`, `1,020`, `1.020` or `1.020,50`.

use serde::{Deserialize, Serialize};

use super::{model::Performance, table::Window};
use crate::{
    google::{Account, CampaignKind, normalize},
    input::{AppStore, fold},
};

/// Folded names of the columns the file needs.
const COLUMNS: [&str; 4] = ["date", "store", "installs", "in app actions"];

#[derive(Debug, Clone, PartialEq)]
struct EventRow {
    /// `YYYY-MM-DD`, so text order is date order.
    date: String,
    store: AppStore,
    installs: f64,
    in_app_actions: f64,
}

/// The rows of one events file.
#[derive(Debug, Clone, PartialEq)]
pub struct AppEvents {
    rows: Vec<EventRow>,
}

/// What the app counted for one store over a period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoreEvents {
    pub installs: f64,
    pub in_app_actions: f64,
}

/// What an App campaign got from the events file. The numbers are the store's, so two App
/// campaigns of one store carry the same ones: the file does not say which campaign an install
/// came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppPerf {
    pub store: AppStore,
    /// Installs the app's own analytics counted in the report window.
    pub app_installs_seen: f64,
    /// In-app actions those installs led to.
    pub in_app_actions: f64,
    /// `in_app_actions` as a share of `app_installs_seen`.
    pub install_to_action_pct: Option<f64>,
    /// Cost of the store's App campaigns over `in_app_actions`.
    pub cost_per_in_app_action: Option<f64>,
    /// Google's conversions over `app_installs_seen`. Above 1, Google counts installs the app
    /// never sees.
    pub store_vs_app_install_ratio: Option<f64>,
    /// Cost of every App campaign of the store in the window.
    pub store_cost: f64,
}

impl AppPerf {
    pub fn new(store: AppStore, seen: StoreEvents, cost: f64, conversions: f64) -> AppPerf {
        let ratio = |a: f64, b: f64| (b > 0.0).then(|| (a / b * 100.0).round() / 100.0);
        AppPerf {
            store,
            app_installs_seen: seen.installs,
            in_app_actions: seen.in_app_actions,
            install_to_action_pct: ratio(seen.in_app_actions * 100.0, seen.installs),
            cost_per_in_app_action: ratio(cost, seen.in_app_actions),
            store_vs_app_install_ratio: ratio(conversions, seen.installs),
            store_cost: cost,
        }
    }
}

impl AppEvents {
    /// Reads the file. Err names the line or the column that is wrong.
    pub fn parse(bytes: &[u8]) -> Result<AppEvents, String> {
        let text = super::table::decode(bytes)?;
        let records = best_records(&text);
        let header: Vec<String> = records
            .first()
            .map(|h| h.iter().map(|c| fold(c)).collect())
            .unwrap_or_default();
        let at = |name: &str| header.iter().position(|h| h == name);
        let missing: Vec<String> = COLUMNS
            .iter()
            .filter(|c| at(c).is_none())
            .map(|c| c.replace(' ', "_"))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "missing column {} (the file needs date, store, installs and in_app_actions)",
                missing.join(", ")
            ));
        }
        let [date, store, installs, actions] = COLUMNS.map(|c| at(c).unwrap_or_default());
        let mut rows = Vec::new();
        for (i, cells) in records.iter().enumerate().skip(1) {
            if cells.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            let cell = |n: usize| cells.get(n).map_or("", |c| c.trim());
            let at_line = |what: String| format!("line {}: {what}", i + 1);
            let number = |n: usize| {
                count(cell(n)).ok_or_else(|| at_line(format!("'{}' is not a number", cell(n))))
            };
            rows.push(EventRow {
                date: iso_date(cell(date)).ok_or_else(|| {
                    at_line(format!("'{}' is not a date, use YYYY-MM-DD", cell(date)))
                })?,
                store: store_of(cell(store)).ok_or_else(|| {
                    at_line(format!(
                        "unknown store '{}', use google_play or app_store",
                        cell(store)
                    ))
                })?,
                installs: number(installs)?,
                in_app_actions: number(actions)?,
            });
        }
        Ok(AppEvents { rows })
    }

    /// The sum of one store over the report window, both days included. Without a window the
    /// whole file counts. None when the file has no row of the store in the period.
    pub fn totals(&self, store: AppStore, window: Option<&Window>) -> Option<StoreEvents> {
        let inside = |r: &&EventRow| window.is_none_or(|w| r.date >= w.start && r.date <= w.end);
        let rows: Vec<&EventRow> = self
            .rows
            .iter()
            .filter(|r| r.store == store)
            .filter(inside)
            .collect();
        if rows.is_empty() {
            return None;
        }
        Some(StoreEvents {
            installs: rows.iter().map(|r| r.installs).sum(),
            in_app_actions: rows.iter().map(|r| r.in_app_actions).sum(),
        })
    }
}

/// Puts what the app counted on each App campaign of the account, store by store. A store the
/// file has no row for in the window leaves its campaigns without numbers and is listed in
/// `app_events_missing`. The account must name the store of each App campaign.
pub fn attach(p: &mut Performance, account: &Account, events: &AppEvents) {
    for store in [AppStore::GooglePlay, AppStore::AppStore] {
        let names: Vec<&str> = account
            .campaigns
            .iter()
            .filter(|c| c.kind == CampaignKind::AppInstalls && c.app_store == Some(store))
            .map(|c| c.name.as_str())
            .collect();
        if names.is_empty() {
            continue;
        }
        let Some(seen) = events.totals(store, p.window.as_ref()) else {
            p.app_events_missing
                .extend(names.iter().map(|n| n.to_string()));
            continue;
        };
        let keys: Vec<String> = names.iter().map(|n| normalize(n)).collect();
        let ours = |name: &str| keys.contains(&normalize(name));
        let (mut cost, mut conversions) = (0.0, 0.0);
        for c in p.campaigns.iter().filter(|c| ours(&c.name)) {
            cost += c.metrics.cost;
            conversions += c.metrics.conversions;
        }
        let app = AppPerf::new(store, seen, cost, conversions);
        for c in p.campaigns.iter_mut().filter(|c| ours(&c.name)) {
            c.app = Some(app.clone());
        }
    }
}

/// The records of the delimiter under which the header has the most of the needed columns.
fn best_records(text: &str) -> Vec<Vec<String>> {
    let mut best: Vec<Vec<String>> = Vec::new();
    let mut best_hits = 0;
    for delimiter in *b";,\t" {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(text.as_bytes());
        let parsed: Result<Vec<Vec<String>>, csv::Error> = reader
            .records()
            .map(|r| r.map(|r| r.iter().map(str::to_string).collect()))
            .collect();
        let Ok(records) = parsed else {
            continue;
        };
        let hits = records.first().map_or(0, |h| {
            let folded: Vec<String> = h.iter().map(|c| fold(c)).collect();
            COLUMNS
                .iter()
                .filter(|c| folded.iter().any(|f| f == *c))
                .count()
        });
        if best.is_empty() || hits > best_hits {
            best = records;
            best_hits = hits;
        }
    }
    best
}

/// `2026-10-03`, or a timestamp that starts with it, as `YYYY-MM-DD`.
fn iso_date(cell: &str) -> Option<String> {
    let mut parts = cell.get(..10)?.split('-').map(str::parse::<u32>);
    let y = parts.next()?.ok()?;
    let m = parts.next()?.ok()?;
    let d = parts.next()?.ok()?;
    ((1000..=9999).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d))
        .then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn store_of(cell: &str) -> Option<AppStore> {
    match fold(cell).as_str() {
        "google play" => Some(AppStore::GooglePlay),
        "app store" => Some(AppStore::AppStore),
        _ => None,
    }
}

/// A count written `1020`, `1,020`, `1.020`, `1,020.5` or `1.020,5`. Empty is zero. None for text
/// and negative numbers.
fn count(cell: &str) -> Option<f64> {
    let s = cell.trim();
    if s.is_empty() || s == "--" {
        return Some(0.0);
    }
    let plain = match (s.rfind(','), s.rfind('.')) {
        (Some(c), Some(d)) if c > d => s.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => s.replace(',', ""),
        (Some(_), None) => one_separator(s, ','),
        (None, Some(_)) => one_separator(s, '.'),
        (None, None) => s.to_string(),
    };
    plain
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
}

/// Only one kind of separator: several of it, or one with exactly 3 digits after it, groups
/// thousands in a count. Otherwise it is a decimal point.
fn one_separator(s: &str, sep: char) -> String {
    let groups: Vec<&str> = s.split(sep).collect();
    let thousands = groups.len() > 2 || groups.get(1).is_some_and(|g| g.len() == 3);
    if thousands {
        s.replace(sep, "")
    } else {
        s.replace(sep, ".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{BidStrategy, Campaign, Cents, Intent},
        perf::{CampaignPerf, Metrics},
    };

    const COMMA: &str = "date,store,installs,in_app_actions\n2026-10-02,google_play,100,10\n2026-10-03,google_play,\"1.000\",50\n2026-10-04,google_play,1000,50\n2026-10-05,app_store,40,4\n";

    fn window() -> Window {
        Window {
            start: "2026-10-03".into(),
            end: "2026-10-05".into(),
            days: 3,
        }
    }

    fn totals(text: &str, store: AppStore) -> Option<StoreEvents> {
        AppEvents::parse(text.as_bytes())
            .unwrap()
            .totals(store, Some(&window()))
    }

    #[test]
    fn sums_a_store_over_the_report_window_only() {
        let got = totals(COMMA, AppStore::GooglePlay).unwrap();
        assert_eq!((got.installs, got.in_app_actions), (2000.0, 100.0));
        let got = totals(COMMA, AppStore::AppStore).unwrap();
        assert_eq!((got.installs, got.in_app_actions), (40.0, 4.0));
        let all = AppEvents::parse(COMMA.as_bytes())
            .unwrap()
            .totals(AppStore::GooglePlay, None)
            .unwrap();
        assert_eq!(all.installs, 2100.0, "no window, the whole file");
    }

    #[test]
    fn a_store_without_rows_in_the_window_has_no_totals() {
        let text = "date,store,installs,in_app_actions\n2026-09-01,app_store,5,1\n";
        assert_eq!(totals(text, AppStore::AppStore), None);
        assert_eq!(totals(text, AppStore::GooglePlay), None);
    }

    #[test]
    fn reads_semicolons_and_comma_decimals() {
        let text = "date;store;installs;in_app_actions\n2026-10-03;google_play;1.020,50;10,5\n2026-10-04;google_play;0,5;1\n";
        let got = totals(text, AppStore::GooglePlay).unwrap();
        assert_eq!((got.installs, got.in_app_actions), (1021.0, 11.5));
    }

    #[test]
    fn reads_dot_decimals_tabs_and_header_case() {
        let text = "Date\tStore\tInstalls\tIn App Actions\n2026-10-03\tGoogle Play\t1,020.50\t7\n2026-10-04T10:00:00Z\tapp_store\t3\t--\n";
        let got = totals(text, AppStore::GooglePlay).unwrap();
        assert_eq!((got.installs, got.in_app_actions), (1020.5, 7.0));
        let got = totals(text, AppStore::AppStore).unwrap();
        assert_eq!((got.installs, got.in_app_actions), (3.0, 0.0));
    }

    #[test]
    fn counts_are_read_with_either_separator() {
        for (cell, want) in [
            ("1020", 1020.0),
            ("1,020", 1020.0),
            ("1.020", 1020.0),
            ("1.020.300", 1020300.0),
            ("1,020.50", 1020.5),
            ("1.020,50", 1020.5),
            ("12,5", 12.5),
            ("12.5", 12.5),
            ("", 0.0),
        ] {
            assert_eq!(count(cell), Some(want), "{cell}");
        }
        for bad in ["abc", "-3", "1.2.3,4,5x"] {
            assert_eq!(count(bad), None, "{bad}");
        }
    }

    #[test]
    fn an_unknown_store_names_the_line() {
        let text = "date,store,installs,in_app_actions\n2026-10-03,google_play,1,1\n2026-10-04,huawei,1,1\n";
        let err = AppEvents::parse(text.as_bytes()).unwrap_err();
        assert_eq!(
            err,
            "line 3: unknown store 'huawei', use google_play or app_store"
        );
    }

    #[test]
    fn missing_columns_are_named() {
        let err = AppEvents::parse(b"date,store,installs\n2026-10-03,google_play,1\n").unwrap_err();
        assert!(err.starts_with("missing column in_app_actions"), "{err}");
        let err = AppEvents::parse(b"foo,bar\n1,2\n").unwrap_err();
        assert!(
            err.contains("date, store, installs, in_app_actions"),
            "{err}"
        );
        assert!(AppEvents::parse(b"").is_err());
    }

    #[test]
    fn a_bad_number_or_date_names_the_line() {
        let head = "date,store,installs,in_app_actions\n";
        let err = AppEvents::parse(format!("{head}2026-10-03,google_play,many,1\n").as_bytes())
            .unwrap_err();
        assert_eq!(err, "line 2: 'many' is not a number");
        let err =
            AppEvents::parse(format!("{head}03/10/2026,google_play,1,1\n").as_bytes()).unwrap_err();
        assert!(
            err.starts_with("line 2: '03/10/2026' is not a date"),
            "{err}"
        );
    }

    #[test]
    fn a_file_with_a_byte_order_mark_is_read() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend(COMMA.as_bytes());
        assert!(AppEvents::parse(&bytes).is_ok());
    }

    fn app_campaign(name: &str, store: Option<AppStore>) -> Campaign {
        Campaign {
            app_store: store,
            kind: CampaignKind::AppInstalls,
            asset_groups: Vec::new(),
            name: name.into(),
            slug: crate::input::slugify(name),
            intent: Intent::Generic,
            daily_budget: Cents(5000),
            bid_strategy: BidStrategy::MaximizeConversions,
            rationale: "r".into(),
            planned_ad_groups: Vec::new(),
            ad_groups: Vec::new(),
            negatives: Vec::new(),
            assets: None,
        }
    }

    fn perf_of(name: &str, cost: f64, conversions: f64) -> CampaignPerf {
        CampaignPerf {
            name: name.into(),
            metrics: Metrics {
                cost,
                conversions,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn each_app_campaign_gets_the_numbers_of_its_store() {
        let account = Account {
            brand_kit: None,
            campaigns: vec![
                app_campaign("App Android", Some(AppStore::GooglePlay)),
                app_campaign("App iOS", Some(AppStore::AppStore)),
            ],
        };
        let mut p = Performance {
            window: Some(window()),
            campaigns: vec![
                perf_of("App Android", 300.0, 2400.0),
                perf_of("App iOS", 20.0, 40.0),
            ],
            ..Default::default()
        };
        attach(
            &mut p,
            &account,
            &AppEvents::parse(COMMA.as_bytes()).unwrap(),
        );
        let android = p.campaigns[0].app.as_ref().unwrap();
        assert_eq!(android.store, AppStore::GooglePlay);
        assert_eq!(android.app_installs_seen, 2000.0);
        assert_eq!(android.in_app_actions, 100.0);
        assert_eq!(android.install_to_action_pct, Some(5.0));
        assert_eq!(android.cost_per_in_app_action, Some(3.0));
        assert_eq!(android.store_vs_app_install_ratio, Some(1.2));
        assert_eq!(android.store_cost, 300.0);
        let ios = p.campaigns[1].app.as_ref().unwrap();
        assert_eq!(ios.cost_per_in_app_action, Some(5.0));
        assert!(p.app_events_missing.is_empty());
    }

    #[test]
    fn campaigns_of_one_store_share_its_cost() {
        let account = Account {
            brand_kit: None,
            campaigns: vec![
                app_campaign("App A", Some(AppStore::GooglePlay)),
                app_campaign("App B", Some(AppStore::GooglePlay)),
            ],
        };
        let mut p = Performance {
            window: Some(window()),
            campaigns: vec![perf_of("App A", 100.0, 10.0), perf_of("app b", 200.0, 20.0)],
            ..Default::default()
        };
        attach(
            &mut p,
            &account,
            &AppEvents::parse(COMMA.as_bytes()).unwrap(),
        );
        for c in &p.campaigns {
            let app = c.app.as_ref().unwrap();
            assert_eq!(app.store_cost, 300.0);
            assert_eq!(app.cost_per_in_app_action, Some(3.0));
        }
    }

    #[test]
    fn a_store_without_rows_is_listed_and_its_campaign_has_no_numbers() {
        let account = Account {
            brand_kit: None,
            campaigns: vec![app_campaign("App iOS", Some(AppStore::AppStore))],
        };
        let text = "date,store,installs,in_app_actions\n2026-10-03,google_play,5,1\n";
        let mut p = Performance {
            window: Some(window()),
            campaigns: vec![perf_of("App iOS", 20.0, 40.0)],
            ..Default::default()
        };
        attach(
            &mut p,
            &account,
            &AppEvents::parse(text.as_bytes()).unwrap(),
        );
        assert_eq!(p.campaigns[0].app, None);
        assert_eq!(p.app_events_missing, ["App iOS"]);
    }

    #[test]
    fn without_actions_there_is_no_cost_per_action() {
        let app = AppPerf::new(
            AppStore::GooglePlay,
            StoreEvents {
                installs: 0.0,
                in_app_actions: 0.0,
            },
            50.0,
            5.0,
        );
        assert_eq!(app.install_to_action_pct, None);
        assert_eq!(app.cost_per_in_app_action, None);
        assert_eq!(app.store_vs_app_install_ratio, None);
        assert_eq!(app.store_cost, 50.0);
    }
}
