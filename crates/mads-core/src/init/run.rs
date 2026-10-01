use std::{io, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::Mutex;

use super::{CATALOG_FILE, InitState, InitTools, SiteFetch};
use crate::{
    agent::{Driver, DriverCtx, MissionOutcome, MissionReport, MissionSpec, TokenBudget},
    events::{Event, EventSink, Totals},
    money::Cents,
    usage::Usage,
};

const INIT_ID: &str = "init";
const BUSINESS_FILE: &str = "business.toml";
const INIT_PROMPT: &str = include_str!("../../prompts/init.md");

pub struct InitConfig {
    /// Where `business.toml` and `catalog.csv` are written.
    pub out_dir: PathBuf,
    pub start_url: String,
    pub daily: Cents,
    pub currency: String,
    pub catalog_limit: usize,
    pub force: bool,
    pub max_turns: usize,
    pub mission_timeout: Duration,
    pub mission_retries: u32,
    pub max_tokens: u64,
    /// Pages the agent may fetch.
    pub page_limit: usize,
    pub provider: String,
    pub model: Option<String>,
}

impl InitConfig {
    pub fn new(out_dir: PathBuf, start_url: String, daily: Cents, currency: String) -> Self {
        Self {
            out_dir,
            start_url,
            daily,
            currency,
            catalog_limit: 50,
            force: false,
            max_turns: 40,
            mission_timeout: Duration::from_secs(15 * 60),
            mission_retries: 1,
            max_tokens: 4_000_000,
            page_limit: 30,
            provider: String::new(),
            model: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("{} already exist: use --force to overwrite", .0.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    Exists(Vec<PathBuf>),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub struct InitResult {
    pub exit_code: i32,
    pub business: PathBuf,
    pub catalog: Option<PathBuf>,
    pub totals: Totals,
}

/// Runs the `init` mission and, when it finishes, writes the two files. Exit codes: 0 ok, 1 failure.
pub async fn run_init(
    cfg: InitConfig,
    driver: Arc<dyn Driver>,
    site: Arc<dyn SiteFetch>,
    events: EventSink,
) -> Result<InitResult, InitError> {
    let business_path = cfg.out_dir.join(BUSINESS_FILE);
    let catalog_path = cfg.out_dir.join(CATALOG_FILE);
    if !cfg.force {
        let existing: Vec<PathBuf> = [&business_path, &catalog_path]
            .into_iter()
            .filter(|p| p.exists())
            .cloned()
            .collect();
        if !existing.is_empty() {
            return Err(InitError::Exists(existing));
        }
    }
    events.emit(Event::RunStarted {
        run_id: INIT_ID.into(),
        run_dir: String::new(),
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
    });

    let state = Arc::new(Mutex::new(InitState::new(
        &cfg.start_url,
        cfg.daily,
        &cfg.currency,
        cfg.catalog_limit,
    )));
    let budget = Arc::new(TokenBudget::new(cfg.max_tokens));
    let mission = MissionSpec {
        id: INIT_ID.into(),
        system: INIT_PROMPT.into(),
        user: format!(
            "Draft business.toml and catalog.csv for {url}. Start with fetch_page on {url}. The catalog can have at most {limit} items.",
            url = cfg.start_url,
            limit = cfg.catalog_limit
        ),
    };
    let (mut usage, mut finished) = (Usage::default(), false);
    for attempt in 1..=1 + cfg.mission_retries {
        if budget.exceeded() {
            break;
        }
        events.emit(Event::MissionStarted {
            mission: INIT_ID.into(),
            attempt,
        });
        let tools = Arc::new(InitTools::new(
            state.clone(),
            site.clone(),
            cfg.max_turns,
            cfg.page_limit,
        ));
        let ctx = DriverCtx {
            events: events.clone(),
            max_turns: cfg.max_turns,
            budget: budget.clone(),
            transcripts: None,
        };
        let report = tokio::time::timeout(
            cfg.mission_timeout,
            driver.run_mission(&mission, tools, &ctx),
        )
        .await
        .unwrap_or_else(|_| MissionReport {
            outcome: MissionOutcome::Failed("timeout".into()),
            usage: Usage::default(),
            turns: 0,
        });
        usage.add(&report.usage);
        let (ok, reason) = match report.outcome {
            MissionOutcome::Finished => (true, None),
            MissionOutcome::BudgetExceeded => (false, Some("token budget exceeded".to_string())),
            MissionOutcome::Failed(r) => (false, Some(r)),
        };
        events.emit(Event::MissionFinished {
            mission: INIT_ID.into(),
            ok,
            reason,
        });
        if ok {
            finished = true;
        }
        if ok || budget.exceeded() {
            break;
        }
    }

    let catalog = if finished {
        write_files(&cfg, &state, &events).await?
    } else {
        None
    };
    let exit_code = if finished { 0 } else { 1 };
    let totals = Totals {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_usd: usage.cost_usd,
        missions: 1,
        failed_missions: usize::from(!finished),
    };
    events.emit(Event::RunFinished {
        ok: finished,
        exit_code,
        totals: totals.clone(),
    });
    Ok(InitResult {
        exit_code,
        business: business_path,
        catalog,
        totals,
    })
}

async fn write_files(
    cfg: &InitConfig,
    state: &Mutex<InitState>,
    events: &EventSink,
) -> Result<Option<PathBuf>, InitError> {
    let (toml, csv) = state
        .lock()
        .await
        .render_files()
        .map_err(io::Error::other)?;
    std::fs::create_dir_all(&cfg.out_dir)?;
    let business = cfg.out_dir.join(BUSINESS_FILE);
    std::fs::write(&business, toml)?;
    events.emit(Event::ArtifactWritten {
        path: business.display().to_string(),
    });
    let catalog = cfg.out_dir.join(CATALOG_FILE);
    match csv {
        Some(text) => {
            std::fs::write(&catalog, text)?;
            events.emit(Event::ArtifactWritten {
                path: catalog.display().to_string(),
            });
            Ok(Some(catalog))
        }
        None => {
            // `--force` over an older run: a leftover catalog would not match the new business.toml.
            if cfg.force && catalog.exists() {
                std::fs::remove_file(&catalog)?;
            }
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        agent::ScriptedDriver,
        events::{Event, EventSink},
        init::FetchedPage,
        input::load_input,
    };

    struct FakeSite;

    #[async_trait]
    impl SiteFetch for FakeSite {
        fn is_same_site(&self, url: &str) -> bool {
            url.starts_with("https://vinellu.com")
        }
        async fn fetch_page(&self, url: &str) -> Result<FetchedPage, String> {
            if url != "https://vinellu.com" {
                return Err("HTTP 404".into());
            }
            Ok(FetchedPage {
                url: url.into(),
                status: 200,
                title: "Vinellu".into(),
                description: "App de vinhos".into(),
                text: "App social de vinhos".into(),
                links: vec!["https://vinellu.com/app".into()],
            })
        }
        async fn fetch_sitemap(&self, _: Option<&str>) -> Result<Vec<String>, String> {
            Ok(vec![
                "https://vinellu.com/w/alamos".into(),
                "https://vinellu.com/w/luigi".into(),
            ])
        }
    }

    fn resp(calls: Vec<(&str, Value)>) -> Value {
        let tool_calls: Vec<Value> = calls
            .into_iter()
            .enumerate()
            .map(|(i, (n, a))| json!({"id": format!("c{i}"), "name": n, "arguments": a}))
            .collect();
        json!({"text": null, "tool_calls": tool_calls, "usage": {"input_tokens": 100, "output_tokens": 20, "cost_usd": null}})
    }

    fn good_script() -> Value {
        resp(vec![
            ("fetch_page", json!({"url": "https://vinellu.com"})),
            ("fetch_sitemap", json!({})),
            (
                "write_business",
                json!({"name": "Vinellu", "url": "https://vinellu.com", "language": "pt-BR", "locations": ["Brazil"], "goal": "cadastros no app",
                "description": "App social de vinhos com reviews, safras e harmonização.", "competitors": ["Vivino"],
                "pages": [{"name": "app", "url": "https://vinellu.com/app"}]}),
            ),
            (
                "add_catalog_items",
                json!({"items": [
                {"name": "Alamos Malbec", "url": "https://vinellu.com/w/alamos", "category": "malbec", "aliases": ["alamos"], "third_party": true},
                {"name": "Luigi Bosca", "url": "https://vinellu.com/w/luigi", "category": "malbec", "third_party": true}]}),
            ),
            ("finish", json!({})),
        ])
    }

    fn idle() -> Value {
        json!({"text": "hmm", "tool_calls": [], "usage": {"input_tokens": 1, "output_tokens": 1, "cost_usd": null}})
    }

    fn driver(missions: Value) -> Arc<dyn Driver> {
        Arc::new(ScriptedDriver::from_json(&json!({"missions": missions}).to_string()).unwrap())
    }

    fn cfg(dir: &tempfile::TempDir) -> InitConfig {
        let mut c = InitConfig::new(
            dir.path().to_path_buf(),
            "https://vinellu.com".into(),
            Cents(5000),
            "BRL".into(),
        );
        c.mission_retries = 0;
        c
    }

    async fn run(
        cfg: InitConfig,
        d: Arc<dyn Driver>,
    ) -> (Result<InitResult, InitError>, Vec<Event>) {
        let (events, mut rx) = EventSink::channel();
        let result = run_init(cfg, d, Arc::new(FakeSite), events).await;
        let mut all = Vec::new();
        while let Ok(e) = rx.try_recv() {
            all.push(e.event);
        }
        (result, all)
    }

    #[tokio::test]
    async fn the_generated_files_load_with_the_real_input_loader() {
        let dir = tempfile::tempdir().unwrap();
        let (result, _) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        let r = result.unwrap();
        assert_eq!(r.exit_code, 0);
        let input = load_input(&dir.path().join("business.toml"))
            .expect("generate must accept what init wrote");
        assert_eq!(input.business.name, "Vinellu");
        assert_eq!(input.catalog.len(), 2);
        assert_eq!(input.catalog[0].id, "alamos-malbec");
        assert_eq!(
            (input.budget.daily, input.budget.currency.as_str()),
            (Cents(5000), "BRL")
        );
        assert_eq!(r.business, dir.path().join("business.toml"));
        assert_eq!(r.catalog, Some(dir.path().join("catalog.csv")));
    }

    #[tokio::test]
    async fn events_cover_the_mission_and_the_files() {
        let dir = tempfile::tempdir().unwrap();
        let (_, events) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        assert!(matches!(events.first(), Some(Event::RunStarted { .. })));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::MissionStarted { mission, .. } if mission == "init"))
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::ArtifactWritten { .. }))
                .count(),
            2
        );
        assert!(matches!(
            events.last(),
            Some(Event::RunFinished {
                ok: true,
                exit_code: 0,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn existing_files_are_not_overwritten_without_force() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("business.toml"), "mine").unwrap();
        let (result, events) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        assert!(matches!(result, Err(InitError::Exists(_))));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("business.toml")).unwrap(),
            "mine"
        );
        assert!(events.is_empty(), "nothing runs before the check");
        let mut forced = cfg(&dir);
        forced.force = true;
        let (result, _) = run(forced, driver(json!({"init": [good_script()]}))).await;
        assert_eq!(result.unwrap().exit_code, 0);
        assert!(
            std::fs::read_to_string(dir.path().join("business.toml"))
                .unwrap()
                .contains("Vinellu")
        );
    }

    #[tokio::test]
    async fn a_failed_mission_writes_nothing_and_exits_1() {
        let dir = tempfile::tempdir().unwrap();
        let (result, events) = run(cfg(&dir), driver(json!({}))).await;
        assert_eq!(result.unwrap().exit_code, 1);
        assert!(!dir.path().join("business.toml").exists());
        assert!(matches!(
            events.last(),
            Some(Event::RunFinished {
                ok: false,
                exit_code: 1,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn a_failed_attempt_is_retried() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = cfg(&dir);
        c.mission_retries = 1;
        let (result, _) = run(
            c,
            driver(json!({"init": [idle(), idle(), idle(), good_script()]})),
        )
        .await;
        assert_eq!(result.unwrap().exit_code, 0);
    }

    #[tokio::test]
    async fn without_catalog_items_only_the_toml_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let script = resp(vec![
            ("fetch_page", json!({"url": "https://vinellu.com"})),
            (
                "write_business",
                json!({"name": "Vinellu", "url": "https://vinellu.com", "language": "pt-BR", "locations": ["Brazil"], "goal": "g",
                "description": "App social de vinhos com reviews, safras e harmonização."}),
            ),
            ("finish", json!({})),
        ]);
        let (result, _) = run(cfg(&dir), driver(json!({"init": [script]}))).await;
        let r = result.unwrap();
        assert_eq!(r.catalog, None);
        assert!(!dir.path().join("catalog.csv").exists());
        assert!(
            load_input(&dir.path().join("business.toml"))
                .unwrap()
                .catalog
                .is_empty()
        );
    }

    #[tokio::test]
    async fn the_output_directory_is_created_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = cfg(&dir);
        c.out_dir = dir.path().join("nested/site");
        let (result, _) = run(c, driver(json!({"init": [good_script()]}))).await;
        assert_eq!(result.unwrap().exit_code, 0);
        assert!(dir.path().join("nested/site/business.toml").exists());
    }
}
