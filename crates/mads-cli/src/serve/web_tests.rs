use std::{fs, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use mads_core::{
    events::{Event, EventEnvelope},
    google::{
        AdGroup, BidStrategy, Campaign, CampaignKind, Cents, Intent, Keyword, MatchType, Rsa,
    },
    input::load_input,
    usage::Usage,
    workspace::{MissionState, MissionStatus, Workspace},
};
use serde_json::json;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::serve::{AppState, Config, app};

const RUN_ID: &str = "20261024-153012-a1b2c3";
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const BUSINESS: &str = r#"
[business]
name = "Acme"
url = "https://acme.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "vender"
description = "Uma descrição com mais de vinte caracteres para o teste."

[budget]
daily = 50
currency = "BRL"
"#;

fn fixture() -> (TempDir, AppState) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("mads");
    let project = root.join("acme");
    let run = project.join("out").join(RUN_ID);
    fs::create_dir_all(run.join("google-ads")).unwrap();
    fs::create_dir_all(run.join("transcripts")).unwrap();
    fs::write(project.join("business.toml"), BUSINESS).unwrap();

    let input = load_input(&project.join("business.toml")).unwrap();
    let mut workspace = Workspace::new(input);
    workspace.account.campaigns.push(Campaign {
        name: "Acme - Marca".into(),
        slug: "acme-marca".into(),
        kind: CampaignKind::Search,
        intent: Intent::Brand,
        daily_budget: Cents(5000),
        bid_strategy: BidStrategy::ManualCpc,
        rationale: "protege a marca".into(),
        planned_ad_groups: vec![mads_core::google::PlannedAdGroup {
            name: "marca".into(),
            theme: "acme".into(),
            entity_ids: Vec::new(),
            final_url: "https://acme.com/".into(),
        }],
        ad_groups: vec![AdGroup {
            name: "marca".into(),
            default_cpc: Cents(150),
            cpc_rationale: "estimativa".into(),
            final_url: "https://acme.com/".into(),
            keywords: vec![Keyword {
                text: "acme".into(),
                match_type: MatchType::Exact,
            }],
            negatives: Vec::new(),
            rsa: Rsa {
                headlines: vec![
                    "Acme".into(),
                    "Acme Oficial".into(),
                    "Compre na Acme".into(),
                ],
                descriptions: vec!["Compre na Acme".into()],
                path1: Some("marca".into()),
                path2: None,
            },
        }],
        asset_groups: Vec::new(),
        negatives: Vec::new(),
        assets: None,
    });
    workspace.missions.insert(
        "plan".into(),
        MissionState {
            status: MissionStatus::Finished,
            attempts: 1,
            usage: Usage::default(),
        },
    );
    workspace.save(&run.join("workspace.json")).unwrap();

    fs::write(
        run.join("run.json"),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "run_id": RUN_ID,
            "provider": "replay",
            "model": "fixture",
            "layout": "bulk",
            "status": "success",
            "exit_code": 0,
            "totals": {
                "input_tokens": 1000,
                "output_tokens": 200,
                "cost_usd": null,
                "missions": 1,
                "failed_missions": 0
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let events = [
        EventEnvelope {
            seq: 1,
            ts: "2026-10-24T15:30:00Z".into(),
            event: Event::RunStarted {
                run_id: RUN_ID.into(),
                run_dir: run.display().to_string(),
                provider: "replay".into(),
                model: Some("fixture".into()),
            },
        },
        EventEnvelope {
            seq: 2,
            ts: "2026-10-24T15:30:01Z".into(),
            event: Event::Validation {
                errors: Vec::new(),
                warnings: Vec::new(),
            },
        },
        EventEnvelope {
            seq: 3,
            ts: "2026-10-24T15:30:02Z".into(),
            event: Event::RunFinished {
                ok: true,
                exit_code: 0,
                totals: mads_core::events::Totals {
                    input_tokens: 1000,
                    output_tokens: 200,
                    cost_usd: None,
                    missions: 1,
                    failed_missions: 0,
                },
            },
        },
    ];
    let ndjson: Vec<String> = events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect();
    fs::write(run.join("events.ndjson"), ndjson.join("\n")).unwrap();
    fs::write(run.join("report.md"), "# Acme\n\nPronto para importar.\n").unwrap();
    fs::write(
        run.join("google-ads/1-campaign.csv"),
        "Campaign,Campaign status\nAcme - Marca,Paused\n",
    )
    .unwrap();
    fs::write(run.join("transcripts/plan.jsonl"), "{}\n").unwrap();

    let mut state = AppState::new(Config {
        root,
        bind: "127.0.0.1".parse().unwrap(),
        port: 4780,
        token: TOKEN.into(),
        allow_remote: false,
        open: false,
        print_token: true,
    });
    state.secrets = Arc::new(crate::serve::secrets::Secrets::at(
        temp.path().join("secrets.toml"),
    ));
    (temp, state)
}

async fn send(state: &AppState, request: Request<Body>) -> Response {
    app(state.clone()).oneshot(request).await.unwrap()
}

async fn body_text(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn get(uri: &str, bearer: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(uri)
        .header(header::HOST, "127.0.0.1:4780");
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn bootstrap_is_served_without_a_session() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/", false)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let text = body_text(response).await;
    assert!(text.contains("Abra o estúdio"));
}

#[tokio::test]
async fn bearer_opens_the_dashboard() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("Projetos"));
    assert!(text.contains("Acme"));
}

#[tokio::test]
async fn session_exchange_sets_an_http_only_cookie() {
    let (_temp, state) = fixture();
    let request = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"token": TOKEN}).to_string()))
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    let session = cookie.split(';').next().unwrap().to_string();

    let request = Request::builder()
        .uri("/")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::COOKIE, session)
        .body(Body::empty())
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_wrong_bootstrap_token_is_rejected() {
    let (_temp, state) = fixture();
    let request = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"token": "b".repeat(64)}).to_string()))
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_projects_api_lists_the_project() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/api/projects", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("\"Acme\""));
}

#[tokio::test]
async fn the_account_page_shows_the_campaign() {
    let (_temp, state) = fixture();
    let response = send(&state, get(&format!("/runs/{RUN_ID}/account"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("Acme - Marca"));
    assert!(text.contains("Compre na Acme"));
}

#[tokio::test]
async fn a_hostile_host_is_rejected() {
    let (_temp, state) = fixture();
    let request = Request::builder()
        .uri("/")
        .header(header::HOST, "evil.example:4780")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_file_download_serves_the_csv() {
    let (_temp, state) = fixture();
    let response = send(
        &state,
        get(
            &format!("/api/runs/{RUN_ID}/files/google-ads/1-campaign.csv"),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("Acme - Marca"));
}

#[tokio::test]
async fn file_traversal_is_rejected() {
    let (_temp, state) = fixture();
    let response = send(
        &state,
        get(
            &format!("/api/runs/{RUN_ID}/files/..%2F..%2F..%2Fetc%2Fpasswd"),
            true,
        ),
    )
    .await;
    assert!(matches!(
        response.status(),
        StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST
    ));
}

#[tokio::test]
async fn the_events_api_replays_the_run_finished_event() {
    let (_temp, state) = fixture();
    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/events"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("run_finished"));
}

#[tokio::test]
async fn a_stale_if_match_is_a_conflict_and_a_fresh_one_passes() {
    let (_temp, state) = fixture();
    let edit = json!({"edits": [{"path": "campaigns[0].rationale", "value": "editado por voce"}]});
    // No header: accepted (the browser always sends it, scripts may not).
    let response = send(
        &state,
        post_json(&format!("/api/runs/{RUN_ID}/revisions"), edit.clone(), true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let (_temp2, state) = fixture();
    let response = send(
        &state,
        request_with_header(
            post_json(&format!("/api/runs/{RUN_ID}/revisions"), edit, true),
            header::IF_MATCH,
            "deadbeef",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let (_temp3, state) = fixture();
    let account_response = send(&state, get(&format!("/api/runs/{RUN_ID}/account"), true)).await;
    let account_text = body_text(account_response).await;
    let account: serde_json::Value = serde_json::from_str(&account_text).unwrap();
    let hash = account["account_hash"].as_str().unwrap().to_string();
    let response = send(
        &state,
        request_with_header(
            post_json(
                &format!("/api/runs/{RUN_ID}/revisions"),
                json!({"edits": [{"path": "campaigns[0].rationale", "value": "editado por todos"}]}),
                true,
            ),
            header::IF_MATCH,
            &hash,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn the_models_endpoint_reports_cli_and_unknown_providers() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/api/providers/claude-cli/models", true)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(response).await.contains("picks its own model"));
    let response = send(&state, get("/api/providers/nope/models", true)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_models_endpoint_reads_the_openai_compat_shape() {
    // A stub provider on loopback answers the same JSON shape OpenAI uses.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stub: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut socket = socket;
                let _ = socket.read(&mut buffer).await;
                let body = "{\"data\":[{\"id\":\"compat-b\"},{\"id\":\"compat-a\"}]}";
                let answer = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(answer.as_bytes()).await;
            });
        }
    });

    let (_temp, state) = fixture();
    // The endpoint only serves providers with a resolved key.
    let response = send(
        &state,
        put_json(
            "/api/secrets/openai-compat",
            json!({"value": "sk-compat"}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = send(
        &state,
        get(
            &format!("/api/providers/openai-compat/models?base_url=http://{addr}"),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(
        payload["models"],
        json!(["compat-a", "compat-b"]),
        "sorted and deduplicated"
    );
    stub.abort();
}

#[tokio::test]
async fn the_run_stream_sends_unnamed_events() {
    let (_temp, state) = fixture();
    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/stream"), true)).await;
    let text = body_text(response).await;
    // `onmessage` only receives unnamed events; the type stays in the payload.
    assert!(text.contains("data:"));
    assert!(
        !text.contains("\nevent:"),
        "named events never reach onmessage: {text}"
    );
}

fn request_with_header(
    mut request: Request<Body>,
    name: header::HeaderName,
    value: &str,
) -> Request<Body> {
    request.headers_mut().insert(name, value.parse().unwrap());
    request
}

#[tokio::test]
async fn the_sse_stream_replays_and_closes() {
    let (_temp, state) = fixture();
    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/stream"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.starts_with("text/event-stream"));
    let text = body_text(response).await;
    assert!(text.contains("run_finished"));
}

fn send_json(method: &str, uri: &str, body: serde_json::Value, bearer: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::CONTENT_TYPE, "application/json");
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn post_json(uri: &str, body: serde_json::Value, bearer: bool) -> Request<Body> {
    send_json("POST", uri, body, bearer)
}

fn put_json(uri: &str, body: serde_json::Value, bearer: bool) -> Request<Body> {
    send_json("PUT", uri, body, bearer)
}

fn plan_script(site: &str) -> serde_json::Value {
    let headlines: Vec<String> = (1..=10).map(|i| format!("Titulo da marca {i}")).collect();
    let descriptions: Vec<String> = (1..=3)
        .map(|i| format!("Descricao da marca com chamada para acao {i}"))
        .collect();
    let calls = json!([
        {"id": "c0", "name": "set_brand_kit", "arguments": {
            "headlines": headlines,
            "descriptions": descriptions
        }},
        {"id": "c1", "name": "set_account_plan", "arguments": {
            "campaigns": [{
                "name": "Acme - Marca",
                "intent": "brand",
                "daily_budget": 50.0,
                "bid_strategy": {"type": "manual_cpc"},
                "rationale": "protege a marca",
                "ad_groups": [{
                    "name": "marca",
                    "theme": "acme",
                    "entity_ids": []
                }]
            }]
        }},
        {"id": "c2", "name": "finish", "arguments": {}}
    ]);
    let _ = site;
    json!({"text": null, "tool_calls": calls, "usage": {"input_tokens": 100, "output_tokens": 20, "cost_usd": null}})
}

fn campaign_script(site: &str) -> serde_json::Value {
    let headlines: Vec<String> = (1..=5).map(|i| format!("Titulo do grupo {i}")).collect();
    let sitelinks: Vec<serde_json::Value> = ["App", "Sobre", "Blog", "Ajuda"]
        .iter()
        .map(|name| json!({"text": name, "url": format!("{site}/")}))
        .collect();
    let callouts: Vec<String> = (1..=4).map(|i| format!("Callout {i}")).collect();
    let values: Vec<String> = (1..=3).map(|i| format!("Tipo {i}")).collect();
    let calls = json!([
        {"id": "d0", "name": "upsert_ad_group", "arguments": {
            "name": "marca",
            "default_cpc": 1.5,
            "cpc_rationale": "estimativa",
            "keywords": {"variants": ["acme"], "modifiers": [], "exact_heads": true},
            "negatives": [],
            "rsa": {
                "headlines": headlines,
                "descriptions": ["Descricao do grupo para teste"],
                "path1": "marca",
                "path2": ""
            }
        }},
        {"id": "d1", "name": "set_assets", "arguments": {
            "sitelinks": sitelinks,
            "callouts": callouts,
            "snippets": [{"header": "types", "values": values}]
        }},
        {"id": "d2", "name": "finish", "arguments": {}}
    ]);
    let _ = site;
    json!({"text": null, "tool_calls": calls, "usage": {"input_tokens": 100, "output_tokens": 20, "cost_usd": null}})
}

#[tokio::test]
async fn a_generate_job_runs_offline_and_writes_a_run() {
    let (temp, state) = fixture();
    let project = temp.path().join("mads/acme");
    let script = temp.path().join("script.json");
    let value = json!({
        "missions": {
            "plan": [plan_script("https://acme.com")],
            "campaign:acme-marca": [campaign_script("https://acme.com")]
        }
    });
    fs::write(&script, serde_json::to_vec(&value).unwrap()).unwrap();

    let request = post_json(
        "/api/projects/acme/runs",
        json!({
            "provider": "replay",
            "script": script.display().to_string(),
            "image_provider": "none",
            "layout": "bulk",
            "skip_url_check": true,
            "plan_review": false,
            "parallel": 1
        }),
        true,
    );
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let text = body_text(response).await;
    let job: serde_json::Value = serde_json::from_str(&text).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    assert_eq!(job["state"], "running");

    let mut finished = None;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = send(&state, get(&format!("/api/jobs/{job_id}"), true)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let text = body_text(response).await;
        let view: serde_json::Value = serde_json::from_str(&text).unwrap();
        let state_name = view["state"].as_str().unwrap_or_default();
        if matches!(state_name, "done" | "failed" | "cancelled") {
            finished = Some(view);
            break;
        }
    }
    let view = finished.expect("the job never finished");
    assert_eq!(view["state"], "done", "{view}");
    assert!(
        view["error_text"].as_str().unwrap_or_default().is_empty(),
        "{view}"
    );

    let run_id = view["run_id_text"].as_str().unwrap();
    let run = project.join("out").join(run_id);
    assert!(
        run.join("google-ads/1-campaign.csv").is_file(),
        "the generated run has no campaign CSV"
    );
    let csv = fs::read_to_string(run.join("google-ads/1-campaign.csv")).unwrap();
    assert!(csv.contains("Acme - Marca"));
    let run_json: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_json["status"], "success");
    assert_eq!(run_json["origin"], "agent");
}

use std::time::Duration;

#[tokio::test]
async fn a_saved_key_is_written_to_the_private_file_and_never_returned() {
    let (temp, state) = fixture();
    let response = send(
        &state,
        put_json(
            "/api/secrets/anthropic",
            json!({"value": "sk-test-123"}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let file = temp.path().join("secrets.toml");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("anthropic"));
    assert!(text.contains("sk-test-123"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let response = send(&state, get("/api/secrets", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(!body.contains("sk-test-123"));
    assert!(body.contains("arquivo 0600"));

    let request = Request::builder()
        .method("DELETE")
        .uri("/api/secrets/anthropic")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!fs::read_to_string(&file).unwrap().contains("sk-test-123"));
}

#[tokio::test]
async fn the_new_project_page_is_served() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/projects/new", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("Novo projeto"));
    assert!(text.contains("Study site"));
}

#[tokio::test]
async fn creating_a_project_rejects_bad_input() {
    let (_temp, state) = fixture();
    let response = send(
        &state,
        post_json(
            "/api/projects",
            json!({
                "name": "",
                "start_url": "not a url",
                "daily": 0,
                "currency": "BR",
                "provider": "anthropic"
            }),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let text = body_text(response).await;
    assert!(text.contains("nome"));
}

#[tokio::test]
async fn optimize_prepare_creates_a_derived_run_with_a_digest() {
    let (_temp, state) = fixture();
    let report = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Custo,Cliques,Impr.\nAtivada,Acme - Marca,\"10,00\",8,90\n";
    let boundary = "mads-test-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"reports\"; filename=\"campaigns.csv\"\r\nContent-Type: text/csv\r\n\r\n{report}\r\n--{boundary}--\r\n"
    );
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/runs/{RUN_ID}/optimize/prepare"))
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let text = body_text(response).await;
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    let run_id = payload["run_id"].as_str().unwrap();
    let review = send(
        &state,
        get(&format!("/runs/{run_id}/optimize/review"), true),
    )
    .await;
    assert_eq!(review.status(), StatusCode::OK);
    let page = body_text(review).await;
    assert!(page.contains("Diagnosis"));
    assert!(page.contains("Acme - Marca"));
    let run_dir = _temp.path().join("mads/acme/out").join(run_id);
    let run_json: serde_json::Value =
        serde_json::from_slice(&fs::read(run_dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_json["origin"], "optimize");
    assert_eq!(run_json["parent_run"], RUN_ID);
    assert_eq!(run_json["status"], "prepared");
}

#[tokio::test]
async fn a_revision_is_created_from_an_account_edit() {
    let (temp, state) = fixture();
    let response = send(
        &state,
        post_json(
            &format!("/api/runs/{RUN_ID}/revisions"),
            json!({"edits": [{"path": "campaigns[0].rationale", "value": "editado por voce"}]}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let text = body_text(response).await;
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    let job_id = payload["job_id"].as_str().unwrap().to_string();
    let run_id = payload["run_id"].as_str().unwrap().to_string();
    assert!(run_id.ends_with("-r1"));

    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = send(&state, get(&format!("/api/jobs/{job_id}"), true)).await;
        let view: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
        if matches!(
            view["state"].as_str().unwrap_or_default(),
            "done" | "failed" | "cancelled"
        ) {
            break;
        }
    }
    let project = temp.path().join("mads/acme");
    let revision = project.join("out").join(&run_id);
    let run_json: serde_json::Value =
        serde_json::from_slice(&fs::read(revision.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_json["origin"], "human");
    assert_eq!(run_json["parent_run"], RUN_ID);
    assert_eq!(run_json["revision"], 1);
    let workspace: serde_json::Value =
        serde_json::from_slice(&fs::read(revision.join("workspace.json")).unwrap()).unwrap();
    assert_eq!(
        workspace["account"]["campaigns"][0]["rationale"],
        "editado por voce"
    );
}

#[tokio::test]
async fn the_draft_page_saves_business_fields() {
    let (temp, state) = fixture();
    let response = send(&state, get("/projects/acme/draft", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("Rascunho"));
    assert!(page.contains("Acme"));

    let response = send(
        &state,
        put_json(
            "/api/projects/acme/draft",
            json!({
                "name": "Acme",
                "url": "https://acme.com",
                "language": "pt-BR",
                "location": "Brazil",
                "goal": "novo objetivo",
                "description": "Uma descrição com mais de vinte caracteres para o teste.",
                "conversion_tracking": false,
                "brand_terms": "",
                "competitors": "",
                "avoid": "",
                "restricted": [],
                "daily": 50.0,
                "currency": "BRL",
                "max_cpc": null,
                "export_status": "Paused",
                "url_suffix": "",
                "eu_political_ads": false,
                "decimal_comma": false,
                "formats": [],
                "catalog_text": ""
            }),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = fs::read_to_string(temp.path().join("mads/acme/business.toml")).unwrap();
    assert!(text.contains("novo objetivo"));
    let reloaded = send(&state, get("/projects/acme/draft", true)).await;
    assert_eq!(reloaded.status(), StatusCode::OK);
    assert!(body_text(reloaded).await.contains("novo objetivo"));
}

#[tokio::test]
async fn the_plan_checkpoint_stops_and_continue_finishes() {
    let (temp, state) = fixture();
    let script = temp.path().join("checkpoint.json");
    let value = json!({
        "missions": {
            "plan": [plan_script("https://acme.com")],
            "campaign:acme-marca": [campaign_script("https://acme.com")]
        }
    });
    fs::write(&script, serde_json::to_vec(&value).unwrap()).unwrap();

    let response = send(
        &state,
        post_json(
            "/api/projects/acme/runs",
            json!({
                "provider": "replay",
                "script": script.display().to_string(),
                "image_provider": "none",
                "layout": "bulk",
                "skip_url_check": true,
                "plan_review": true,
                "parallel": 1
            }),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    let run_id = wait_job(&state, &job_id).await;
    let project = temp.path().join("mads/acme");
    let run = project.join("out").join(&run_id);
    let run_json: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_json["status"], "planned");
    assert_eq!(run_json["provider"], "replay");
    assert!(!run.join("google-ads/1-campaign.csv").is_file());

    let response = send(
        &state,
        post_json(
            &format!("/api/runs/{run_id}/continue"),
            json!({
                "provider": "replay",
                "script": script.display().to_string(),
                "image_provider": "none",
                "layout": "bulk",
                "skip_url_check": true,
                "plan_review": false,
                "parallel": 1
            }),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let continued = wait_job(&state, job["id"].as_str().unwrap()).await;
    assert_eq!(continued, run_id);
    let run_json: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("run.json")).unwrap()).unwrap();
    assert_eq!(run_json["status"], "success");
    assert!(run.join("google-ads/1-campaign.csv").is_file());
}

async fn wait_job(state: &AppState, job_id: &str) -> String {
    for _ in 0..300 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = send(state, get(&format!("/api/jobs/{job_id}"), true)).await;
        let view: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
        let state_name = view["state"].as_str().unwrap_or_default();
        if matches!(state_name, "done" | "failed" | "cancelled") {
            assert_eq!(state_name, "done", "{view}");
            return view["run_id_text"].as_str().unwrap_or_default().to_string();
        }
    }
    panic!("job did not finish");
}

#[tokio::test]
async fn a_job_can_be_cancelled_while_the_model_waits() {
    let (_temp, state) = fixture();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stream.is_err() {
                return;
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    let response = send(
        &state,
        post_json(
            "/api/projects/acme/runs",
            json!({
                "provider": "openai-compat",
                "base_url": format!("http://{addr}/v1"),
                "model": "fake-model",
                "image_provider": "none",
                "layout": "bulk",
                "skip_url_check": true,
                "plan_review": false,
                "parallel": 1
            }),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let response = send(
        &state,
        post_json(&format!("/api/jobs/{job_id}/cancel"), json!({}), true),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let mut cancelled = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let response = send(&state, get(&format!("/api/jobs/{job_id}"), true)).await;
        let view: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
        if view["state"] == "cancelled" {
            cancelled = true;
            break;
        }
        if matches!(view["state"].as_str(), Some("done" | "failed")) {
            break;
        }
    }
    assert!(cancelled, "the job did not report cancellation");
    let run_id = {
        let response = send(&state, get(&format!("/api/jobs/{job_id}"), true)).await;
        let view: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
        view["run_id_text"].as_str().unwrap_or_default().to_string()
    };
    let run_json: serde_json::Value = serde_json::from_slice(
        &fs::read(
            _temp
                .path()
                .join("mads/acme/out")
                .join(run_id)
                .join("run.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(run_json["status"], "cancelled");
}

#[tokio::test]
async fn a_project_without_a_draft_offers_a_retry() {
    let (temp, state) = fixture();
    let dir = temp.path().join("mads/empty");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("project.json"), r#"{"name":"Empty"}"#).unwrap();
    let response = send(&state, get("/projects/empty/draft", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("business.toml is missing"));
    assert!(page.contains("/projects/empty/init"));

    let response = send(&state, get("/projects/empty/init", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("Study the site"));
    assert!(page.contains("data-init-form"));
}

#[tokio::test]
async fn the_language_switch_translates_the_shell() {
    let (_temp, state) = fixture();
    let request = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"token": TOKEN}).to_string()))
        .unwrap();
    let response = send(&state, request).await;
    let session = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let request = Request::builder()
        .uri("/settings/language?lang=en&next=/")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::COOKIE, &session)
        .body(Body::empty())
        .unwrap();
    let response = send(&state, request).await;
    assert!(matches!(
        response.status(),
        StatusCode::SEE_OTHER | StatusCode::TEMPORARY_REDIRECT | StatusCode::FOUND
    ));
    let lang_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let request = Request::builder()
        .uri("/")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::COOKIE, format!("{session}; {lang_cookie}"))
        .body(Body::empty())
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("Projects"));
    assert!(
        !page.contains("English is in beta"),
        "the beta notice is gone"
    );
}

#[tokio::test]
async fn a_run_without_a_draft_redirects_to_init() {
    let (temp, state) = fixture();
    let dir = temp.path().join("mads/empty");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("project.json"), r#"{"name":"Empty"}"#).unwrap();
    let response = send(&state, get("/projects/empty/run", true)).await;
    assert!(matches!(
        response.status(),
        StatusCode::SEE_OTHER | StatusCode::TEMPORARY_REDIRECT | StatusCode::FOUND
    ));
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/projects/empty/init");

    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/empty/runs")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"provider": "replay"}).to_string()))
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let text = body_text(response).await;
    assert!(text.contains("não tem business.toml"));
    assert!(!text.contains("No such file"));
}

#[tokio::test]
async fn a_saved_key_shows_as_ready_in_the_project_forms() {
    let (_temp, state) = fixture();
    let response = send(
        &state,
        put_json(
            "/api/secrets/anthropic",
            json!({"value": "sk-test-ready"}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = send(&state, get("/projects/new", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    // The form lists only the configured providers, by name.
    assert!(page.contains("<option value=\"anthropic\""));
    assert!(!page.contains("missing ANTHROPIC_API_KEY"));
}

#[tokio::test]
async fn the_performance_and_diff_pages_explain_non_optimized_runs() {
    let (_temp, state) = fixture();
    let response = send(&state, get(&format!("/runs/{RUN_ID}/performance"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("performance") || page.contains("otimiza"));

    let response = send(&state, get(&format!("/runs/{RUN_ID}/diff"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("revisão") || page.contains("Alterações"));
}

#[tokio::test]
async fn research_and_design_are_saved_and_reloaded() {
    let (temp, state) = fixture();
    let response = send(
        &state,
        put_json(
            "/api/projects/acme/research",
            json!({"text": "Oportunidades: vinho e review."}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = send(
        &state,
        put_json(
            "/api/projects/acme/design",
            json!({"text": "Cores: vinho e creme."}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = fs::read_to_string(temp.path().join("mads/acme/research.md")).unwrap();
    assert!(text.contains("Oportunidades"));
    let text = fs::read_to_string(temp.path().join("mads/acme/DESIGN.md")).unwrap();
    assert!(text.contains("vinho e creme"));
    let response = send(&state, get("/projects/acme/draft", true)).await;
    let page = body_text(response).await;
    assert!(page.contains("Oportunidades"));
    assert!(page.contains("vinho e creme"));
}

#[tokio::test]
async fn the_json_api_exposes_draft_account_validation_report_and_files() {
    let (_temp, state) = fixture();

    let response = send(&state, get("/api/projects/acme/draft", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("Acme"));

    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/account"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("campaigns"));

    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/validation"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("errors"));

    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/report"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/markdown; charset=utf-8"
    );

    let response = send(&state, get(&format!("/api/runs/{RUN_ID}/files"), true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("files"));

    let response = send(
        &state,
        post_json(
            &format!("/api/runs/{RUN_ID}/account/validate"),
            json!({"edits": [{"path": "campaigns[0].rationale", "value": "editado por voce"}]}),
            true,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("ok"));
}

#[tokio::test]
async fn a_broken_logo_upload_is_rejected() {
    let (_temp, state) = fixture();
    let boundary = "mads-test-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"logo\"; filename=\"logo.png\"\r\nContent-Type: image/png\r\n\r\nnot-a-png\r\n--{boundary}--\r\n"
    );
    let request = Request::builder()
        .method("POST")
        .uri("/api/projects/acme/logo")
        .header(header::HOST, "127.0.0.1:4780")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = send(&state, request).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let response = send(&state, get("/api/projects/acme/logo", true)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_jobs_index_lists_recent_jobs() {
    let (_temp, state) = fixture();
    let response = send(&state, get("/jobs", true)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_text(response).await;
    assert!(page.contains("Nenhum job") || page.contains("Jobs"));
}
