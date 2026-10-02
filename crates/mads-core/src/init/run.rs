use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

use super::{
    CATALOG_FILE, DESIGN_FILE, InitState, InitTools, KeptByHand, LOGO_FILE, RESEARCH_FILE,
    RenderedFiles, SiteFetch,
};
use crate::{
    agent::{
        Driver, DriverCtx, MissionOutcome, MissionReport, MissionSpec, TokenBudget, transcript_path,
    },
    events::{Event, EventSink, Totals},
    money::Cents,
    usage::Usage,
};

const INIT_ID: &str = "init";
const BUSINESS_FILE: &str = "business.toml";
/// With web search on, a model can skip it and write from memory. Citing the web is the proof it searched.
const WEB_SOURCES_MIN: usize = 3;
/// Hidden, so a reviewed folder still shows only the files to read.
const TRANSCRIPT_DIR: &str = ".mads/transcripts";
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
    /// Lets the agent search the web when the driver supports it.
    pub web_search: bool,
    /// The start URL is the one offer to advertise, not the whole business.
    pub focus: bool,
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
            web_search: true,
            focus: false,
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
    pub research: Option<PathBuf>,
    /// `brand/logo.png` when a logo was found on the site.
    pub logo: Option<PathBuf>,
    /// What the agent did, kept whether the mission finished or not.
    pub transcripts: PathBuf,
    pub totals: Totals,
}

/// The init mission. `web` is whether the driver can give the agent a web search.
pub fn mission_spec(cfg: &InitConfig, web: bool) -> MissionSpec {
    let web_search = cfg.web_search && web;
    let web_note = if web_search {
        "You can search the web with your own search tool. Use it before you write anything: the research must cite at least 3 pages outside the business site."
    } else {
        "You cannot search the web in this run: learn from the site only and list what you could not check in open_questions."
    };
    let focus_note = if cfg.focus {
        format!(
            " Focus: the account advertises only the offer of {}, not the whole business. Set `focus` in write_business with that page and its close variants, and keep the catalog and the opportunities inside the focus.",
            cfg.start_url
        )
    } else {
        String::new()
    };
    MissionSpec {
        id: INIT_ID.into(),
        system: INIT_PROMPT.into(),
        user: format!(
            "Research {url} and draft business.toml, catalog.csv and research.md. Start with fetch_page on {url}. The catalog can have at most {limit} items. {web_note}{focus_note}",
            url = cfg.start_url,
            limit = cfg.catalog_limit
        ),
        web_search,
    }
}

/// Runs the `init` mission and, when it finishes, writes its files. Exit codes: 0 ok, 1 failure.
pub async fn run_init(
    cfg: InitConfig,
    driver: Arc<dyn Driver>,
    site: Arc<dyn SiteFetch>,
    events: EventSink,
) -> Result<InitResult, InitError> {
    let business_path = cfg.out_dir.join(BUSINESS_FILE);
    let catalog_path = cfg.out_dir.join(CATALOG_FILE);
    let research_path = cfg.out_dir.join(RESEARCH_FILE);
    let logo_path = cfg.out_dir.join(LOGO_FILE);
    let design_path = cfg.out_dir.join(DESIGN_FILE);
    if !cfg.force {
        let existing: Vec<PathBuf> = [
            &business_path,
            &catalog_path,
            &research_path,
            &logo_path,
            &design_path,
        ]
        .into_iter()
        .filter(|p| p.exists())
        .cloned()
        .collect();
        if !existing.is_empty() {
            return Err(InitError::Exists(existing));
        }
    }
    let transcripts = fresh_transcripts(&cfg.out_dir)?;
    events.emit(Event::RunStarted {
        run_id: INIT_ID.into(),
        run_dir: String::new(),
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
    });

    let mission = mission_spec(&cfg, driver.web_search());
    let mut draft = InitState::new(&cfg.start_url, cfg.daily, &cfg.currency, cfg.catalog_limit);
    if cfg.focus {
        draft.require_focus();
    }
    keep_hand_written(&business_path, &mut draft, &events);
    if mission.web_search {
        draft.require_web_sources(WEB_SOURCES_MIN);
    }
    let state = Arc::new(Mutex::new(draft));
    let budget = Arc::new(TokenBudget::new(cfg.max_tokens));
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
            transcripts: Some(transcripts.clone()),
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

    let (catalog, research, logo) = if finished {
        if let Some(note) = state.lock().await.stale_formats_note() {
            events.emit(Event::Step {
                name: "keep".into(),
                detail: note,
            });
        }
        let logo = save_logo(&cfg, &state, site.as_ref(), &events).await?;
        let (catalog, research) = write_files(&cfg, &state, &events).await?;
        (catalog, research, logo)
    } else {
        (None, None, None)
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
        research,
        logo,
        transcripts,
        totals,
    })
}

/// Downloads the logo candidates in order and saves the first one Google would accept.
/// Without one, image campaigns stay off and the user can add `[brand] logo` by hand.
async fn save_logo(
    cfg: &InitConfig,
    state: &Mutex<InitState>,
    site: &dyn SiteFetch,
    events: &EventSink,
) -> Result<Option<PathBuf>, InitError> {
    let candidates = state.lock().await.logo_candidates().to_vec();
    let path = cfg.out_dir.join(LOGO_FILE);
    for url in &candidates {
        let prepared = site
            .fetch_image(url)
            .await
            .and_then(|b| crate::images::prepare_logo(&b));
        let Ok(png) = prepared else {
            continue;
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let colors = crate::images::logo_palette(&png).unwrap_or_default();
        std::fs::write(&path, png)?;
        let mut s = state.lock().await;
        s.set_logo(LOGO_FILE);
        s.set_logo_colors(&colors);
        drop(s);
        events.emit(Event::Step {
            name: "logo".into(),
            detail: format!("saved from {url}"),
        });
        events.emit(Event::ArtifactWritten {
            path: path.display().to_string(),
        });
        return Ok(Some(path));
    }
    if let Some(kept) = keep_existing_logo(&path, state).await {
        events.emit(Event::Step {
            name: "logo".into(),
            detail: format!("no logo on the site, kept {}", path.display()),
        });
        return Ok(Some(kept));
    }
    events.emit(Event::Step {
        name: "logo".into(),
        detail: format!(
            "none of {} candidates is a PNG or JPEG of 144 px or more: add [brand] logo for image campaigns",
            candidates.len()
        ),
    });
    Ok(None)
}

/// A logo already in the folder, put there by an earlier init or by hand, when it still passes the checks.
async fn keep_existing_logo(path: &Path, state: &Mutex<InitState>) -> Option<PathBuf> {
    let bytes = std::fs::read(path).ok()?;
    crate::images::check_logo(&bytes).ok()?;
    let colors = crate::images::logo_palette(&bytes).unwrap_or_default();
    let mut s = state.lock().await;
    s.set_logo(LOGO_FILE);
    s.set_logo_colors(&colors);
    Some(path.to_path_buf())
}

/// `--force` replaces business.toml, but what a person added by hand survives.
fn keep_hand_written(business: &Path, draft: &mut InitState, events: &EventSink) {
    let Ok(text) = std::fs::read_to_string(business) else {
        return;
    };
    let kept = KeptByHand::from_toml(&text);
    let names = kept.names();
    if !names.is_empty() {
        events.emit(Event::Step {
            name: "keep".into(),
            detail: format!("from the old business.toml: {}", names.join(", ")),
        });
    }
    draft.keep(kept);
}

/// The transcript folder with no file of an older init: a new run must not append to the last one.
fn fresh_transcripts(out_dir: &Path) -> Result<PathBuf, InitError> {
    let dir = out_dir.join(TRANSCRIPT_DIR);
    std::fs::create_dir_all(&dir)?;
    for extension in ["jsonl", "cli.jsonl"] {
        let old = transcript_path(&dir, INIT_ID, extension);
        if old.exists() {
            std::fs::remove_file(old)?;
        }
    }
    Ok(dir)
}

async fn write_files(
    cfg: &InitConfig,
    state: &Mutex<InitState>,
    events: &EventSink,
) -> Result<(Option<PathBuf>, Option<PathBuf>), InitError> {
    let RenderedFiles {
        toml,
        csv,
        research,
        design,
    } = state
        .lock()
        .await
        .render_files()
        .map_err(io::Error::other)?;
    std::fs::create_dir_all(&cfg.out_dir)?;
    let business = cfg.out_dir.join(BUSINESS_FILE);
    write_file(&business, &toml, events)?;
    let catalog = write_optional(cfg, CATALOG_FILE, csv, events)?;
    let research = write_optional(cfg, RESEARCH_FILE, research, events)?;
    write_optional(cfg, DESIGN_FILE, design, events)?;
    Ok((catalog, research))
}

fn write_file(path: &Path, text: &str, events: &EventSink) -> Result<(), InitError> {
    std::fs::write(path, text)?;
    events.emit(Event::ArtifactWritten {
        path: path.display().to_string(),
    });
    Ok(())
}

/// Writes the file when there is content. Without content, `--force` removes a leftover from an
/// older run: it would not match the new business.toml.
fn write_optional(
    cfg: &InitConfig,
    name: &str,
    content: Option<String>,
    events: &EventSink,
) -> Result<Option<PathBuf>, InitError> {
    let path = cfg.out_dir.join(name);
    match content {
        Some(text) => {
            write_file(&path, &text, events)?;
            Ok(Some(path))
        }
        None => {
            if cfg.force && path.exists() {
                std::fs::remove_file(&path)?;
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
        init::{FetchedPage, SitemapUrls},
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
                image: String::new(),
                theme_color: "#AD1457".into(),
                app_links: vec![
                    "https://play.google.com/store/apps/details?id=com.vinellu.app".into(),
                ],
                logos: vec![
                    "https://vinellu.com/tiny.png".into(),
                    "https://vinellu.com/apple-touch-icon.png".into(),
                ],
            })
        }
        async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, String> {
            let side = if url.ends_with("tiny.png") { 32 } else { 180 };
            let img = image::RgbaImage::from_pixel(side, side, image::Rgba([90, 0, 40, 255]));
            let mut out = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        async fn fetch_sitemap(&self, _: Option<&str>) -> Result<SitemapUrls, String> {
            Ok(SitemapUrls {
                urls: vec![
                    "https://vinellu.com/w/alamos".into(),
                    "https://vinellu.com/w/luigi".into(),
                ],
                skipped: vec![],
            })
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
                "pages": [{"name": "app", "url": "https://vinellu.com/app"}], "restricted": ["alcohol"]}),
            ),
            (
                "add_catalog_items",
                json!({"items": [
                {"name": "Alamos Malbec", "url": "https://vinellu.com/w/alamos", "category": "malbec", "aliases": ["alamos"], "third_party": true},
                {"name": "Luigi Bosca", "url": "https://vinellu.com/w/luigi", "category": "malbec", "third_party": true}]}),
            ),
            ("write_research", research_args()),
            ("finish", json!({})),
        ])
    }

    fn research_args() -> Value {
        json!({"summary": "A social app for wine lovers. People rate labels and follow friends.",
               "opportunities": [{"name": "Labels by name", "intent": "catalog", "searches": ["alamos malbec"],
                                  "demand": "high", "competition": "low", "evidence": "Bestsellers get searched by name."}],
               "open_questions": ["How much is an install worth?"]})
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
        let logo = dir.path().join("brand/logo.png");
        assert_eq!(
            r.logo,
            Some(logo.clone()),
            "the tiny icon is skipped, the touch icon kept"
        );
        assert!(input.logo.is_some_and(|l| l.ends_with("brand/logo.png")));
        assert_eq!(input.app.map(|a| a.id), Some("com.vinellu.app".to_string()));
        crate::images::check_logo(&std::fs::read(logo).unwrap()).unwrap();
        let design = std::fs::read_to_string(dir.path().join("DESIGN.md")).unwrap();
        assert!(
            design.contains(": logo\n")
                && design.contains("#AD1457: theme color of https://vinellu.com"),
            "{design}"
        );
        assert_eq!(input.design, design, "generate reads it back");
    }

    struct NoLogoSite;

    #[async_trait]
    impl SiteFetch for NoLogoSite {
        fn is_same_site(&self, url: &str) -> bool {
            FakeSite.is_same_site(url)
        }
        async fn fetch_page(&self, url: &str) -> Result<FetchedPage, String> {
            let mut p = FakeSite.fetch_page(url).await?;
            p.logos.clear();
            Ok(p)
        }
        async fn fetch_sitemap(&self, url: Option<&str>) -> Result<SitemapUrls, String> {
            FakeSite.fetch_sitemap(url).await
        }
    }

    #[tokio::test]
    async fn without_a_logo_on_the_site_an_existing_one_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let brand = dir.path().join("brand");
        std::fs::create_dir(&brand).unwrap();
        let square = crate::images::solid_png(crate::google::AspectRatio::Square, [200, 30, 90]);
        std::fs::write(
            brand.join("logo.png"),
            crate::images::prepare_logo(&square).unwrap(),
        )
        .unwrap();
        let mut c = cfg(&dir);
        c.force = true;
        let (events, _rx) = EventSink::channel();
        let r = run_init(
            c,
            driver(json!({"init": [good_script()]})),
            Arc::new(NoLogoSite),
            events,
        )
        .await
        .unwrap();
        assert_eq!(r.logo, Some(brand.join("logo.png")));
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        assert!(input.logo.is_some(), "[brand] points at the kept logo");
        assert!(
            input.design.contains("Pink"),
            "its colors reach DESIGN.md: {}",
            input.design
        );
    }

    const OLD_TOML: &str = "[business]\nname = \"Old\"\n\n[budget]\ndaily = 1.0\ncurrency = \"USD\"\nmax_cpc = 2.5\n\n[export]\nstatus = \"Enabled\"\n\n[campaigns]\nformats = [\"search\", \"demand_gen\"]\n\n[app]\nstore = \"app_store\"\nid = \"123\"\n";

    #[tokio::test]
    async fn force_keeps_what_a_person_wrote_by_hand() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("business.toml"), OLD_TOML).unwrap();
        let mut c = cfg(&dir);
        c.force = true;
        let (r, events) = run(c, driver(json!({"init": [good_script()]}))).await;
        assert_eq!(r.unwrap().exit_code, 0);
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        assert_eq!(
            input.business.name, "Vinellu",
            "the draft replaces [business]"
        );
        assert_eq!(input.budget.max_cpc, Some(Cents(250)));
        assert_eq!(input.export.status, crate::input::ExportStatus::Enabled);
        assert_eq!(input.formats.len(), 2);
        assert_eq!(
            input.app.map(|a| a.id),
            Some("com.vinellu.app".to_string()),
            "the app the site links to wins over the old one"
        );
        assert!(events.iter().any(|e| matches!(e, Event::Step { name, detail } if name == "keep" && detail.contains("[campaigns]"))));
        assert!(
            events.iter().any(|e| matches!(e, Event::Step { detail, .. } if detail.contains("add \"app_installs\""))),
            "the kept formats skip the app the site links to"
        );
    }

    #[tokio::test]
    async fn a_kept_part_that_no_longer_fits_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let old = OLD_TOML.replace("\"demand_gen\"", "\"performance_max\"");
        std::fs::write(dir.path().join("business.toml"), old).unwrap();
        let mut c = cfg(&dir);
        c.force = true;
        let (r, _) = run(c, driver(json!({"init": [good_script()]}))).await;
        assert_eq!(r.unwrap().exit_code, 0);
        let input = load_input(&dir.path().join("business.toml"))
            .expect("Performance Max without conversion tracking is dropped, the file loads");
        assert!(input.formats.is_empty());
    }

    #[test]
    fn focus_reaches_the_task_only_when_asked() {
        let mut c = InitConfig::new(
            PathBuf::from("."),
            "https://x.com/route/a-b".into(),
            Cents(100),
            "BRL".into(),
        );
        assert!(!mission_spec(&c, false).user.contains("Focus"));
        c.focus = true;
        let m = mission_spec(&c, false);
        assert!(
            m.user.contains(
                "Focus: the account advertises only the offer of https://x.com/route/a-b"
            )
        );
        assert!(m.system.contains("## focus"));
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
            5,
            "business.toml, catalog.csv, research.md, the logo and DESIGN.md"
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
    async fn the_research_is_written_and_reaches_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let (result, _) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        assert_eq!(
            result.unwrap().research,
            Some(dir.path().join("research.md"))
        );
        let md = std::fs::read_to_string(dir.path().join("research.md")).unwrap();
        assert!(md.starts_with("# Research: Vinellu"), "{md}");
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        assert_eq!(
            input.research, md,
            "generate reads the notes the operator reviewed"
        );
    }

    #[test]
    fn the_prompt_names_every_tool_and_stays_business_agnostic() {
        let dir = tempfile::tempdir().unwrap();
        let tools = InitTools::new(
            Arc::new(Mutex::new(InitState::new(
                "https://vinellu.com",
                Cents(5000),
                "BRL",
                5,
            ))),
            Arc::new(FakeSite),
            40,
            30,
        );
        let system = mission_spec(&cfg(&dir), true).system;
        for spec in crate::tools::ToolHost::specs(&tools) {
            assert!(
                system.contains(&spec.name),
                "init prompt misses {}",
                spec.name
            );
        }
        for niche in ["wine", "vinho", "grape", "label"] {
            assert!(
                !system.to_lowercase().contains(niche),
                "the prompt must not teach one business: found '{niche}'"
            );
        }
    }

    #[test]
    fn the_mission_asks_for_web_search_only_when_it_can_have_it() {
        let dir = tempfile::tempdir().unwrap();
        let c = cfg(&dir);
        let on = mission_spec(&c, true);
        assert!(on.web_search);
        assert!(on.user.contains("You can search the web"), "{}", on.user);
        let off = mission_spec(&c, false);
        assert!(!off.web_search);
        assert!(off.user.contains("cannot search the web"), "{}", off.user);
        let mut opted_out = cfg(&dir);
        opted_out.web_search = false;
        assert!(!mission_spec(&opted_out, true).web_search);
    }

    /// A scripted driver that says it can search the web, like an agent CLI.
    struct WebDriver(Arc<dyn Driver>);

    #[async_trait]
    impl Driver for WebDriver {
        fn web_search(&self) -> bool {
            true
        }
        async fn run_mission(
            &self,
            mission: &MissionSpec,
            tools: Arc<dyn crate::tools::ToolHost>,
            ctx: &DriverCtx,
        ) -> MissionReport {
            self.0.run_mission(mission, tools, ctx).await
        }
    }

    fn script_with_sources(sources: Value) -> Value {
        let mut script = good_script();
        let calls = script["tool_calls"].as_array_mut().unwrap();
        let research = calls
            .iter_mut()
            .find(|c| c["name"] == "write_research")
            .unwrap();
        research["arguments"]["opportunities"][0]["sources"] = sources;
        script
    }

    #[tokio::test]
    async fn with_web_search_research_without_web_sources_cannot_finish() {
        let dir = tempfile::tempdir().unwrap();
        let web = Arc::new(WebDriver(driver(json!({"init": [good_script()]}))));
        let (result, _) = run(cfg(&dir), web).await;
        assert_eq!(result.unwrap().exit_code, 1);
        let transcript =
            std::fs::read_to_string(dir.path().join(".mads/transcripts/init.jsonl")).unwrap();
        assert!(
            transcript.contains("outside the business site"),
            "{transcript}"
        );
    }

    #[tokio::test]
    async fn with_web_search_cited_web_sources_let_it_finish() {
        let dir = tempfile::tempdir().unwrap();
        let script = script_with_sources(json!([
            "https://example.com/ranking",
            "https://news.example.org/top",
            "https://shop.example.net/bestsellers"
        ]));
        let web = Arc::new(WebDriver(driver(json!({"init": [script]}))));
        let (result, _) = run(cfg(&dir), web).await;
        assert_eq!(result.unwrap().exit_code, 0);
    }

    #[tokio::test]
    async fn the_transcript_is_kept_next_to_the_files() {
        let dir = tempfile::tempdir().unwrap();
        let (result, _) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        let path = dir.path().join(".mads/transcripts/init.jsonl");
        assert_eq!(
            result.unwrap().transcripts,
            dir.path().join(".mads/transcripts")
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("write_research"), "{text}");
    }

    #[tokio::test]
    async fn a_failed_run_keeps_its_transcript_for_debugging() {
        let dir = tempfile::tempdir().unwrap();
        let (result, _) = run(cfg(&dir), driver(json!({"init": [idle(), idle(), idle()]}))).await;
        assert_eq!(result.unwrap().exit_code, 1);
        assert!(dir.path().join(".mads/transcripts/init.jsonl").exists());
    }

    #[tokio::test]
    async fn a_new_run_starts_a_new_transcript() {
        let dir = tempfile::tempdir().unwrap();
        run(cfg(&dir), driver(json!({"init": [good_script()]})))
            .await
            .0
            .unwrap();
        let mut again = cfg(&dir);
        again.force = true;
        run(again, driver(json!({"init": [good_script()]})))
            .await
            .0
            .unwrap();
        let text =
            std::fs::read_to_string(dir.path().join(".mads/transcripts/init.jsonl")).unwrap();
        assert_eq!(text.matches("write_research").count(), 1, "{text}");
    }

    #[tokio::test]
    async fn an_existing_research_file_is_protected_too() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research.md"), "mine").unwrap();
        let (result, _) = run(cfg(&dir), driver(json!({"init": [good_script()]}))).await;
        assert!(matches!(result, Err(InitError::Exists(_))));
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
                "description": "App social de vinhos com reviews, safras e harmonização.", "restricted": []}),
            ),
            ("write_research", research_args()),
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
