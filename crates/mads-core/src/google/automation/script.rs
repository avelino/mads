//! `daily-sync.js`: the Google Ads script that reads the account and links the workbooks.

use serde_json::{Value, json};

use super::columns::{FeedColumn, image_assets, image_campaigns};
use super::model::Package;

const TEMPLATE: &str = include_str!("daily_sync.js.tpl");

/// The script with the package's folder, automated campaigns, mirror list and feed columns.
/// `mirrors` is `(file path without .xlsx, tab, range of "0. Feed")`.
pub(crate) fn daily_sync_js(p: &Package, mirrors: &[(&str, &str, String)]) -> String {
    let mirrors: Vec<Value> = mirrors
        .iter()
        .map(|(file, tab, range)| json!([file, tab, range]))
        .collect();
    let feed_col = format!(
        "{{ action: {}, automated: {}, campaign: {}, adGroup: {}, keyword: {} }}",
        FeedColumn::Action.index(),
        FeedColumn::Automated.index(),
        FeedColumn::Campaign.index(),
        FeedColumn::AdGroup.index(),
        FeedColumn::Keyword.index()
    );
    // `Value`'s Display is JSON and cannot fail, unlike `serde_json::to_string`.
    fill(
        TEMPLATE,
        &[
            ("__FOLDER_NAME__", json!(p.folder).to_string()),
            (
                "__AUTOMATED_CAMPAIGNS__",
                json!(p.automated_campaigns).to_string(),
            ),
            ("__MIRRORS__", Value::Array(mirrors).to_string()),
            ("__FEED_COL__", feed_col),
            ("__IMAGE_CAMPAIGN_COL__", image_campaign_col().to_string()),
            ("__IMAGE_ASSET_COL__", image_asset_col().to_string()),
        ],
    )
}

/// Zero-based columns of the `Image Campaigns` tab, keyed by the names the script reads.
fn image_campaign_col() -> Value {
    use image_campaigns as c;
    json!({
        "key": c::KEY,
        "campaign": c::CAMPAIGN,
        "campaignType": c::CAMPAIGN_TYPE,
        "budget": c::BUDGET,
        "bidStrategy": c::BID_STRATEGY,
        "maxCpc": c::MAX_CPC,
        "language": c::LANGUAGE,
        "location": c::LOCATION,
        "finalUrl": c::FINAL_URL,
        "businessName": c::BUSINESS_NAME,
        "appId": c::APP_ID,
        "appStore": c::APP_STORE,
        "action": c::ACTION,
    })
}

/// Zero-based columns of the `Image Assets` tab.
fn image_asset_col() -> Value {
    use image_assets as a;
    json!({
        "key": a::KEY,
        "group": a::GROUP,
        "field": a::FIELD,
        "text": a::TEXT,
        "imageFile": a::IMAGE_FILE,
    })
}

/// Replaces each placeholder in the template text only, in one pass, so a value that happens
/// to contain another placeholder's name (a folder called `__MIRRORS__`) stays literal.
fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        let next = values
            .iter()
            .filter_map(|(key, value)| rest.find(key).map(|at| (at, *key, value)))
            .min_by_key(|(at, _, _)| *at);
        let Some((at, key, value)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..at]);
        out.push_str(value);
        rest = &rest[at + key.len()..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::automation::testutil::sample_package;
    use std::path::Path;
    use std::process::Command;

    fn mirrors() -> Vec<(&'static str, &'static str, String)> {
        vec![("Daily/D1. New Ad Groups", "Feed", "Feed!A5:AL156".into())]
    }

    fn node_available() -> bool {
        Command::new("node").arg("--version").output().is_ok()
    }

    fn run(cmd: &mut Command) {
        let out = cmd.output().expect("node runs");
        assert!(
            out.status.success(),
            "{cmd:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn generated_script_passes_node_check_and_tests() {
        if !node_available() {
            eprintln!("skipping daily-sync.js node tests: `node` is not on PATH");
            return;
        }
        let js = daily_sync_js(&sample_package(), &mirrors());
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("daily-sync.js");
        std::fs::write(&file, &js).expect("write script");
        run(Command::new("node").arg("--check").arg(&file));
        let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/js");
        run(Command::new("node")
            .arg("--test")
            .arg(tests.join("daily_sync.test.mjs"))
            .arg(tests.join("image_campaigns.test.mjs"))
            .env("DAILY_SYNC_JS", &file));
    }

    #[test]
    fn values_are_json_escaped() {
        let mut p = sample_package();
        p.folder = "Don \"Melchor\" Ads Automation".into();
        p.automated_campaigns = vec!["Catalog - \"Wines\"".into()];
        let js = daily_sync_js(&p, &mirrors());
        assert!(js.contains(r#"var FOLDER_NAME = "Don \"Melchor\" Ads Automation";"#));
        assert!(js.contains(r#"var AUTOMATED_CAMPAIGNS = ["Catalog - \"Wines\""];"#));
        assert!(
            js.contains(r#"var MIRRORS = [["Daily/D1. New Ad Groups","Feed","Feed!A5:AL156"]];"#)
        );
    }

    #[test]
    fn feed_columns_come_from_the_rust_table() {
        use crate::google::automation::columns::FeedColumn;
        let js = daily_sync_js(&sample_package(), &mirrors());
        let expected = format!(
            "var FEED_COL = {{ action: {}, automated: {}, campaign: {}, adGroup: {}, keyword: {} }};",
            FeedColumn::Action.index(),
            FeedColumn::Automated.index(),
            FeedColumn::Campaign.index(),
            FeedColumn::AdGroup.index(),
            FeedColumn::Keyword.index()
        );
        assert!(js.contains(&expected), "{expected}");
    }

    #[test]
    fn image_columns_come_from_the_rust_tables() {
        let js = daily_sync_js(&sample_package(), &mirrors());
        let campaigns = format!("var IMAGE_CAMPAIGN_COL = {};", image_campaign_col());
        let assets = format!("var IMAGE_ASSET_COL = {};", image_asset_col());
        assert!(js.contains(&campaigns), "{campaigns}");
        assert!(js.contains(&assets), "{assets}");
        assert_eq!(image_campaign_col()["action"], image_campaigns::ACTION);
        assert_eq!(image_asset_col()["imageFile"], image_assets::IMAGE_FILE);
    }

    #[test]
    fn a_value_naming_a_placeholder_stays_literal() {
        let mut p = sample_package();
        p.folder = "__MIRRORS__".into();
        let js = daily_sync_js(&p, &mirrors());
        assert!(js.contains(r#"var FOLDER_NAME = "__MIRRORS__";"#));
    }

    #[test]
    fn no_placeholder_is_left() {
        let js = daily_sync_js(&sample_package(), &mirrors());
        assert!(!js.contains("__"), "unfilled placeholder in daily-sync.js");
        assert!(js.contains("function main()"));
        assert!(js.contains("function linkSheets()"));
        assert!(js.contains("function setupImageCampaigns()"));
    }
}
