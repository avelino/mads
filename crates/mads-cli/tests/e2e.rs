#![allow(clippy::unwrap_used)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use serde_json::{Value, json};

const SITE: &str = "https://vinellu.com";

fn texts(prefix: &str, n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("{prefix} numero {i}")).collect()
}

fn resp(calls: Vec<(&str, Value)>) -> Value {
    let tool_calls: Vec<Value> = calls
        .into_iter()
        .enumerate()
        .map(|(i, (n, a))| json!({"id": format!("c{i}"), "name": n, "arguments": a}))
        .collect();
    json!({"text": null, "tool_calls": tool_calls, "usage": {"input_tokens": 100, "output_tokens": 20, "cost_usd": null}})
}

fn ad_group(name: &str, variants: &[&str]) -> Value {
    json!({"name": name, "default_cpc": 1.5, "cpc_rationale": "estimativa",
           "keywords": {"variants": variants, "modifiers": ["review"], "exact_heads": true},
           "negatives": [{"text": "emprego", "match": "phrase"}],
           "rsa": {"headlines": texts("Titulo do grupo", 5), "descriptions": ["Descricao do grupo para teste"], "path1": "vinhos", "path2": ""}})
}

fn assets(site: &str) -> Value {
    let sitelinks: Vec<Value> = ["app", "sobre", "blog", "ajuda"]
        .iter()
        .map(|p| json!({"text": format!("Link {p}"), "url": format!("{site}/{p}")}))
        .collect();
    json!({"sitelinks": sitelinks, "callouts": texts("Callout", 4), "snippets": [{"header": "types", "values": texts("Tipo", 3)}]})
}

fn plan(site: &str) -> Value {
    let _ = site;
    let kit = json!({"headlines": texts("Titulo da marca", 10), "descriptions": texts("Descricao da marca com chamada pra acao", 3)});
    let plan = json!({"campaigns": [
        {"name": "Vinellu - Catalogo", "intent": "catalog", "daily_budget": 30.0, "bid_strategy": {"type": "manual_cpc"}, "rationale": "intencao alta",
         "ad_groups": [{"name": "alamos", "theme": "alamos", "entity_ids": ["alamos-malbec"]}, {"name": "luigi", "theme": "luigi", "entity_ids": ["luigi-bosca"]}]},
        {"name": "Vinellu - Marca", "intent": "brand", "daily_budget": 20.0, "bid_strategy": {"type": "manual_cpc"}, "rationale": "protege marca",
         "ad_groups": [{"name": "marca", "theme": "vinellu", "entity_ids": []}]}
    ]});
    resp(vec![
        ("set_brand_kit", kit),
        ("set_account_plan", plan),
        ("finish", json!({})),
    ])
}

fn catalog_campaign(site: &str) -> Value {
    resp(vec![
        (
            "upsert_ad_group",
            ad_group("alamos", &["alamos malbec", "alamos"]),
        ),
        ("upsert_ad_group", ad_group("luigi", &["luigi bosca"])),
        ("set_assets", assets(site)),
        ("finish", json!({})),
    ])
}

fn brand_campaign(site: &str) -> Value {
    resp(vec![
        (
            "upsert_ad_group",
            ad_group("marca", &["vinellu", "vinellu app"]),
        ),
        ("set_assets", assets(site)),
        ("finish", json!({})),
    ])
}

struct Project {
    dir: tempfile::TempDir,
    site: String,
}

impl Project {
    fn new(site: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!(
            r#"[business]
name = "Vinellu"
url = "{site}"
language = "pt-BR"
locations = ["Brazil"]
goal = "cadastros no app"
description = "App social de vinhos com reviews, safras e harmonização."
competitors = ["Vivino"]
avoid = ["melhor do mundo"]

[[business.pages]]
name = "app"
url = "{site}/app"
[[business.pages]]
name = "sobre"
url = "{site}/sobre"
[[business.pages]]
name = "blog"
url = "{site}/blog"
[[business.pages]]
name = "ajuda"
url = "{site}/ajuda"

[budget]
daily = 50
currency = "BRL"
max_cpc = 3.0

[catalog]
file = "catalog.csv"
"#
        );
        fs::write(dir.path().join("business.toml"), toml).unwrap();
        let csv = format!(
            "name,url,category,aliases,third_party\nAlamos Malbec,{site}/w/alamos,malbec,alamos,true\nLuigi Bosca,{site}/w/luigi,malbec,,true\n"
        );
        fs::write(dir.path().join("catalog.csv"), csv).unwrap();
        Self {
            dir,
            site: site.into(),
        }
    }

    fn script(&self, name: &str, missions: Value) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, json!({"missions": missions}).to_string()).unwrap();
        path
    }

    fn full_script(&self) -> PathBuf {
        self.script("script.json", json!({"plan": [plan(&self.site)], "campaign:vinellu-catalogo": [catalog_campaign(&self.site)], "campaign:vinellu-marca": [brand_campaign(&self.site)]}))
    }

    fn mads(&self) -> Command {
        let mut c = Command::cargo_bin("mads").unwrap();
        c.current_dir(self.dir.path())
            .env_remove("GITHUB_ACTIONS")
            .env_remove("MADS_PROVIDER")
            .env_remove("MADS_FORMAT")
            .env_remove("MADS_IMAGE_PROVIDER")
            .env_remove("MADS_IMAGE_MODEL")
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY");
        c
    }

    fn generate(&self, script: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
        self.mads()
            .args([
                "generate",
                "business.toml",
                "--provider",
                "replay",
                "--script",
            ])
            .arg(script)
            .args(["--out", "out", "--mission-retries", "0"])
            .args(extra)
            .assert()
    }

    fn run_dir(&self) -> PathBuf {
        let mut dirs: Vec<PathBuf> = fs::read_dir(self.dir.path().join("out"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        dirs.sort();
        dirs.pop().expect("a run directory")
    }
}

#[test]
fn help_lists_the_commands() {
    let out = Command::cargo_bin("mads")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for cmd in ["init", "generate", "export", "providers"] {
        assert!(text.contains(cmd), "{cmd} missing in help");
    }
}

#[test]
fn providers_lists_the_documented_names() {
    let out = Command::cargo_bin("mads")
        .unwrap()
        .arg("providers")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for name in [
        "anthropic",
        "openai",
        "claude-cli",
        "codex-cli",
        "gemini-cli",
        "ollama",
    ] {
        assert!(text.contains(name), "{name} missing:\n{text}");
    }
    assert!(!text.contains("replay"), "replay is hidden");
}

#[test]
fn generate_with_json_format_streams_valid_events_and_writes_the_run_dir() {
    let p = Project::new(SITE);
    let out = p
        .generate(&p.full_script(), &["--format", "json", "--skip-url-check"])
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let events: Vec<Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect();
    assert_eq!(events.first().unwrap()["event"]["type"], "run_started");
    let last = &events.last().unwrap()["event"];
    assert_eq!(
        (last["type"].as_str(), last["exit_code"].as_i64()),
        (Some("run_finished"), Some(0))
    );
    let run = p.run_dir();
    for f in [
        "1-campaign.csv",
        "2-ad-groups.csv",
        "3-keywords.csv",
        "4-negative-keywords.csv",
        "5-responsive-search-ads.csv",
    ] {
        assert!(run.join("google-ads").join(f).exists(), "{f}");
    }
    for f in [
        "report.md",
        "workspace.json",
        "run.json",
        "events.ndjson",
        "input/business.toml",
        "input/catalog.csv",
    ] {
        assert!(run.join(f).exists(), "{f}");
    }
    let ndjson = fs::read_to_string(run.join("events.ndjson")).unwrap();
    assert_eq!(
        ndjson.lines().count(),
        events.len(),
        "events.ndjson has every event"
    );
}

#[test]
fn generate_in_plain_format_prints_mission_lines() {
    let p = Project::new(SITE);
    let out = p
        .generate(&p.full_script(), &["--format", "plain", "--skip-url-check"])
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(
        text.contains("[plan] > set_brand_kit")
            && text.contains("[campaign:vinellu-marca]")
            && text.contains("[run] done (exit 0)"),
        "{text}"
    );
}

#[test]
fn github_actions_env_selects_the_github_format() {
    let p = Project::new(SITE);
    let summary = p.dir.path().join("summary.md");
    let out = p
        .mads()
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_STEP_SUMMARY", &summary)
        .args([
            "generate",
            "business.toml",
            "--provider",
            "replay",
            "--script",
        ])
        .arg(p.full_script())
        .args(["--out", "out", "--skip-url-check"])
        .assert()
        .success();
    assert!(String::from_utf8_lossy(&out.get_output().stdout).contains("[run] done"));
    let md = fs::read_to_string(summary).unwrap();
    assert!(
        md.contains("# mads report: Vinellu"),
        "report goes to the step summary"
    );
}

#[test]
fn a_missing_business_file_exits_2() {
    let p = Project::new(SITE);
    fs::remove_file(p.dir.path().join("business.toml")).unwrap();
    p.generate(&p.full_script(), &[])
        .code(2)
        .stderr(predicates::str::contains("business.toml"));
}

#[test]
fn an_invalid_business_file_exits_2_and_names_the_key() {
    let p = Project::new(SITE);
    let toml = fs::read_to_string(p.dir.path().join("business.toml"))
        .unwrap()
        .replace("daily = 50", "daily = 50.123");
    fs::write(p.dir.path().join("business.toml"), toml).unwrap();
    p.generate(&p.full_script(), &[])
        .code(2)
        .stderr(predicates::str::contains("budget.daily"));
}

#[test]
fn missing_and_unknown_providers_exit_2() {
    let p = Project::new(SITE);
    p.mads()
        .args(["generate", "business.toml"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--provider"));
    p.mads()
        .args(["generate", "business.toml", "--provider", "gpt-9000"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("mads providers"));
    assert!(
        !p.dir.path().join("out").exists(),
        "a failed start must not leave a run directory behind"
    );
}

#[test]
fn unreachable_urls_exit_3_and_export_can_skip_the_check() {
    let p = Project::new("http://127.0.0.1:1");
    p.generate(&p.full_script(), &["--format", "plain"]).code(3);
    let run = p.run_dir();
    assert!(!run.join("google-ads/1-campaign.csv").exists());
    assert!(
        fs::read_to_string(run.join("report.md"))
            .unwrap()
            .contains("E15")
    );
    p.mads()
        .arg("export")
        .arg(&run)
        .arg("--skip-url-check")
        .assert()
        .success();
    assert!(run.join("google-ads/1-campaign.csv").exists());
}

#[test]
fn a_failed_mission_exits_1_and_resume_finishes_the_run() {
    let p = Project::new(SITE);
    let partial = p.script(
        "partial.json",
        json!({"plan": [plan(SITE)], "campaign:vinellu-catalogo": [catalog_campaign(SITE)]}),
    );
    p.generate(&partial, &["--skip-url-check"]).code(1);
    let run = p.run_dir();
    let rest = p.script(
        "rest.json",
        json!({"campaign:vinellu-marca": [brand_campaign(SITE)]}),
    );
    p.mads()
        .args(["generate", "--resume"])
        .arg(&run)
        .args(["--provider", "replay", "--script"])
        .arg(rest)
        .arg("--skip-url-check")
        .assert()
        .success();
    assert!(run.join("google-ads/1-campaign.csv").exists());
}

#[test]
fn export_on_a_missing_run_dir_fails_with_code_1() {
    Command::cargo_bin("mads")
        .unwrap()
        .args(["export", "/nonexistent/run"])
        .assert()
        .code(1);
}

// ---- mads init ----

/// A tiny HTTP server: home page, sitemap and 404 for everything else.
fn serve_site() -> String {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let site = base.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, ctype, body) = match path.as_str() {
                "/" => (
                    "200 OK",
                    "text/html",
                    format!(
                        "<html><head><title>Vinellu</title><meta name=\"description\" content=\"App de vinhos\"></head><body><a href=\"{site}/app\">App</a><p>App social de vinhos</p></body></html>"
                    ),
                ),
                "/sitemap.xml" => (
                    "200 OK",
                    "application/xml",
                    format!(
                        "<urlset><url><loc>{site}/w/alamos</loc></url><url><loc>{site}/w/luigi</loc></url></urlset>"
                    ),
                ),
                _ => ("404 Not Found", "text/plain", "not found".to_string()),
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    base
}

fn init_script(site: &str) -> Value {
    let calls = vec![
        ("fetch_page", json!({"url": site})),
        ("fetch_sitemap", json!({})),
        ("search_site", json!({"names": ["Álamos", "Luigi Bosca"]})),
        (
            "write_business",
            json!({"name": "Vinellu", "url": site, "language": "pt-BR", "locations": ["Brazil"], "goal": "cadastros no app",
            "description": "App social de vinhos com reviews, safras e harmonização.", "pages": [{"name": "app", "url": format!("{site}/app")}]}),
        ),
        (
            "add_catalog_items",
            json!({"items": [
            {"name": "Alamos Malbec", "url": format!("{site}/w/alamos"), "category": "malbec", "aliases": ["alamos"], "third_party": true},
            {"name": "Luigi Bosca", "url": format!("{site}/w/luigi"), "category": "malbec", "third_party": true}]}),
        ),
        (
            "write_research",
            json!({"summary": "A social app for wine lovers. People rate labels and follow friends.",
            "opportunities": [{"name": "Labels by name", "intent": "catalog", "searches": ["alamos malbec"],
            "demand": "high", "competition": "low", "evidence": "Bestsellers get searched by name."}]}),
        ),
        ("finish", json!({})),
    ];
    resp(calls)
}

fn init_cmd(p: &Project, site: &str, script: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
    p.mads()
        .env("MADS_ALLOW_PRIVATE_HOSTS", "1")
        .args([
            "init",
            "--from-url",
            site,
            "--daily-budget",
            "50",
            "--currency",
            "BRL",
            "--provider",
            "replay",
            "--script",
        ])
        .arg(script)
        .args(["--out-dir", "site", "--mission-retries", "0"])
        .args(extra)
        .assert()
}

#[test]
fn init_drafts_business_and_catalog_from_a_site() {
    let site = serve_site();
    let p = Project::new(SITE);
    let script = p.script("init.json", json!({"init": [init_script(&site)]}));
    let out = init_cmd(&p, &site, &script, &["--format", "plain"]).success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(
        text.contains("[init] > fetch_page") && text.contains("[run] done (exit 0)"),
        "{text}"
    );
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("research.md"),
        "the operator is told to read it: {stderr}"
    );
    let toml = fs::read_to_string(p.dir.path().join("site/business.toml")).unwrap();
    assert!(
        toml.contains("name = \"Vinellu\"")
            && toml.contains("daily = 50.0")
            && toml.contains("currency = \"BRL\""),
        "{toml}"
    );
    assert!(toml.contains("file = \"catalog.csv\""));
    assert!(toml.contains("file = \"research.md\""), "{toml}");
    let research = fs::read_to_string(p.dir.path().join("site/research.md")).unwrap();
    assert!(research.contains("### 1. Labels by name"), "{research}");
    let csv = fs::read_to_string(p.dir.path().join("site/catalog.csv")).unwrap();
    assert!(csv.contains("Alamos Malbec") && csv.contains("Luigi Bosca"));
    assert!(
        !p.dir.path().join("site/events.ndjson").exists(),
        "init writes only its three files"
    );
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let site = serve_site();
    let p = Project::new(SITE);
    let script = p.script(
        "init.json",
        json!({"init": [init_script(&site), init_script(&site)]}),
    );
    init_cmd(&p, &site, &script, &[]).success();
    init_cmd(&p, &site, &script, &[])
        .code(2)
        .stderr(predicates::str::contains("--force"));
    init_cmd(&p, &site, &script, &["--force"]).success();
}

#[test]
fn a_failed_init_points_at_its_transcript() {
    let site = serve_site();
    let p = Project::new(SITE);
    let idle = json!({"text": "hmm", "tool_calls": [], "usage": {"input_tokens": 1, "output_tokens": 1, "cost_usd": null}});
    let script = p.script(
        "init.json",
        json!({"init": [idle.clone(), idle.clone(), idle]}),
    );
    init_cmd(&p, &site, &script, &[])
        .code(1)
        .stderr(predicates::str::contains(".mads/transcripts"));
    assert!(
        p.dir
            .path()
            .join("site/.mads/transcripts/init.jsonl")
            .exists()
    );
}

#[test]
fn init_runs_without_web_search_when_asked() {
    let site = serve_site();
    let p = Project::new(SITE);
    let script = p.script("init.json", json!({"init": [init_script(&site)]}));
    init_cmd(&p, &site, &script, &["--no-web-search"]).success();
    assert!(p.dir.path().join("site/research.md").exists());
}

#[test]
fn init_rejects_bad_arguments_with_exit_2() {
    let p = Project::new(SITE);
    let script = p.script("s.json", json!({"init": []}));
    let run = |args: &[&str]| {
        p.mads()
            .env("MADS_ALLOW_PRIVATE_HOSTS", "1")
            .arg("init")
            .args(args)
            .args(["--provider", "replay", "--script"])
            .arg(&script)
            .assert()
    };
    run(&[
        "--from-url",
        "not a url",
        "--daily-budget",
        "50",
        "--currency",
        "BRL",
    ])
    .code(2)
    .stderr(predicates::str::contains("--from-url"));
    run(&[
        "--from-url",
        "https://x.com",
        "--daily-budget",
        "0",
        "--currency",
        "BRL",
    ])
    .code(2)
    .stderr(predicates::str::contains("--daily-budget"));
    run(&[
        "--from-url",
        "https://x.com",
        "--daily-budget",
        "50.123",
        "--currency",
        "BRL",
    ])
    .code(2)
    .stderr(predicates::str::contains("--daily-budget"));
    run(&[
        "--from-url",
        "https://x.com",
        "--daily-budget",
        "50",
        "--currency",
        "brl",
    ])
    .code(2)
    .stderr(predicates::str::contains("--currency"));
    run(&[
        "--from-url",
        "ftp://x.com",
        "--daily-budget",
        "50",
        "--currency",
        "BRL",
    ])
    .code(2);
}

#[test]
fn init_without_a_provider_exits_2() {
    let p = Project::new(SITE);
    p.mads()
        .args([
            "init",
            "--from-url",
            "https://x.com",
            "--daily-budget",
            "50",
            "--currency",
            "BRL",
        ])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--provider"));
}

#[test]
fn a_failed_init_exits_1_and_writes_nothing() {
    let site = serve_site();
    let p = Project::new(SITE);
    let script = p.script("empty.json", json!({}));
    init_cmd(&p, &site, &script, &[]).code(1);
    assert!(!p.dir.path().join("site/business.toml").exists());
}

#[test]
fn init_by_default_blocks_loopback_sites() {
    let site = serve_site();
    let p = Project::new(SITE);
    let script = p.script("init.json", json!({"init": [init_script(&site)]}));
    p.mads()
        .args([
            "init",
            "--from-url",
            &site,
            "--daily-budget",
            "50",
            "--currency",
            "BRL",
            "--provider",
            "replay",
            "--script",
        ])
        .arg(&script)
        .args(["--out-dir", "site", "--mission-retries", "0"])
        .assert()
        .code(1);
    assert!(!p.dir.path().join("site/business.toml").exists());
}

fn image_plan() -> Value {
    let kit = json!({"headlines": texts("Titulo da marca", 10), "descriptions": texts("Descricao da marca com chamada pra acao", 3)});
    let plan = json!({"campaigns": [
        {"name": "Vinellu - Marca", "intent": "brand", "daily_budget": 20.0, "bid_strategy": {"type": "manual_cpc"}, "rationale": "protege marca",
         "ad_groups": [{"name": "marca", "theme": "vinellu", "entity_ids": []}]},
        {"name": "Vinellu - Feed", "kind": "demand_gen", "intent": "generic", "daily_budget": 30.0, "bid_strategy": {"type": "maximize_clicks"}, "rationale": "criar demanda",
         "ad_groups": [{"name": "tintos", "theme": "vinhos tintos", "entity_ids": []}]}
    ]});
    resp(vec![
        ("set_brand_kit", kit),
        ("set_account_plan", plan),
        ("finish", json!({})),
    ])
}

fn feed_campaign() -> Value {
    let prompt = "Photo of friends sharing red wine at a dinner table, warm evening light";
    let group = json!({"name": "tintos", "business_name": "Vinellu", "headlines": texts("Titulo", 3), "descriptions": ["Reviews reais de vinhos no app"]});
    let briefs = json!({"asset_group": "tintos", "images": [
        {"id": "jantar", "ratio": "landscape", "prompt": prompt},
        {"id": "taca", "ratio": "square", "prompt": prompt},
        {"id": "story", "ratio": "vertical", "prompt": prompt}
    ]});
    resp(vec![
        ("upsert_asset_group", group),
        ("set_image_briefs", briefs),
        ("finish", json!({})),
    ])
}

impl Project {
    fn with_logo(self) -> Self {
        let square =
            mads_core::images::solid_png(mads_core::google::AspectRatio::Square, [1, 2, 3]);
        let logo = mads_core::images::prepare_logo(&square).unwrap();
        fs::create_dir_all(self.dir.path().join("brand")).unwrap();
        fs::write(self.dir.path().join("brand/logo.png"), logo).unwrap();
        let toml = self.dir.path().join("business.toml");
        let text = fs::read_to_string(&toml).unwrap();
        fs::write(
            &toml,
            format!("{text}\n[brand]\nlogo = \"brand/logo.png\"\n"),
        )
        .unwrap();
        self
    }

    fn image_script(&self) -> PathBuf {
        self.script(
            "images.json",
            json!({"plan": [image_plan()], "campaign:vinellu-marca": [brand_campaign(&self.site)], "campaign:vinellu-feed": [feed_campaign()]}),
        )
    }
}

#[test]
fn generate_with_images_writes_the_editor_csv_and_the_pictures() {
    let p = Project::new(SITE).with_logo();
    p.generate(
        &p.image_script(),
        &["--skip-url-check", "--image-provider", "solid"],
    )
    .code(0);
    let run = p.run_dir();
    let editor = run.join("google-ads/editor");
    for f in [
        "image-campaigns.csv",
        "images/logo.png",
        "images/vinellu-feed/tintos-jantar.jpg",
        "images/vinellu-feed/tintos-taca.jpg",
        "images/vinellu-feed/tintos-story.jpg",
    ] {
        assert!(editor.join(f).is_file(), "missing {f}");
    }
    assert!(
        run.join("input/logo.png").is_file(),
        "the logo is part of the run"
    );
    assert!(
        run.join("google-ads/1-campaign.csv").is_file(),
        "search campaigns keep files 1 to 5"
    );
    let campaigns = fs::read_to_string(run.join("google-ads/1-campaign.csv")).unwrap();
    assert!(
        !campaigns.contains("Feed"),
        "image campaigns stay out of the bulk files"
    );
    let report = fs::read_to_string(run.join("report.md")).unwrap();
    assert!(report.contains("Model solid: 3 generated"), "{report}");

    p.mads()
        .args(["export"])
        .arg(&run)
        .arg("--skip-url-check")
        .assert()
        .code(0);
    let report = fs::read_to_string(run.join("report.md")).unwrap();
    assert!(
        report.contains("0 generated in this run, 3 reused"),
        "{report}"
    );
}

#[test]
fn without_an_image_model_the_plan_cannot_pick_an_image_campaign() {
    let p = Project::new(SITE).with_logo();
    p.generate(
        &p.image_script(),
        &["--skip-url-check", "--image-provider", "none"],
    )
    .code(1);
    let transcript = fs::read_to_string(p.run_dir().join("transcripts/plan.jsonl")).unwrap();
    assert!(
        transcript.contains("no image model in this run"),
        "{transcript}"
    );
}

#[test]
fn an_unknown_image_provider_is_a_usage_error() {
    let p = Project::new(SITE);
    p.generate(&p.full_script(), &["--image-provider", "dalle"])
        .code(2)
        .stderr(predicates::str::contains("unknown image provider"));
}

#[test]
fn providers_lists_the_image_providers() {
    let out = Command::cargo_bin("mads")
        .unwrap()
        .arg("providers")
        .env_remove("GEMINI_API_KEY")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(
        text.contains("IMAGE PROVIDER") && text.contains("missing GEMINI_API_KEY"),
        "{text}"
    );
}

#[test]
fn a_required_image_format_without_an_image_model_is_a_usage_error() {
    let p = Project::new(SITE).with_logo();
    let toml = p.dir.path().join("business.toml");
    let text = fs::read_to_string(&toml).unwrap();
    fs::write(
        &toml,
        format!("{text}\n[campaigns]\nformats = [\"search\", \"demand_gen\"]\n"),
    )
    .unwrap();
    p.generate(&p.image_script(), &["--image-provider", "none"])
        .code(2)
        .stderr(predicates::str::contains(
            "campaigns.formats asks for Demand Gen",
        ));
    p.generate(
        &p.image_script(),
        &["--skip-url-check", "--image-provider", "solid"],
    )
    .code(0);
}
