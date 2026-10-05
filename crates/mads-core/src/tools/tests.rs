use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::*;
use crate::google::MatchType;
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
    assert_eq!(
        r["rules"]["campaign_kinds"]["search"]["bid_strategies"],
        json!(["manual_cpc"])
    );
    assert_eq!(r["image_campaigns"]["available"], false);
    assert!(r.get("research").is_none(), "no notes, no key");
}

#[tokio::test]
async fn get_business_carries_the_research_notes() {
    let mut input = testutil::input();
    input.research = "# Research\n\nPeople search labels by name.".into();
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let out = plan_tools(&ws).await.call("get_business", json!({})).await;
    assert_eq!(
        out.content["result"]["research"],
        "# Research\n\nPeople search labels by name."
    );
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
    // The one-word head 'alamos' is exact only: as phrase it would match any search with it.
    assert_eq!(c.ad_groups[0].keywords.len(), 5 + 2);
    assert_eq!(c.ad_groups[0].default_cpc.0, 150);
    assert_eq!(c.ad_groups[0].final_url, "https://vinellu.com/w/alamos");
    assert_eq!(c.ad_groups[0].rsa.path2, None);
    assert!(out.summary.contains("luigi"), "{}", out.summary);
}

#[tokio::test]
async fn one_word_heads_stay_phrase_only_in_brand_campaigns() {
    let ws = planned().await;
    let mut a = ad_group_args("marca");
    a["keywords"]["variants"] = json!(["vinellu"]);
    let out = campaign_tools(&ws, "vinellu-marca")
        .await
        .call("upsert_ad_group", a)
        .await;
    assert!(!out.is_error, "{}", out.content);
    let guard = ws.lock().await;
    let marca = &guard.account.campaigns[1].ad_groups[0];
    assert!(
        marca
            .keywords
            .iter()
            .any(|k| k.text == "vinellu" && k.match_type == MatchType::Phrase)
    );
}

#[tokio::test]
async fn variants_about_different_things_are_refused_e26() {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let mut mixed = ad_group_args("alamos");
    mixed["keywords"]["variants"] = json!(["malbec", "cabernet sauvignon", "merlot"]);
    let out = t.call("upsert_ad_group", mixed).await;
    assert_eq!(error_codes(&out), ["E26"], "{}", out.content);
    assert!(
        out.content.to_string().contains("extra"),
        "says where synonyms go"
    );
    assert!(
        ws.lock().await.account.campaigns[0].ad_groups.is_empty(),
        "a refused call changes nothing"
    );

    let mut spellings = ad_group_args("alamos");
    spellings["keywords"]["variants"] = json!([
        "cabernet sauvignon",
        "Cabernét",
        "cab sauvignon",
        "cs",
        "cabernetsauvignon"
    ]);
    let out = t.call("upsert_ad_group", spellings).await;
    assert!(!out.is_error, "{}", out.content);
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

// ---- image campaigns ----

fn image_input() -> crate::input::Input {
    let mut i = testutil::input();
    i.logo = Some("/tmp/logo.png".into());
    i.business.conversion_tracking = true;
    i.catalog[0].image = Some("https://cdn.vinellu.com/alamos.jpg".into());
    i
}

fn image_settings() -> ToolSettings {
    ToolSettings {
        image_model: true,
        ..ToolSettings::default()
    }
}

fn image_plan_args() -> Value {
    json!({"campaigns": [
        {"name": "Vinellu - Busca", "intent": "catalog", "daily_budget": 30.0,
         "bid_strategy": {"type": "manual_cpc"}, "rationale": "demanda existente",
         "ad_groups": [{"name": "alamos", "theme": "alamos", "entity_ids": ["alamos-malbec"]}]},
        {"name": "Vinellu - PMax", "kind": "performance_max", "intent": "generic", "daily_budget": 20.0,
         "bid_strategy": {"type": "maximize_conversions"}, "rationale": "escala",
         "ad_groups": [{"name": "tintos", "theme": "vinhos tintos", "entity_ids": ["alamos-malbec"]}]}
    ]})
}

async fn image_planned() -> SharedWorkspace {
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(image_input())));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let out = t.call("set_account_plan", image_plan_args()).await;
    assert!(!out.is_error, "{}", out.content);
    ws
}

async fn pmax_tools(ws: &SharedWorkspace) -> MissionTools {
    let kind = MissionKind::Campaign {
        slug: "vinellu-pmax".into(),
    };
    MissionTools::new(ws.clone(), kind, image_settings(), None)
        .await
        .unwrap()
}

fn asset_group_args() -> Value {
    json!({"name": "tintos", "business_name": "Vinellu",
           "headlines": texts("Titulo", 5), "long_headlines": ["Descubra o vinho certo pra cada jantar"],
           "descriptions": ["Reviews reais de vinhos", "Veja safras, notas e harmonizacoes no app"],
           "search_themes": ["vinho tinto"]})
}

fn briefs_args() -> Value {
    let prompt = "Photo of a glass of red wine on a wooden table at dinner, warm light";
    json!({"asset_group": "tintos", "images": [
        {"id": "mesa", "ratio": "landscape", "prompt": prompt, "reference": "alamos-malbec"},
        {"id": "taca", "ratio": "square", "prompt": prompt},
        {"id": "pessoa", "ratio": "portrait", "prompt": prompt}
    ]})
}

#[tokio::test]
async fn plan_accepts_image_kinds_with_a_model_and_a_logo() {
    let ws = image_planned().await;
    let account = ws.lock().await.account.clone();
    assert_eq!(
        account.campaigns[1].kind,
        crate::google::CampaignKind::PerformanceMax
    );
    assert_eq!(
        account.campaigns[1].bid_strategy,
        crate::google::BidStrategy::MaximizeConversions
    );
}

#[tokio::test]
async fn plan_refuses_image_kinds_without_model_or_logo_and_says_why() {
    for (model, logo) in [(false, true), (true, false)] {
        let mut input = image_input();
        if !logo {
            input.logo = None;
        }
        let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
        let settings = ToolSettings {
            image_model: model,
            ..ToolSettings::default()
        };
        let t = MissionTools::new(ws.clone(), MissionKind::Plan, settings, None)
            .await
            .unwrap();
        let business = t.call("get_business", json!({})).await;
        assert_eq!(
            business.content["result"]["image_campaigns"]["available"],
            false
        );
        let out = t.call("set_account_plan", image_plan_args()).await;
        assert!(error_codes(&out).contains(&"UNSUPPORTED".to_string()));
        assert!(ws.lock().await.account.campaigns.is_empty());
    }
}

#[tokio::test]
async fn plan_refuses_a_bid_that_does_not_fit_the_kind() {
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(image_input())));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    let mut p = image_plan_args();
    p["campaigns"][1]["bid_strategy"]["type"] = json!("manual_cpc");
    let out = t.call("set_account_plan", p).await;
    assert!(
        error_codes(&out).contains(&"E17".to_string()),
        "{:?}",
        error_codes(&out)
    );
}

#[tokio::test]
async fn image_campaign_toolset_is_exactly_the_documented_tools() {
    let ws = image_planned().await;
    let names: Vec<String> = pmax_tools(&ws)
        .await
        .specs()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(
        names,
        [
            "get_brief",
            "upsert_asset_group",
            "set_image_briefs",
            "validate",
            "finish"
        ]
    );
    for spec in pmax_tools(&ws).await.specs() {
        let bad = schema::non_portable_keywords(&spec.input_schema);
        assert!(bad.is_empty(), "{}: {bad:?}", spec.name);
    }
}

#[tokio::test]
async fn image_campaign_flow_builds_texts_and_briefs_then_finishes() {
    let ws = image_planned().await;
    let t = pmax_tools(&ws).await;
    let brief = t.call("get_brief", json!({})).await;
    assert_eq!(brief.content["result"]["entities"][0]["has_photo"], true);
    let out = t.call("finish", json!({})).await;
    assert!(out.is_error, "finish before building");
    let out = t.call("upsert_asset_group", asset_group_args()).await;
    assert!(!out.is_error, "{}", out.content);
    let out = t.call("set_image_briefs", briefs_args()).await;
    assert!(!out.is_error, "{}", out.content);
    let out = t.call("finish", json!({})).await;
    assert!(!out.is_error, "{}", out.content);
    assert!(t.finished());
    let c = ws.lock().await.account.campaigns[1].clone();
    assert_eq!(c.asset_groups[0].images.len(), 3);
    assert_eq!(
        c.asset_groups[0].images[0].reference.as_deref(),
        Some("alamos-malbec")
    );
    assert_eq!(c.asset_groups[0].images[1].reference, None);
}

#[tokio::test]
async fn upsert_asset_group_keeps_the_briefs() {
    let ws = image_planned().await;
    let t = pmax_tools(&ws).await;
    t.call("upsert_asset_group", asset_group_args()).await;
    t.call("set_image_briefs", briefs_args()).await;
    let out = t.call("upsert_asset_group", asset_group_args()).await;
    assert!(!out.is_error);
    assert_eq!(
        ws.lock().await.account.campaigns[1].asset_groups[0]
            .images
            .len(),
        3
    );
}

#[tokio::test]
async fn image_tool_errors_do_not_mutate() {
    let ws = image_planned().await;
    let t = pmax_tools(&ws).await;
    let out = t.call("set_image_briefs", briefs_args()).await;
    assert_eq!(error_codes(&out), ["E12"], "briefs before the asset group");
    t.call("upsert_asset_group", asset_group_args()).await;
    let before = ws.lock().await.account.clone();
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "not planned",
            json!({"name": "outro", "business_name": "V", "headlines": texts("T", 3), "descriptions": texts("D", 2)}),
            "E12",
        ),
        (
            "few headlines",
            json!({"name": "tintos", "business_name": "V", "headlines": texts("T", 2), "long_headlines": ["L"], "descriptions": texts("D", 2)}),
            "E13",
        ),
    ];
    for (label, args, code) in cases {
        let out = t.call("upsert_asset_group", args).await;
        assert!(
            error_codes(&out).contains(&code.to_string()),
            "{label}: {:?}",
            error_codes(&out)
        );
    }
    let mut bad = briefs_args();
    bad["images"][0]["reference"] = json!("luigi-bosca");
    assert!(error_codes(&t.call("set_image_briefs", bad).await).contains(&"E19".to_string()));
    let mut bad = briefs_args();
    bad["images"] = json!([bad["images"][0].clone()]);
    assert!(error_codes(&t.call("set_image_briefs", bad).await).contains(&"E16".to_string()));
    assert_eq!(ws.lock().await.account, before);
}

#[tokio::test]
async fn required_formats_reach_the_agent_and_a_plan_without_them_is_refused() {
    let mut input = image_input();
    input.formats = vec![
        crate::google::CampaignKind::Search,
        crate::google::CampaignKind::DemandGen,
    ];
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    let business = t.call("get_business", json!({})).await;
    assert_eq!(
        business.content["result"]["required_formats"],
        json!(["search", "demand_gen"])
    );
    let out = t.call("set_account_plan", image_plan_args()).await;
    assert_eq!(error_codes(&out), ["E21"], "pmax is not demand_gen");
    assert!(ws.lock().await.account.campaigns.is_empty());
    let mut p = image_plan_args();
    p["campaigns"][1]["kind"] = json!("demand_gen");
    p["campaigns"][1]["bid_strategy"]["type"] = json!("maximize_clicks");
    let out = t.call("set_account_plan", p).await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn with_a_focus_every_landing_page_is_a_focus_page() {
    let mut input = testutil::input();
    input.focus = Some(crate::input::Focus {
        name: "Alamos".into(),
        urls: vec!["https://vinellu.com/w/alamos".into()],
        terms: vec![],
    });
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = plan_tools(&ws).await;
    let business = t.call("get_business", json!({})).await;
    assert_eq!(business.content["result"]["focus"]["name"], "Alamos");
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let out = t.call("set_account_plan", plan_args()).await;
    let codes = error_codes(&out);
    assert_eq!(codes, ["E22"], "luigi lands outside the focus: {codes:?}");
    assert!(ws.lock().await.account.campaigns.is_empty());

    let mut p = plan_args();
    p["campaigns"][0]["ad_groups"] =
        json!([{"name": "alamos", "theme": "alamos", "entity_ids": ["alamos-malbec"]}]);
    let out = t.call("set_account_plan", p).await;
    assert!(!out.is_error, "{}", out.content);
    let plan = ws.lock().await.account.campaigns.clone();
    assert_eq!(
        plan[1].planned_ad_groups[0].final_url, "https://vinellu.com/w/alamos",
        "a group without entity lands on the focus page, not the home page"
    );
}

#[tokio::test]
async fn with_focus_terms_every_keyword_is_about_the_focus() {
    let mut input = testutil::input();
    input.focus = Some(crate::input::Focus {
        name: "Alamos".into(),
        urls: vec!["https://vinellu.com/w/alamos".into()],
        terms: vec![vec!["alamos".into(), "álamos".into()]],
    });
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = plan_tools(&ws).await;
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let mut p = plan_args();
    p["campaigns"][0]["ad_groups"] =
        json!([{"name": "alamos", "theme": "alamos", "entity_ids": ["alamos-malbec"]}]);
    assert!(!t.call("set_account_plan", p).await.is_error);
    let c = campaign_tools(&ws, "vinellu-catalogo").await;

    let mut bare = ad_group_args("alamos");
    bare["keywords"]["variants"] = json!(["alamos malbec", "malbec"]);
    let out = c.call("upsert_ad_group", bare).await;
    assert!(
        error_codes(&out).contains(&"E23".to_string()),
        "{:?}",
        error_codes(&out)
    );
    let msgs = out.content["errors"].to_string();
    assert!(msgs.contains("'malbec' is not about the focus"), "{msgs}");

    let mut ok = ad_group_args("alamos");
    ok["keywords"]["variants"] = json!(["alamos malbec", "Álamos"]);
    let out = c.call("upsert_ad_group", ok).await;
    assert!(!out.is_error, "accents fold: {}", out.content);
}

#[tokio::test]
async fn with_images_available_a_search_only_plan_must_say_why() {
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(image_input())));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let out = t.call("set_account_plan", plan_args()).await;
    assert_eq!(error_codes(&out), ["NO_IMAGE_REASON"]);
    assert!(ws.lock().await.account.campaigns.is_empty());

    let mut p = plan_args();
    p["campaigns"][0]["rationale"] =
        json!("No image campaign: people search label names, pictures add nothing.");
    let out = t.call("set_account_plan", p).await;
    assert!(!out.is_error, "{}", out.content);

    let no_model = ToolSettings::default();
    let ws2: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(image_input())));
    let t2 = MissionTools::new(ws2, MissionKind::Plan, no_model, None)
        .await
        .unwrap();
    t2.call("set_brand_kit", brand_kit_args()).await;
    assert!(
        !t2.call("set_account_plan", plan_args()).await.is_error,
        "no images, no reason needed"
    );
}

#[tokio::test]
async fn an_app_campaign_lands_on_the_store_and_needs_no_logo() {
    let mut input = image_input();
    input.logo = None;
    input.app = Some(crate::input::App {
        store: crate::input::AppStore::GooglePlay,
        id: "com.vinellu.app".into(),
    });
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    let business = t.call("get_business", json!({})).await;
    assert_eq!(
        business.content["result"]["app_campaigns"]["available"],
        true
    );
    assert_eq!(
        business.content["result"]["image_campaigns"]["available"], false,
        "no logo"
    );
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let mut p = image_plan_args();
    p["campaigns"][1] = json!({"name": "Vinellu - App", "kind": "app_installs", "intent": "generic", "daily_budget": 20.0,
        "bid_strategy": {"type": "maximize_conversions"}, "rationale": "instalacoes",
        "ad_groups": [{"name": "app", "theme": "app de vinhos"}]});
    let out = t.call("set_account_plan", p).await;
    assert!(!out.is_error, "{}", out.content);
    let planned = ws.lock().await.account.campaigns[1].planned_ad_groups[0]
        .final_url
        .clone();
    assert_eq!(
        planned,
        "https://play.google.com/store/apps/details?id=com.vinellu.app"
    );

    let kind = MissionKind::Campaign {
        slug: "vinellu-app".into(),
    };
    let c = MissionTools::new(ws.clone(), kind, image_settings(), None)
        .await
        .unwrap();
    let group = json!({"name": "app", "headlines": texts("Titulo", 3), "descriptions": ["Descubra vinhos no app"]});
    assert!(!c.call("upsert_asset_group", group).await.is_error);
    let prompt = "Photo of friends at dinner looking at a phone, warm light";
    let briefs = json!({"asset_group": "app", "images": [
        {"id": "jantar", "ratio": "landscape", "prompt": prompt},
        {"id": "amigos", "ratio": "portrait", "prompt": prompt}]});
    let out = c.call("set_image_briefs", briefs).await;
    assert!(!out.is_error, "{}", out.content);
    let out = c.call("finish", json!({})).await;
    assert!(!out.is_error, "{}", out.content);
}

fn app_input() -> crate::input::Input {
    let mut input = image_input();
    input.app = Some(crate::input::App {
        store: crate::input::AppStore::GooglePlay,
        id: "com.vinellu.app".into(),
    });
    input
}

#[tokio::test]
async fn with_the_app_available_a_plan_without_an_app_campaign_must_say_why() {
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(app_input())));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    assert!(!t.call("set_brand_kit", brand_kit_args()).await.is_error);
    let out = t.call("set_account_plan", image_plan_args()).await;
    assert_eq!(
        error_codes(&out),
        ["NO_APP_REASON"],
        "a PMax that sells the app is not enough"
    );
    assert!(ws.lock().await.account.campaigns.is_empty());
    let mut p = image_plan_args();
    p["campaigns"][0]["rationale"] = json!("No app campaign: the goal is sales on the site.");
    assert!(!t.call("set_account_plan", p).await.is_error);
}

#[tokio::test]
async fn required_formats_decide_and_no_reason_is_asked() {
    let mut input = app_input();
    input.formats = vec![crate::google::CampaignKind::Search];
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    t.call("set_brand_kit", brand_kit_args()).await;
    let out = t.call("set_account_plan", plan_args()).await;
    assert!(!out.is_error, "search only, as required: {}", out.content);
}

#[tokio::test]
async fn an_app_campaign_plans_without_conversion_tracking() {
    // The Vinellu run that stopped: tracking off, formats search + demand_gen + app_installs.
    let mut input = app_input();
    input.business.conversion_tracking = false;
    input.formats = vec![
        crate::google::CampaignKind::Search,
        crate::google::CampaignKind::DemandGen,
        crate::google::CampaignKind::AppInstalls,
    ];
    let ws: SharedWorkspace = Arc::new(Mutex::new(Workspace::new(input)));
    let t = MissionTools::new(ws.clone(), MissionKind::Plan, image_settings(), None)
        .await
        .unwrap();
    t.call("set_brand_kit", brand_kit_args()).await;
    let plan = json!({"campaigns": [
        {"name": "Busca", "intent": "catalog", "daily_budget": 20.0, "bid_strategy": {"type": "manual_cpc"}, "rationale": "r",
         "ad_groups": [{"name": "alamos", "theme": "alamos", "entity_ids": ["alamos-malbec"]}]},
        {"name": "Feed", "kind": "demand_gen", "intent": "generic", "daily_budget": 15.0, "bid_strategy": {"type": "maximize_clicks"}, "rationale": "r",
         "ad_groups": [{"name": "tintos", "theme": "tintos"}]},
        {"name": "App", "kind": "app_installs", "intent": "generic", "daily_budget": 15.0, "bid_strategy": {"type": "maximize_conversions"}, "rationale": "r",
         "ad_groups": [{"name": "app", "theme": "app"}]}
    ]});
    let out = t.call("set_account_plan", plan).await;
    assert!(!out.is_error, "{}", out.content);
}

// ---- optimize: live account and performance ----

async fn live_planned() -> SharedWorkspace {
    let ws = planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    assert!(
        !t.call("upsert_ad_group", ad_group_args("alamos"))
            .await
            .is_error
    );
    {
        let mut guard = ws.lock().await;
        let baseline = guard.account.clone();
        let files = vec![(
            "k.csv".to_string(),
            crate::perf::read_table(
                "Palavra-chave,Tipo de corresp.,Campanha,Grupo de anúncios,Motivos do status,Cliques,Impr.,Custo\n\"\"\"alamos\"\"\",Correspondência de frase,Vinellu - Catalogo,alamos,abaixo do lance de primeira página,9,120,\"9,90\"\n\"\"\"alamos safra\"\"\",Correspondência de frase,Vinellu - Catalogo,alamos,,0,0,\"0,00\"\n"
                    .as_bytes(),
            ),
        )];
        let performance = crate::perf::digest(&files, &baseline);
        guard.live = Some(crate::perf::Live {
            baseline,
            performance,
        });
    }
    ws
}

#[tokio::test]
async fn get_business_shows_the_live_account_and_its_numbers() {
    let ws = live_planned().await;
    let out = plan_tools(&ws).await.call("get_business", json!({})).await;
    let r = &out.content["result"];
    assert_eq!(
        r["live_account"]["campaigns"][0]["name"],
        "Vinellu - Catalogo"
    );
    assert_eq!(r["live_account"]["campaigns"][0]["ad_groups"][0], "alamos");
    let c = &r["performance"]["campaigns"][0];
    assert_eq!(c["name"], "Vinellu - Catalogo");
    assert_eq!(c["metrics"]["clicks"], 9);
    assert_eq!(c["thin"], true);
    assert!(
        r["performance"]["thin_rule"]
            .as_str()
            .unwrap()
            .contains("structure")
    );
    assert!(
        c.get("ad_groups").is_none(),
        "the plan sees campaign totals only"
    );
}

#[tokio::test]
async fn get_brief_shows_the_live_campaign_and_its_keywords() {
    let ws = live_planned().await;
    let out = campaign_tools(&ws, "vinellu-catalogo")
        .await
        .call("get_brief", json!({}))
        .await;
    let r = &out.content["result"];
    assert_eq!(r["live"]["ad_groups"][0]["name"], "alamos");
    assert_eq!(r["live"]["ad_groups"][0]["default_cpc"], 1.5);
    let g = &r["performance"]["ad_groups"][0];
    assert_eq!(g["keywords"], 2);
    assert_eq!(g["signals"]["below_first_page"], 1);
    assert!(
        r["live_detail"]
            .as_str()
            .unwrap()
            .contains("get_ad_group_performance")
    );

    let brand = campaign_tools(&ws, "vinellu-marca")
        .await
        .call("get_brief", json!({}))
        .await;
    assert!(
        brand.content["result"]["live"].is_null()
            || brand.content["result"]["live"]["ad_groups"]
                .as_array()
                .is_some_and(|a| a.is_empty())
    );
}

#[tokio::test]
async fn without_live_data_the_tools_say_nothing_about_it() {
    let ws = planned().await;
    let out = plan_tools(&ws).await.call("get_business", json!({})).await;
    assert!(out.content["result"].get("performance").is_none());
    let out = campaign_tools(&ws, "vinellu-catalogo")
        .await
        .call("get_brief", json!({}))
        .await;
    assert!(out.content["result"].get("live").is_none());
}

#[tokio::test]
async fn image_campaign_brief_shows_its_live_asset_groups_and_labels() {
    let ws = image_planned().await;
    let t = pmax_tools(&ws).await;
    let out = t.call("upsert_asset_group", asset_group_args()).await;
    assert!(!out.is_error, "{}", out.content);
    {
        let mut guard = ws.lock().await;
        let baseline = guard.account.clone();
        let name = baseline
            .campaigns
            .iter()
            .find(|c| c.slug == "vinellu-pmax")
            .map(|c| c.name.clone())
            .unwrap();
        let csv = format!(
            "Recurso,Tipo de recurso,Campanha,Grupo de anúncios,Classificação de desempenho,Impr.\nTitulo numero 1,Título,{name},tintos,Baixo,90\n"
        );
        let files = vec![("a.csv".to_string(), crate::perf::read_table(csv.as_bytes()))];
        let performance = crate::perf::digest(&files, &baseline);
        guard.live = Some(crate::perf::Live {
            baseline,
            performance,
        });
    }
    // Tools are built per mission attempt, after the workspace has its live data.
    let t = pmax_tools(&ws).await;
    let r = &t.call("get_brief", json!({})).await.content["result"];
    assert_eq!(r["live"]["asset_groups"][0], "tintos");
    let r = &t
        .call("get_ad_group_performance", json!({"ad_group": "tintos"}))
        .await
        .content["result"];
    assert_eq!(r["live"]["name"], "tintos");
    assert_eq!(r["performance"]["assets"][0]["label"], "Baixo");
}

fn tool_names(t: &MissionTools) -> Vec<String> {
    t.specs().into_iter().map(|s| s.name).collect()
}

#[tokio::test]
async fn ad_group_performance_is_a_tool_only_with_live_data() {
    let plain = planned().await;
    let names = tool_names(&campaign_tools(&plain, "vinellu-catalogo").await);
    assert!(!names.contains(&"get_ad_group_performance".to_string()));
    let ws = live_planned().await;
    let names = tool_names(&campaign_tools(&ws, "vinellu-catalogo").await);
    assert!(names.contains(&"get_ad_group_performance".to_string()));
    let names = tool_names(&plan_tools(&ws).await);
    assert!(
        !names.contains(&"get_ad_group_performance".to_string()),
        "campaign missions only"
    );
}

#[tokio::test]
async fn ad_group_performance_returns_one_group_in_detail() {
    let ws = live_planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let before = ws.lock().await.account.clone();
    let out = t
        .call("get_ad_group_performance", json!({"ad_group": "ALAMOS"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let r = &out.content["result"];
    assert_eq!(r["live"]["default_cpc"], 1.5);
    assert_eq!(
        r["performance"]["keywords"][0]["signals"][0],
        "below_first_page"
    );

    let out = t
        .call("get_ad_group_performance", json!({"ad_group": "luigi"}))
        .await;
    assert_eq!(error_codes(&out), ["NOT_FOUND"]);
    assert!(
        out.content.to_string().contains("alamos"),
        "lists the groups it knows"
    );
    let bad = t
        .call("get_ad_group_performance", json!({"group": "alamos"}))
        .await;
    assert_eq!(error_codes(&bad), ["ARGS"]);
    assert_eq!(ws.lock().await.account, before, "read only");
}

#[tokio::test]
async fn a_group_that_ran_is_rebuilt_only_after_its_detail() {
    let ws = live_planned().await;
    let t = campaign_tools(&ws, "vinellu-catalogo").await;
    let before = ws.lock().await.account.clone();
    let out = t.call("upsert_ad_group", ad_group_args("alamos")).await;
    assert_eq!(error_codes(&out), ["LIVE_DETAIL"], "{}", out.content);
    assert!(out.content.to_string().contains("get_ad_group_performance"));
    assert_eq!(
        ws.lock().await.account,
        before,
        "a refused call changes nothing"
    );

    let fresh = t.call("upsert_ad_group", ad_group_args("luigi")).await;
    assert!(
        !fresh.is_error,
        "a group that did not run needs no detail: {}",
        fresh.content
    );

    let detail = t
        .call("get_ad_group_performance", json!({"ad_group": "Alamos"}))
        .await;
    assert!(!detail.is_error);
    let out = t.call("upsert_ad_group", ad_group_args("alamos")).await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn an_asset_group_that_ran_is_rebuilt_only_after_its_detail() {
    let ws = image_planned().await;
    assert!(
        !pmax_tools(&ws)
            .await
            .call("upsert_asset_group", asset_group_args())
            .await
            .is_error
    );
    {
        let mut guard = ws.lock().await;
        let baseline = guard.account.clone();
        guard.live = Some(crate::perf::Live {
            baseline,
            performance: Default::default(),
        });
    }
    let t = pmax_tools(&ws).await;
    let out = t.call("upsert_asset_group", asset_group_args()).await;
    assert_eq!(error_codes(&out), ["LIVE_DETAIL"], "{}", out.content);
    t.call("get_ad_group_performance", json!({"ad_group": "tintos"}))
        .await;
    let out = t.call("upsert_asset_group", asset_group_args()).await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn a_group_with_impressions_is_not_dropped_without_a_reason_e25() {
    let ws = live_planned().await;
    let t = plan_tools(&ws).await;
    let mut renamed = plan_args();
    renamed["campaigns"][0]["ad_groups"][0]["name"] = json!("alamos novo");
    let before = ws.lock().await.account.clone();
    let out = t.call("set_account_plan", renamed.clone()).await;
    assert_eq!(error_codes(&out), ["E25"], "{}", out.content);
    assert!(out.content.to_string().contains("Drop alamos:"));
    assert_eq!(
        ws.lock().await.account,
        before,
        "a refused call changes nothing"
    );

    renamed["campaigns"][0]["rationale"] =
        json!("intencao alta\nDrop alamos: misturava dois temas, virou alamos novo");
    let out = t.call("set_account_plan", renamed).await;
    assert!(!out.is_error, "{}", out.content);

    let out = t.call("set_account_plan", plan_args()).await;
    assert!(
        !out.is_error,
        "keeping the name needs no reason: {}",
        out.content
    );
}
