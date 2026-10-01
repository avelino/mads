use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::*;
use crate::testutil;
use crate::workspace::Workspace;

type Case = (&'static str, Box<dyn Fn(&mut Value)>, &'static str);

fn shared() -> SharedWorkspace {
    Arc::new(Mutex::new(Workspace::new(testutil::input())))
}

async fn plan_tools(ws: &SharedWorkspace) -> MissionTools {
    MissionTools::new(ws.clone(), MissionKind::Plan, ToolSettings::default(), None)
        .await
        .unwrap()
}

async fn campaign_tools(ws: &SharedWorkspace, slug: &str) -> MissionTools {
    let kind = MissionKind::Campaign { slug: slug.into() };
    MissionTools::new(ws.clone(), kind, ToolSettings::default(), None)
        .await
        .unwrap()
}

fn error_codes(out: &ToolOutput) -> Vec<String> {
    out.content["errors"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| e["code"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn texts(prefix: &str, n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("{prefix} numero {i}")).collect()
}

fn brand_kit_args() -> Value {
    json!({"headlines": texts("Titulo da marca", 10), "descriptions": texts("Descricao da marca com chamada pra acao", 3)})
}

fn plan_args() -> Value {
    json!({"campaigns": [
        {"name": "Vinellu - Catalogo", "intent": "catalog", "daily_budget": 30.0,
         "bid_strategy": {"type": "manual_cpc"}, "rationale": "intencao alta",
         "ad_groups": [
            {"name": "alamos", "theme": "alamos malbec", "entity_ids": ["alamos-malbec"]},
            {"name": "luigi", "theme": "luigi bosca", "entity_ids": ["luigi-bosca"], "final_url": "https://vinellu.com/w/luigi"}
         ]},
        {"name": "Vinellu - Marca", "intent": "brand", "daily_budget": 20.0,
         "bid_strategy": {"type": "manual_cpc"}, "rationale": "protege marca",
         "ad_groups": [{"name": "marca", "theme": "vinellu", "entity_ids": []}]}
    ]})
}

/// Workspace with a brand kit and the two-campaign plan above.
async fn planned() -> SharedWorkspace {
    let ws = shared();
    let t = plan_tools(&ws).await;
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let out = t.call("set_account_plan", plan_args()).await;
    assert!(!out.is_error, "{}", out.content);
    ws
}

fn ad_group_args(name: &str) -> Value {
    json!({
        "name": name, "default_cpc": 1.5, "cpc_rationale": "estimativa conservadora",
        "keywords": {"variants": ["alamos malbec", "alamos"], "modifiers": ["review", "safra"], "exact_heads": true},
        "negatives": [{"text": "emprego", "match": "phrase"}],
        "rsa": {"headlines": texts("Titulo do grupo", 5), "descriptions": ["Descricao do grupo para teste"], "path1": "vinhos", "path2": ""}
    })
}

fn assets_args() -> Value {
    let sitelinks: Vec<Value> = ["app", "sobre", "blog", "ajuda"]
        .iter()
        .map(|p| json!({"text": format!("Link {p}"), "url": format!("https://vinellu.com/{p}")}))
        .collect();
    json!({
        "sitelinks": sitelinks,
        "callouts": texts("Callout", 4),
        "snippets": [{"header": "types", "values": texts("Tipo", 3)}]
    })
}

// ---- contract ----

#[tokio::test]
async fn plan_toolset_is_exactly_the_documented_tools() {
    let ws = shared();
    let names: Vec<String> = plan_tools(&ws)
        .await
        .specs()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(
        names,
        [
            "get_business",
            "query_catalog",
            "set_brand_kit",
            "set_account_plan",
            "finish"
        ]
    );
}

#[tokio::test]
async fn campaign_toolset_is_exactly_the_documented_tools() {
    let ws = planned().await;
    let names: Vec<String> = campaign_tools(&ws, "vinellu-catalogo")
        .await
        .specs()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(
        names,
        [
            "get_brief",
            "upsert_ad_group",
            "set_campaign_negatives",
            "set_assets",
            "validate",
            "finish"
        ]
    );
}

#[tokio::test]
async fn unknown_tool_and_tools_of_other_missions_are_refused() {
    let ws = planned().await;
    let plan = plan_tools(&ws).await;
    for name in ["nope", "upsert_ad_group", "get_brief"] {
        let out = plan.call(name, json!({})).await;
        assert!(out.is_error);
        assert_eq!(error_codes(&out), ["UNKNOWN_TOOL"], "{name}");
    }
    let camp = campaign_tools(&ws, "vinellu-catalogo").await;
    assert_eq!(
        error_codes(&camp.call("set_account_plan", json!({})).await),
        ["UNKNOWN_TOOL"]
    );
}

#[tokio::test]
async fn unknown_argument_is_rejected_with_args_error_and_changes_nothing() {
    let ws = shared();
    let before = ws.lock().await.clone();
    let out = plan_tools(&ws)
        .await
        .call(
            "set_brand_kit",
            json!({"headlines": [], "descriptions": [], "oops": 1}),
        )
        .await;
    assert!(out.is_error);
    assert_eq!(error_codes(&out), ["ARGS"]);
    assert_eq!(*ws.lock().await, before);
}

#[tokio::test]
async fn call_budget_returns_limit_error() {
    let ws = shared();
    let settings = ToolSettings {
        max_turns: 1,
        ..ToolSettings::default()
    };
    let t = MissionTools::new(ws, MissionKind::Plan, settings, None)
        .await
        .unwrap();
    for _ in 0..4 {
        assert!(!t.call("get_business", json!({})).await.is_error);
    }
    assert_eq!(
        error_codes(&t.call("get_business", json!({})).await),
        ["LIMIT"]
    );
}

#[tokio::test]
async fn campaign_mission_for_unknown_slug_cannot_be_created() {
    let ws = planned().await;
    let kind = MissionKind::Campaign {
        slug: "nao-existe".into(),
    };
    assert!(
        MissionTools::new(ws, kind, ToolSettings::default(), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn every_tool_schema_is_portable_across_providers() {
    let ws = planned().await;
    let mut all = plan_tools(&ws).await.specs();
    all.extend(campaign_tools(&ws, "vinellu-catalogo").await.specs());
    for spec in all {
        let bad = schema::non_portable_keywords(&spec.input_schema);
        assert!(bad.is_empty(), "{}: {bad:?}", spec.name);
        assert_eq!(spec.input_schema["type"], "object", "{}", spec.name);
        assert!(!spec.description.is_empty(), "{}", spec.name);
    }
}

// ---- plan tools ----

#[tokio::test]
async fn get_business_reports_profile_budget_and_rules() {
    let ws = shared();
    let out = plan_tools(&ws).await.call("get_business", json!({})).await;
    assert!(!out.is_error);
    let r = &out.content["result"];
    assert_eq!(r["business"]["name"], "Vinellu");
    assert_eq!(r["budget"]["daily"], 50.0);
    assert_eq!(r["budget"]["currency"], "BRL");
    assert_eq!(r["rules"]["headline_max_chars"], 30);
    assert_eq!(r["rules"]["bid_strategies"], json!(["manual_cpc"]));
}

#[tokio::test]
async fn query_catalog_filters_and_paginates() {
    let ws = shared();
    let t = plan_tools(&ws).await;
    let all = t.call("query_catalog", json!({})).await;
    assert_eq!(all.content["result"]["total"], 3);
    assert_eq!(
        all.content["result"]["categories"],
        json!([{"name": "alentejo", "count": 1}, {"name": "malbec", "count": 2}])
    );
    let by_cat = t.call("query_catalog", json!({"category": "malbec"})).await;
    assert_eq!(by_cat.content["result"]["total"], 2);
    let by_text = t.call("query_catalog", json!({"contains": "CARTU"})).await;
    assert_eq!(by_text.content["result"]["items"][0]["id"], "cartuxa");
    let page = t
        .call("query_catalog", json!({"offset": 1, "limit": 1}))
        .await;
    assert_eq!(page.content["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(page.content["result"]["items"][0]["id"], "luigi-bosca");
}

#[tokio::test]
async fn set_brand_kit_stores_valid_kit() {
    let ws = shared();
    let out = plan_tools(&ws)
        .await
        .call("set_brand_kit", brand_kit_args())
        .await;
    assert!(!out.is_error, "{}", out.content);
    assert_eq!(
        ws.lock()
            .await
            .account
            .brand_kit
            .as_ref()
            .unwrap()
            .headlines
            .len(),
        10
    );
}

#[tokio::test]
async fn set_brand_kit_rejects_long_headline_and_keeps_previous_kit() {
    let ws = shared();
    let t = plan_tools(&ws).await;
    t.call("set_brand_kit", brand_kit_args()).await;
    let mut bad = brand_kit_args();
    bad["headlines"][0] = json!("x".repeat(31));
    let out = t.call("set_brand_kit", bad).await;
    assert!(out.is_error);
    assert!(error_codes(&out).contains(&"E01".to_string()));
    assert_eq!(
        ws.lock()
            .await
            .account
            .brand_kit
            .as_ref()
            .unwrap()
            .headlines[0],
        "Titulo da marca numero 1"
    );
}

#[tokio::test]
async fn set_account_plan_resolves_urls_and_slugs() {
    let ws = planned().await;
    let guard = ws.lock().await;
    let camps = &guard.account.campaigns;
    assert_eq!(camps.len(), 2);
    assert_eq!(camps[0].slug, "vinellu-catalogo");
    assert_eq!(camps[0].daily_budget.0, 3000);
    assert_eq!(
        camps[0].planned_ad_groups[0].final_url,
        "https://vinellu.com/w/alamos"
    );
    assert_eq!(
        camps[0].planned_ad_groups[1].final_url,
        "https://vinellu.com/w/luigi"
    );
    assert_eq!(
        camps[1].planned_ad_groups[0].final_url,
        "https://vinellu.com"
    );
}

#[tokio::test]
async fn set_account_plan_rejects_bad_plans_and_keeps_state() {
    let ws = planned().await;
    let t = plan_tools(&ws).await;
    let before = ws.lock().await.account.clone();
    let cases: Vec<Case> = vec![
        (
            "budget sum",
            Box::new(|p| p["campaigns"][0]["daily_budget"] = json!(10.0)),
            "E06",
        ),
        (
            "unknown entity",
            Box::new(|p| p["campaigns"][0]["ad_groups"][0]["entity_ids"] = json!(["x"])),
            "E12",
        ),
        (
            "foreign url",
            Box::new(|p| {
                p["campaigns"][0]["ad_groups"][0]["final_url"] = json!("https://evil.com")
            }),
            "E07",
        ),
        (
            "unsupported bid",
            Box::new(|p| p["campaigns"][0]["bid_strategy"]["type"] = json!("maximize_clicks")),
            "UNSUPPORTED",
        ),
        (
            "three decimals",
            Box::new(|p| p["campaigns"][0]["daily_budget"] = json!(30.123)),
            "E06",
        ),
        (
            "duplicate names",
            Box::new(|p| p["campaigns"][1]["name"] = json!("Vinellu - Catalogo")),
            "E12",
        ),
    ];
    for (label, mutate, code) in cases {
        let mut p = plan_args();
        mutate(&mut p);
        let out = t.call("set_account_plan", p).await;
        assert!(out.is_error, "{label}");
        assert!(
            error_codes(&out).contains(&code.to_string()),
            "{label}: {:?}",
            error_codes(&out)
        );
        assert_eq!(
            ws.lock().await.account,
            before,
            "{label} mutated the workspace"
        );
    }
}

#[tokio::test]
async fn set_account_plan_with_six_campaigns_is_too_many() {
    let ws = shared();
    let camps: Vec<Value> = (0..6)
        .map(|i| json!({"name": format!("C{i}"), "intent": "generic", "daily_budget": 8.0, "bid_strategy": {"type": "manual_cpc"}, "rationale": "r", "ad_groups": [{"name": "g", "theme": "t", "entity_ids": []}]}))
        .collect();
    let out = plan_tools(&ws)
        .await
        .call("set_account_plan", json!({"campaigns": camps}))
        .await;
    assert!(error_codes(&out).contains(&"E13".to_string()));
}

#[tokio::test]
async fn plan_finish_needs_kit_and_plan() {
    let ws = shared();
    let t = plan_tools(&ws).await;
    assert!(t.call("finish", json!({})).await.is_error);
    assert!(!t.finished());
    t.call("set_brand_kit", brand_kit_args()).await;
    assert!(t.call("finish", json!({})).await.is_error);
    t.call("set_account_plan", plan_args()).await;
    assert!(!t.call("finish", json!({})).await.is_error);
    assert!(t.finished());
}

// ---- campaign tools ----

#[tokio::test]
async fn get_brief_is_scoped_to_its_campaign() {
    let ws = planned().await;
    let out = campaign_tools(&ws, "vinellu-catalogo")
        .await
        .call("get_brief", json!({}))
        .await;
    assert!(!out.is_error);
    let r = &out.content["result"];
    assert_eq!(r["campaign"]["name"], "Vinellu - Catalogo");
    assert_eq!(r["campaign"]["daily_budget"], 30.0);
    assert_eq!(
        r["campaign"]["planned_ad_groups"].as_array().unwrap().len(),
        2
    );
    assert_eq!(r["entities"][0]["id"], "alamos-malbec");
    assert_eq!(r["brand_kit"]["headlines"].as_array().unwrap().len(), 10);
    assert!(!out.content.to_string().contains("Vinellu - Marca"));
}

#[tokio::test]
async fn upsert_ad_group_expands_keywords_and_stores_in_plan_order() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let out = t.call("upsert_ad_group", ad_group_args("luigi")).await;
    assert!(!out.is_error, "{}", out.content);
    assert!(
        !t.call("upsert_ad_group", ad_group_args("alamos"))
            .await
            .is_error
    );
    let guard = ws.lock().await;
    let c = &guard.account.campaigns[0];
    assert_eq!(
        c.ad_groups
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["alamos", "luigi"]
    );
    assert_eq!(c.ad_groups[0].keywords.len(), 6 + 2);
    assert_eq!(c.ad_groups[0].default_cpc.0, 150);
    assert_eq!(c.ad_groups[0].final_url, "https://vinellu.com/w/alamos");
    assert_eq!(c.ad_groups[0].rsa.path2, None);
    assert!(out.summary.contains("luigi"), "{}", out.summary);
}

#[tokio::test]
async fn upsert_ad_group_replaces_the_same_name() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    t.call("upsert_ad_group", ad_group_args("alamos")).await;
    let mut again = ad_group_args("alamos");
    again["default_cpc"] = json!(2.0);
    assert!(!t.call("upsert_ad_group", again).await.is_error);
    let guard = ws.lock().await;
    assert_eq!(guard.account.campaigns[0].ad_groups.len(), 1);
    assert_eq!(guard.account.campaigns[0].ad_groups[0].default_cpc.0, 200);
}

#[tokio::test]
async fn upsert_ad_group_errors_do_not_mutate() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let cases: Vec<Case> = vec![
        (
            "unplanned name",
            Box::new(|a| a["name"] = json!("inventado")),
            "E12",
        ),
        (
            "long headline",
            Box::new(|a| a["rsa"]["headlines"][0] = json!("x".repeat(31))),
            "E01",
        ),
        (
            "seven variants",
            Box::new(|a| a["keywords"]["variants"] = json!(["a", "b", "c", "d", "e", "f", "g"])),
            "E13",
        ),
        (
            "bad keyword char",
            Box::new(|a| a["keywords"]["variants"] = json!(["alamos (tinto)"])),
            "E08",
        ),
        (
            "cpc above cap",
            Box::new(|a| a["default_cpc"] = json!(3.5)),
            "E11",
        ),
        (
            "negative blocks keyword",
            Box::new(|a| a["negatives"] = json!([{"text": "review", "match": "phrase"}])),
            "E09",
        ),
        (
            "avoid term",
            Box::new(|a| a["rsa"]["descriptions"][0] = json!("O melhor do mundo em vinhos")),
            "E05",
        ),
    ];
    for (label, mutate, code) in cases {
        let mut a = ad_group_args("alamos");
        mutate(&mut a);
        let out = t.call("upsert_ad_group", a).await;
        assert!(out.is_error, "{label}");
        assert!(
            error_codes(&out).contains(&code.to_string()),
            "{label}: {:?}",
            error_codes(&out)
        );
        assert!(
            ws.lock().await.account.campaigns[0].ad_groups.is_empty(),
            "{label} mutated the workspace"
        );
    }
}

#[tokio::test]
async fn warnings_are_returned_but_do_not_block() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let mut a = ad_group_args("alamos");
    a["rsa"]["headlines"][0] = json!("Alamos Malbec vale a pena?");
    let out = t.call("upsert_ad_group", a).await;
    assert!(!out.is_error);
    let codes: Vec<&str> = out.content["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"W01"), "{codes:?}");
}

#[tokio::test]
async fn set_campaign_negatives_blocks_existing_keywords() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    t.call("upsert_ad_group", ad_group_args("alamos")).await;
    let bad = t
        .call(
            "set_campaign_negatives",
            json!({"negatives": [{"text": "review", "match": "phrase"}]}),
        )
        .await;
    assert!(error_codes(&bad).contains(&"E09".to_string()));
    assert!(ws.lock().await.account.campaigns[0].negatives.is_empty());
    let ok = t.call("set_campaign_negatives", json!({"negatives": [{"text": "emprego", "match": "phrase"}, {"text": "vaga", "match": "phrase"}]})).await;
    assert!(!ok.is_error, "{}", ok.content);
    assert_eq!(ws.lock().await.account.campaigns[0].negatives.len(), 2);
}

#[tokio::test]
async fn set_assets_validates_urls_and_descriptions() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let mut bad_url = assets_args();
    bad_url["sitelinks"][0]["url"] = json!("https://vinellu.com/inventada");
    assert!(error_codes(&t.call("set_assets", bad_url).await).contains(&"E07".to_string()));
    let mut one_desc = assets_args();
    one_desc["sitelinks"][0]["description1"] = json!("so uma descricao");
    assert!(error_codes(&t.call("set_assets", one_desc).await).contains(&"E14".to_string()));
    assert!(ws.lock().await.account.campaigns[0].assets.is_none());
    let ok = t.call("set_assets", assets_args()).await;
    assert!(!ok.is_error, "{}", ok.content);
    assert_eq!(
        ws.lock().await.account.campaigns[0]
            .assets
            .as_ref()
            .unwrap()
            .sitelinks
            .len(),
        4
    );
}

#[tokio::test]
async fn validate_lists_what_is_missing() {
    let ws = planned().await;
    let out = campaign_tools(&ws, "vinellu-catalogo")
        .await
        .call("validate", json!({}))
        .await;
    assert!(!out.is_error);
    assert!(
        out.content["result"]["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["code"] == "E12")
    );
}

#[tokio::test]
async fn campaign_finish_requires_every_ad_group_and_assets() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    assert!(t.call("finish", json!({})).await.is_error);
    t.call("upsert_ad_group", ad_group_args("alamos")).await;
    t.call("upsert_ad_group", ad_group_args("luigi")).await;
    let no_assets = t.call("finish", json!({})).await;
    assert!(no_assets.is_error, "assets are required");
    assert!(!t.finished());
    t.call("set_assets", assets_args()).await;
    let done = t.call("finish", json!({})).await;
    assert!(!done.is_error, "{}", done.content);
    assert!(t.finished());
}

#[tokio::test]
async fn campaign_tools_cannot_touch_another_campaign() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-marca").await;
    let out = t.call("upsert_ad_group", ad_group_args("alamos")).await;
    assert!(
        error_codes(&out).contains(&"E12".to_string()),
        "alamos belongs to the other campaign"
    );
    assert!(
        ws.lock()
            .await
            .account
            .campaigns
            .iter()
            .all(|c| c.ad_groups.is_empty())
    );
}

#[tokio::test]
async fn mutations_are_persisted_when_a_path_is_given() {
    let ws = planned().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.json");
    let kind = MissionKind::Campaign {
        slug: "vinellu-catalogo".into(),
    };
    let t = MissionTools::new(ws, kind, ToolSettings::default(), Some(path.clone()))
        .await
        .unwrap();
    assert!(
        !t.call("upsert_ad_group", ad_group_args("alamos"))
            .await
            .is_error
    );
    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.account.campaigns[0].ad_groups[0].name, "alamos");
}
