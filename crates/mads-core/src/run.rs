use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::Serialize;
use time::{OffsetDateTime, format_description};
use tokio::{
    sync::{Mutex, Semaphore},
    task::JoinSet,
};

use crate::{
    agent::{Driver, DriverCtx, MissionOutcome, MissionReport, MissionSpec, TokenBudget},
    events::{Event, EventSink, Totals},
    finalize::{ExportOpts, Finalized, finalize},
    google::ExportLayout,
    images::{EDITOR_DIR, ImageModel, ImageStepConfig, ImageStepResult, run_image_step},
    input::Input,
    mission::{PLAN_ID, campaign_id, campaign_mission, plan_mission},
    post::cross_negatives,
    report::{ReportData, ReportStatus, render_report},
    tools::{MissionKind, MissionTools, SharedWorkspace, ToolSettings},
    usage::Usage,
    web::Web,
    workspace::{MissionState, MissionStatus, Workspace},
};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("run has unfinished missions ({0}); run `mads generate --resume <run-dir>` first")]
    Unfinished(String),
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Parent of the run directories.
    pub out_dir: PathBuf,
    /// Existing run directory to resume.
    pub run_dir: Option<PathBuf>,
    pub parallel: usize,
    pub max_turns: usize,
    pub mission_timeout: Duration,
    /// Whole-run ceiling, input plus output tokens. Zero disables it.
    pub max_tokens: u64,
    pub mission_retries: u32,
    pub max_ad_groups: usize,
    pub skip_url_check: bool,
    /// `bulk` or the local drive-folder Editor set.
    pub layout: ExportLayout,
    /// New images allowed in one run.
    pub max_images: usize,
    pub provider: String,
    pub model: Option<String>,
}

impl RunConfig {
    pub fn new(out_dir: PathBuf) -> Self {
        Self {
            out_dir,
            run_dir: None,
            parallel: 4,
            max_turns: 40,
            mission_timeout: Duration::from_secs(15 * 60),
            max_tokens: 4_000_000,
            mission_retries: 1,
            max_ad_groups: 50,
            skip_url_check: false,
            layout: ExportLayout::Bulk,
            max_images: 40,
            provider: String::new(),
            model: None,
        }
    }
}

pub struct Drivers {
    pub plan: Arc<dyn Driver>,
    pub campaign: Arc<dyn Driver>,
    /// Makes the pictures of image campaigns. None: the plan can only use Search.
    pub image: Option<Arc<dyn ImageModel>>,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub run_dir: PathBuf,
    pub exit_code: i32,
    pub totals: Totals,
}

/// `<out>/<YYYYMMDD-HHMMSS>-<6 hex>/`
#[derive(Debug, Clone)]
pub struct RunDir {
    root: PathBuf,
}

/// Files for each ad platform live in their own folder, so a run can hold more than one later.
pub const PLATFORM_DIR: &str = "google-ads";

const SUBDIRS: [&str; 3] = ["input", "transcripts", PLATFORM_DIR];

impl RunDir {
    pub fn create(out: &Path) -> io::Result<Self> {
        let fmt =
            format_description::parse_borrowed::<2>("[year][month][day]-[hour][minute][second]")
                .map_err(io::Error::other)?;
        let stamp = OffsetDateTime::now_utc()
            .format(&fmt)
            .map_err(io::Error::other)?;
        let suffix: String = uuid::Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(6)
            .collect();
        let root = out.join(format!("{stamp}-{suffix}"));
        std::fs::create_dir_all(&root)?;
        Self::open(&root)
    }

    pub fn open(root: &Path) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("run directory not found: {}", root.display()),
            ));
        }
        for sub in SUBDIRS {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn id(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.root.join("workspace.json")
    }

    pub fn platform_dir(&self) -> PathBuf {
        self.root.join(PLATFORM_DIR)
    }

    pub fn report_path(&self) -> PathBuf {
        self.root.join("report.md")
    }

    pub fn run_json_path(&self) -> PathBuf {
        self.root.join("run.json")
    }
}

struct Runner {
    ws: SharedWorkspace,
    ws_path: PathBuf,
    transcripts: PathBuf,
    events: EventSink,
    budget: Arc<TokenBudget>,
    max_turns: usize,
    mission_timeout: Duration,
    mission_retries: u32,
    max_ad_groups: usize,
    image_model: bool,
}

fn is_finished(ws: &Workspace, id: &str) -> bool {
    ws.missions
        .get(id)
        .is_some_and(|m| m.status == MissionStatus::Finished)
}

impl Runner {
    async fn update(&self, id: &str, f: impl FnOnce(&mut MissionState)) {
        let mut ws = self.ws.lock().await;
        f(ws.missions.entry(id.to_string()).or_default());
        // The final write in `conclude` reports IO failures; tools also persist after every change.
        let _ = ws.save(&self.ws_path);
    }

    async fn fail(&self, id: &str, reason: &str) {
        let r = reason.to_string();
        self.update(id, |m| m.status = MissionStatus::Failed { reason: r })
            .await;
        self.events.emit(Event::MissionFinished {
            mission: id.into(),
            ok: false,
            reason: Some(reason.into()),
        });
    }

    async fn run_mission(
        &self,
        spec: MissionSpec,
        kind: MissionKind,
        driver: Arc<dyn Driver>,
    ) -> bool {
        let allowed = 1 + self.mission_retries;
        for attempt in 1..=allowed {
            if self.budget.exceeded() {
                self.fail(&spec.id, "token budget exceeded").await;
                return false;
            }
            self.update(&spec.id, |m| {
                m.attempts += 1;
                m.status = MissionStatus::Running;
            })
            .await;
            self.events.emit(Event::MissionStarted {
                mission: spec.id.clone(),
                attempt,
            });
            let settings = ToolSettings {
                max_ad_groups: self.max_ad_groups,
                max_turns: self.max_turns,
                image_model: self.image_model,
            };
            let tools = match MissionTools::new(
                self.ws.clone(),
                kind.clone(),
                settings,
                Some(self.ws_path.clone()),
            )
            .await
            {
                Ok(t) => Arc::new(t),
                Err(e) => {
                    self.fail(&spec.id, &e.to_string()).await;
                    return false;
                }
            };
            let ctx = DriverCtx {
                events: self.events.clone(),
                max_turns: self.max_turns,
                budget: self.budget.clone(),
                transcripts: Some(self.transcripts.clone()),
            };
            let report = match tokio::time::timeout(
                self.mission_timeout,
                driver.run_mission(&spec, tools, &ctx),
            )
            .await
            {
                Ok(r) => r,
                Err(_) => MissionReport {
                    outcome: MissionOutcome::Failed("timeout".into()),
                    usage: Usage::default(),
                    turns: 0,
                },
            };
            let used = report.usage;
            self.update(&spec.id, |m| m.usage.add(&used)).await;
            match report.outcome {
                MissionOutcome::Finished => {
                    self.update(&spec.id, |m| m.status = MissionStatus::Finished)
                        .await;
                    self.events.emit(Event::MissionFinished {
                        mission: spec.id.clone(),
                        ok: true,
                        reason: None,
                    });
                    return true;
                }
                MissionOutcome::BudgetExceeded => {
                    self.fail(&spec.id, "token budget exceeded").await;
                    return false;
                }
                MissionOutcome::Failed(reason) => {
                    self.events.emit(Event::MissionFinished {
                        mission: spec.id.clone(),
                        ok: false,
                        reason: Some(reason.clone()),
                    });
                    if attempt == allowed {
                        self.update(&spec.id, |m| m.status = MissionStatus::Failed { reason })
                            .await;
                    }
                }
            }
        }
        false
    }

    /// Plan first, then every unfinished campaign in parallel. A failed campaign does not stop the others.
    async fn run_all(self: &Arc<Self>, drivers: &Drivers, parallel: usize) -> bool {
        let (needs_plan, plan) = {
            let ws = self.ws.lock().await;
            (
                !is_finished(&ws, PLAN_ID) || ws.account.campaigns.is_empty(),
                plan_mission(&ws.input),
            )
        };
        if needs_plan
            && !self
                .run_mission(plan, MissionKind::Plan, drivers.plan.clone())
                .await
        {
            return false;
        }
        let pending: Vec<(MissionSpec, MissionKind)> = {
            let ws = self.ws.lock().await;
            ws.account
                .campaigns
                .iter()
                .filter(|c| !is_finished(&ws, &campaign_id(&c.slug)))
                .map(|c| {
                    (
                        campaign_mission(&ws.input, c),
                        MissionKind::Campaign {
                            slug: c.slug.clone(),
                        },
                    )
                })
                .collect()
        };
        let gate = Arc::new(Semaphore::new(parallel.max(1)));
        let mut set = JoinSet::new();
        for (spec, kind) in pending {
            let (runner, driver, gate) = (self.clone(), drivers.campaign.clone(), gate.clone());
            set.spawn(async move {
                let _permit = gate.acquire().await;
                runner.run_mission(spec, kind, driver).await
            });
        }
        let mut all_ok = true;
        while let Some(done) = set.join_next().await {
            all_ok &= done.unwrap_or(false);
        }
        all_ok
    }
}

#[derive(Serialize)]
struct RunJson<'a> {
    version: u32,
    run_id: &'a str,
    provider: &'a str,
    model: &'a Option<String>,
    status: &'static str,
    exit_code: i32,
    totals: &'a Totals,
}

fn totals_of(ws: &Workspace) -> Totals {
    let mut usage = Usage::default();
    ws.missions.values().for_each(|m| usage.add(&m.usage));
    Totals {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_usd: usage.cost_usd,
        missions: ws.missions.len(),
        failed_missions: ws
            .missions
            .values()
            .filter(|m| matches!(m.status, MissionStatus::Failed { .. }))
            .count(),
    }
}

fn today() -> String {
    format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .ok()
        .and_then(|f| OffsetDateTime::now_utc().format(&f).ok())
        .unwrap_or_default()
}

/// Runs the whole generation: missions, post-processing, files. Exit codes: 0 ok, 1 failure, 3 invalid account.
pub async fn generate(
    input: Input,
    drivers: Drivers,
    web: Arc<dyn Web>,
    cfg: RunConfig,
    events: EventSink,
) -> Result<RunResult, RunError> {
    let run = match &cfg.run_dir {
        Some(p) => RunDir::open(p)?,
        None => RunDir::create(&cfg.out_dir)?,
    };
    let ws_path = run.workspace_path();
    let workspace = if ws_path.exists() {
        Workspace::load(&ws_path)?
    } else {
        Workspace::new(input)
    };
    let ws: SharedWorkspace = Arc::new(Mutex::new(workspace));
    events.emit(Event::RunStarted {
        run_id: run.id(),
        run_dir: run.root().display().to_string(),
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
    });
    let runner = Arc::new(Runner {
        ws: ws.clone(),
        ws_path,
        transcripts: run.root().join("transcripts"),
        events: events.clone(),
        budget: Arc::new(TokenBudget::new(cfg.max_tokens)),
        max_turns: cfg.max_turns,
        mission_timeout: cfg.mission_timeout,
        mission_retries: cfg.mission_retries,
        max_ad_groups: cfg.max_ad_groups,
        image_model: drivers.image.is_some(),
    });
    let missions_ok = runner.run_all(&drivers, cfg.parallel).await;
    let finish = Conclusion {
        missions_ok,
        image: drivers.image.clone(),
    };
    conclude(&run, &ws, web.as_ref(), &cfg, &events, finish).await
}

/// Reruns validation, URL check and export on a finished run. No LLM involved.
pub async fn export_run(
    run_dir: &Path,
    web: &dyn Web,
    skip_url_check: bool,
    max_ad_groups: usize,
    layout: ExportLayout,
    events: &EventSink,
) -> Result<RunResult, RunError> {
    let run = RunDir::open(run_dir)?;
    let workspace = Workspace::load(&run.workspace_path())?;
    let unfinished: Vec<&str> = workspace
        .missions
        .iter()
        .filter(|(_, m)| m.status != MissionStatus::Finished)
        .map(|(id, _)| id.as_str())
        .collect();
    if workspace.missions.is_empty() || !unfinished.is_empty() {
        return Err(RunError::Unfinished(unfinished.join(", ")));
    }
    let previous: serde_json::Value = std::fs::read(run.run_json_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::Value::Null);
    let text = |key: &str| {
        previous
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(String::from)
    };
    let mut cfg = RunConfig::new(run.root().parent().unwrap_or(Path::new(".")).to_path_buf());
    cfg.provider = text("provider").unwrap_or_default();
    cfg.model = text("model");
    cfg.skip_url_check = skip_url_check;
    cfg.max_ad_groups = max_ad_groups;
    cfg.layout = layout;
    events.emit(Event::RunStarted {
        run_id: run.id(),
        run_dir: run.root().display().to_string(),
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
    });
    let ws: SharedWorkspace = Arc::new(Mutex::new(workspace));
    let finish = Conclusion {
        missions_ok: true,
        image: None,
    };
    conclude(&run, &ws, web, &cfg, events, finish).await
}

/// A failed export must not leave CSVs from an earlier, different account next to the new report.
/// Images stay: they cost money and the next export reuses them. An `editor/` directory that
/// only held `account.csv` is removed, so a bulk run re-exported as drive-folders does not
/// leave an empty folder behind.
fn remove_stale_csvs(dir: &Path) {
    remove_csvs_in(dir);
    let editor = dir.join(EDITOR_DIR);
    remove_csvs_in(&editor);
    remove_dir_if_empty(&editor);
    let drive = dir.join("drive");
    if drive.exists() {
        let _ = std::fs::remove_dir_all(drive);
    }
}

fn remove_dir_if_empty(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    if entries.flatten().next().is_none() {
        let _ = std::fs::remove_dir(dir);
    }
}

fn remove_csvs_in(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "csv") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// How the missions ended and what can still make images.
struct Conclusion {
    missions_ok: bool,
    image: Option<Arc<dyn ImageModel>>,
}

/// Makes the missing images. A run without image campaigns returns at once.
async fn images_step(
    run: &RunDir,
    ws: &SharedWorkspace,
    web: &dyn Web,
    cfg: &RunConfig,
    events: &EventSink,
    image: Option<Arc<dyn ImageModel>>,
) -> ImageStepResult {
    let step_cfg = ImageStepConfig {
        dir: run.platform_dir().join(EDITOR_DIR),
        workspace_path: Some(run.workspace_path()),
        model: image,
        max_new: cfg.max_images,
        parallel: cfg.parallel,
    };
    run_image_step(ws, web, &step_cfg, events).await
}

async fn conclude(
    run: &RunDir,
    ws: &SharedWorkspace,
    web: &dyn Web,
    cfg: &RunConfig,
    events: &EventSink,
    finish: Conclusion,
) -> Result<RunResult, RunError> {
    let mut notes = Vec::new();
    let mut images = ImageStepResult::default();
    let fin = if finish.missions_ok {
        events.emit(Event::Step {
            name: "cross-negatives".into(),
            detail: "brand and competitor terms".into(),
        });
        {
            let mut guard = ws.lock().await;
            let w: &mut Workspace = &mut guard;
            notes = cross_negatives(&mut w.account, &w.input.business);
            w.save(&run.workspace_path())?;
        };
        images = images_step(run, ws, web, cfg, events, finish.image).await;
        notes.extend(images.notes.iter().cloned());
        let (input, account, live) = {
            let w = ws.lock().await;
            (w.input.clone(), w.account.clone(), w.live.clone())
        };
        finalize(
            &input,
            &account,
            live.as_ref(),
            web,
            ExportOpts {
                skip_url_check: cfg.skip_url_check,
                max_ad_groups: cfg.max_ad_groups,
                layout: cfg.layout,
            },
            events,
        )
        .await
    } else {
        Finalized {
            exit_code: 1,
            errors: Vec::new(),
            warnings: Vec::new(),
            urls: None,
            csv: Vec::new(),
        }
    };

    remove_stale_csvs(&run.platform_dir());
    for file in &fin.csv {
        let path = run.platform_dir().join(file.name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &file.bytes)?;
        events.emit(Event::ArtifactWritten {
            path: path.display().to_string(),
        });
    }
    let guard = ws.lock().await;
    guard.save(&run.workspace_path())?;
    let totals = totals_of(&guard);
    let status = match fin.exit_code {
        0 => ReportStatus::Success,
        3 => ReportStatus::Invalid,
        _ => ReportStatus::Incomplete,
    };
    let report = ReportData {
        run_id: run.id(),
        date: today(),
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
        input: guard.input.clone(),
        account: guard.account.clone(),
        errors: fin.errors,
        warnings: fin.warnings,
        notes,
        urls: fin.urls,
        missions: guard.missions.clone(),
        totals: totals.clone(),
        status,
        images,
        live: guard.live.clone(),
        layout: cfg.layout,
    };
    std::fs::write(run.report_path(), render_report(&report))?;
    events.emit(Event::ArtifactWritten {
        path: run.report_path().display().to_string(),
    });
    let status_name = match status {
        ReportStatus::Success => "success",
        ReportStatus::Invalid => "invalid",
        ReportStatus::Incomplete => "incomplete",
    };
    let json = RunJson {
        version: 1,
        run_id: &report.run_id,
        provider: &cfg.provider,
        model: &cfg.model,
        status: status_name,
        exit_code: fin.exit_code,
        totals: &totals,
    };
    std::fs::write(
        run.run_json_path(),
        serde_json::to_vec_pretty(&json).map_err(io::Error::other)?,
    )?;
    events.emit(Event::RunFinished {
        ok: fin.exit_code == 0,
        exit_code: fin.exit_code,
        totals: totals.clone(),
    });
    Ok(RunResult {
        run_dir: run.root().to_path_buf(),
        exit_code: fin.exit_code,
        totals,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        agent::{DriverCtx, MissionOutcome, MissionReport, MissionSpec, ScriptedDriver},
        events::Event,
        google::ExportLayout,
        testutil,
        tools::ToolHost,
        usage::Usage,
        workspace::MissionStatus,
    };

    struct FakeWeb(HashMap<String, u16>);

    #[async_trait]
    impl Web for FakeWeb {
        async fn check_url(&self, url: &str) -> Result<u16, String> {
            Ok(self.0.get(url).copied().unwrap_or(200))
        }
    }

    fn texts(prefix: &str, n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("{prefix} numero {i}")).collect()
    }

    fn resp(calls: Vec<(&str, Value)>) -> Value {
        let tool_calls: Vec<Value> = calls
            .into_iter()
            .enumerate()
            .map(|(i, (n, a))| json!({"id": format!("c{i}"), "name": n, "arguments": a}))
            .collect();
        json!({"text": null, "tool_calls": tool_calls, "usage": {"input_tokens": 100, "output_tokens": 0, "cost_usd": null}})
    }

    fn idle() -> Value {
        json!({"text": "hmm", "tool_calls": [], "usage": {"input_tokens": 10, "output_tokens": 0, "cost_usd": null}})
    }

    fn plan_script() -> Value {
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

    fn ad_group(name: &str, variants: &[&str]) -> Value {
        json!({"name": name, "default_cpc": 1.5, "cpc_rationale": "estimativa",
               "keywords": {"variants": variants, "modifiers": ["review"], "exact_heads": true},
               "negatives": [{"text": "emprego", "match": "phrase"}],
               "rsa": {"headlines": texts("Titulo do grupo", 5), "descriptions": ["Descricao do grupo para teste"], "path1": "vinhos", "path2": ""}})
    }

    fn assets() -> Value {
        let sitelinks: Vec<Value> = ["app", "sobre", "blog", "ajuda"]
            .iter()
            .map(
                |p| json!({"text": format!("Link {p}"), "url": format!("https://vinellu.com/{p}")}),
            )
            .collect();
        json!({"sitelinks": sitelinks, "callouts": texts("Callout", 4), "snippets": [{"header": "types", "values": texts("Tipo", 3)}]})
    }

    fn catalog_script() -> Value {
        resp(vec![
            (
                "upsert_ad_group",
                ad_group("alamos", &["alamos malbec", "alamos"]),
            ),
            ("upsert_ad_group", ad_group("luigi", &["luigi bosca"])),
            ("set_assets", assets()),
            ("finish", json!({})),
        ])
    }

    fn brand_script() -> Value {
        resp(vec![
            (
                "upsert_ad_group",
                ad_group("marca", &["vinellu", "vinellu app"]),
            ),
            ("set_assets", assets()),
            ("finish", json!({})),
        ])
    }

    fn driver(missions: Value) -> Arc<dyn Driver> {
        Arc::new(ScriptedDriver::from_json(&json!({"missions": missions}).to_string()).unwrap())
    }

    fn full_script() -> Arc<dyn Driver> {
        driver(
            json!({"plan": [plan_script()], "campaign:vinellu-catalogo": [catalog_script()], "campaign:vinellu-marca": [brand_script()]}),
        )
    }

    fn drivers(d: Arc<dyn Driver>) -> Drivers {
        Drivers {
            plan: d.clone(),
            campaign: d,
            image: None,
        }
    }

    fn web(dead: &[&str]) -> Arc<dyn Web> {
        Arc::new(FakeWeb(
            dead.iter().map(|u| ((*u).to_string(), 404)).collect(),
        ))
    }

    fn cfg(dir: &tempfile::TempDir) -> RunConfig {
        let mut c = RunConfig::new(dir.path().to_path_buf());
        c.provider = "replay".into();
        c.mission_retries = 0;
        c
    }

    async fn run(cfg: RunConfig, d: Arc<dyn Driver>, w: Arc<dyn Web>) -> (RunResult, Vec<Event>) {
        let (events, mut rx) = EventSink::channel();
        let result = generate(testutil::input(), drivers(d), w, cfg, events)
            .await
            .unwrap();
        let mut all = Vec::new();
        while let Ok(e) = rx.try_recv() {
            all.push(e.event);
        }
        (result, all)
    }

    #[tokio::test]
    async fn happy_path_writes_csvs_report_and_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let (r, events) = run(cfg(&dir), full_script(), web(&[])).await;
        assert_eq!(r.exit_code, 0, "{events:#?}");
        for f in [
            "1-campaign.csv",
            "2-ad-groups.csv",
            "3-keywords.csv",
            "4-negative-keywords.csv",
            "5-responsive-search-ads.csv",
        ] {
            assert!(r.run_dir.join(PLATFORM_DIR).join(f).exists(), "{f}");
        }
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(report.contains("Ready to import") && report.contains("Vinellu - Catalogo"));
        let ws = Workspace::load(&r.run_dir.join("workspace.json")).unwrap();
        assert!(
            ws.missions
                .values()
                .all(|m| m.status == MissionStatus::Finished)
        );
        assert_eq!(ws.missions.len(), 3);
        assert!(r.run_dir.join("run.json").exists());
        assert!(matches!(events.first(), Some(Event::RunStarted { .. })));
        assert!(matches!(
            events.last(),
            Some(Event::RunFinished {
                ok: true,
                exit_code: 0,
                ..
            })
        ));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::MissionStarted { .. }))
                .count(),
            3
        );
        assert_eq!(r.totals.missions, 3);
        assert_eq!(r.totals.input_tokens, 300);
    }

    #[tokio::test]
    async fn cross_negatives_land_in_the_exported_negatives_file() {
        let dir = tempfile::tempdir().unwrap();
        let (r, _) = run(cfg(&dir), full_script(), web(&[])).await;
        let negatives =
            std::fs::read_to_string(r.run_dir.join("google-ads/4-negative-keywords.csv")).unwrap();
        assert!(
            negatives.contains("Vinellu - Catalogo,alamos,vinellu,Phrase match"),
            "{negatives}"
        );
        assert!(negatives.contains("vivino"));
        assert!(
            !negatives.contains("Vinellu - Marca,marca,vinellu,"),
            "the brand campaign must not exclude its own brand"
        );
    }

    #[tokio::test]
    async fn a_failed_campaign_makes_the_run_incomplete_and_resume_finishes_it() {
        let dir = tempfile::tempdir().unwrap();
        let partial = driver(
            json!({"plan": [plan_script()], "campaign:vinellu-catalogo": [catalog_script()]}),
        );
        let (first, events) = run(cfg(&dir), partial, web(&[])).await;
        assert_eq!(first.exit_code, 1);
        assert!(!first.run_dir.join("google-ads/1-campaign.csv").exists());
        let report = std::fs::read_to_string(first.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("Incomplete")
                && report.contains("campaign:vinellu-marca")
                && report.contains("--resume")
        );
        assert!(events.iter().any(|e| matches!(e, Event::MissionFinished { mission, ok: false, .. } if mission == "campaign:vinellu-marca")));

        let mut resume = cfg(&dir);
        resume.run_dir = Some(first.run_dir.clone());
        let only_marca = driver(json!({"campaign:vinellu-marca": [brand_script()]}));
        let (second, _) = run(resume, only_marca, web(&[])).await;
        assert_eq!(
            second.exit_code, 0,
            "plan and catalog were finished, so only marca needs a script"
        );
        assert_eq!(second.run_dir, first.run_dir);
        assert!(second.run_dir.join("google-ads/1-campaign.csv").exists());
    }

    #[tokio::test]
    async fn a_dead_url_gives_exit_3_and_no_csv() {
        let dir = tempfile::tempdir().unwrap();
        let (r, _) = run(
            cfg(&dir),
            full_script(),
            web(&["https://vinellu.com/w/alamos"]),
        )
        .await;
        assert_eq!(r.exit_code, 3);
        assert!(!r.run_dir.join("google-ads/1-campaign.csv").exists());
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(report.contains("Not exported") && report.contains("E15"));
    }

    #[tokio::test]
    async fn skip_url_check_ignores_dead_urls() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = cfg(&dir);
        c.skip_url_check = true;
        let (r, _) = run(c, full_script(), web(&["https://vinellu.com/w/alamos"])).await;
        assert_eq!(r.exit_code, 0);
    }

    #[tokio::test]
    async fn a_mission_is_retried_from_the_partial_state() {
        let dir = tempfile::tempdir().unwrap();
        let brand = driver(json!({
            "plan": [plan_script()],
            "campaign:vinellu-catalogo": [catalog_script()],
            "campaign:vinellu-marca": [idle(), idle(), idle(), brand_script()]
        }));
        let mut c = cfg(&dir);
        c.mission_retries = 1;
        let (r, _) = run(c, brand, web(&[])).await;
        assert_eq!(r.exit_code, 0);
        let ws = Workspace::load(&r.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.missions["campaign:vinellu-marca"].attempts, 2);
        assert_eq!(ws.missions["campaign:vinellu-catalogo"].attempts, 1);
    }

    #[tokio::test]
    async fn the_token_budget_stops_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = cfg(&dir);
        c.max_tokens = 150;
        let (r, _) = run(c, full_script(), web(&[])).await;
        assert_eq!(r.exit_code, 1);
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(report.contains("Incomplete") && report.contains("token budget"));
    }

    struct SlowDriver;

    #[async_trait]
    impl Driver for SlowDriver {
        async fn run_mission(
            &self,
            _: &MissionSpec,
            _: Arc<dyn ToolHost>,
            _: &DriverCtx,
        ) -> MissionReport {
            tokio::time::sleep(Duration::from_secs(5)).await;
            MissionReport {
                outcome: MissionOutcome::Finished,
                usage: Usage::default(),
                turns: 0,
            }
        }
    }

    #[tokio::test]
    async fn a_mission_that_exceeds_the_timeout_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = cfg(&dir);
        c.mission_timeout = Duration::from_millis(20);
        let (r, _) = run(c, Arc::new(SlowDriver), web(&[])).await;
        assert_eq!(r.exit_code, 1);
        let ws = Workspace::load(&r.run_dir.join("workspace.json")).unwrap();
        assert!(
            matches!(&ws.missions["plan"].status, MissionStatus::Failed { reason } if reason.contains("timeout"))
        );
    }

    #[tokio::test]
    async fn a_failed_plan_skips_the_campaigns() {
        let dir = tempfile::tempdir().unwrap();
        let (r, events) = run(cfg(&dir), driver(json!({})), web(&[])).await;
        assert_eq!(r.exit_code, 1);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::MissionStarted { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn run_dir_names_are_sortable_and_subdirs_exist() {
        let dir = tempfile::tempdir().unwrap();
        let run = RunDir::create(dir.path()).unwrap();
        let name = run.id();
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 3, "{name}");
        assert!(parts[0].len() == 8 && parts[0].chars().all(|c| c.is_ascii_digit()));
        assert!(parts[1].len() == 6 && parts[1].chars().all(|c| c.is_ascii_digit()));
        assert!(parts[2].len() == 6 && parts[2].chars().all(|c| c.is_ascii_hexdigit()));
        for sub in ["input", "transcripts", "google-ads"] {
            assert!(run.root().join(sub).is_dir(), "{sub}");
        }
    }

    #[test]
    fn opening_a_missing_run_dir_is_an_error() {
        assert!(RunDir::open(std::path::Path::new("/nonexistent/run")).is_err());
    }

    #[tokio::test]
    async fn export_run_reruns_post_processing_without_any_driver() {
        let dir = tempfile::tempdir().unwrap();
        let (first, _) = run(
            cfg(&dir),
            full_script(),
            web(&["https://vinellu.com/w/alamos"]),
        )
        .await;
        assert_eq!(first.exit_code, 3);
        let (events, _rx) = EventSink::channel();
        let second = export_run(
            &first.run_dir,
            web(&["https://vinellu.com/w/alamos"]).as_ref(),
            true,
            50,
            ExportLayout::Bulk,
            &events,
        )
        .await
        .unwrap();
        assert_eq!(second.exit_code, 0);
        assert!(
            second
                .run_dir
                .join("google-ads/5-responsive-search-ads.csv")
                .exists()
        );
        let report = std::fs::read_to_string(second.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("Ready to import") && report.contains("replay"),
            "provider is kept from run.json"
        );
    }

    #[tokio::test]
    async fn export_run_refuses_a_run_with_unfinished_missions() {
        let dir = tempfile::tempdir().unwrap();
        let partial = driver(
            json!({"plan": [plan_script()], "campaign:vinellu-catalogo": [catalog_script()]}),
        );
        let (first, _) = run(cfg(&dir), partial, web(&[])).await;
        let (events, _rx) = EventSink::channel();
        let err = export_run(
            &first.run_dir,
            web(&[]).as_ref(),
            true,
            50,
            ExportLayout::Bulk,
            &events,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("unfinished") && err.to_string().contains("--resume"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn export_run_on_a_missing_directory_fails() {
        let (events, _rx) = EventSink::channel();
        assert!(
            export_run(
                std::path::Path::new("/nonexistent/run"),
                web(&[]).as_ref(),
                true,
                50,
                ExportLayout::Bulk,
                &events
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn a_failed_export_removes_csvs_left_by_an_earlier_one() {
        let dir = tempfile::tempdir().unwrap();
        let (first, _) = run(cfg(&dir), full_script(), web(&[])).await;
        assert_eq!(first.exit_code, 0);
        assert!(first.run_dir.join("google-ads/1-campaign.csv").exists());
        let ws_path = first.run_dir.join("workspace.json");
        let mut ws = Workspace::load(&ws_path).unwrap();
        ws.account.campaigns[0].daily_budget = crate::money::Cents(100);
        ws.save(&ws_path).unwrap();
        let (events, _rx) = EventSink::channel();
        let again = export_run(
            &first.run_dir,
            web(&[]).as_ref(),
            true,
            50,
            ExportLayout::Bulk,
            &events,
        )
        .await
        .unwrap();
        assert_eq!(again.exit_code, 3);
        for f in ["1-campaign.csv", "5-responsive-search-ads.csv"] {
            assert!(
                !first.run_dir.join(PLATFORM_DIR).join(f).exists(),
                "{f} is stale and must be removed"
            );
        }
    }

    #[tokio::test]
    async fn export_run_honors_the_ad_group_limit() {
        let dir = tempfile::tempdir().unwrap();
        let (first, _) = run(cfg(&dir), full_script(), web(&[])).await;
        let (events, _rx) = EventSink::channel();
        let limited = export_run(
            &first.run_dir,
            web(&[]).as_ref(),
            true,
            1,
            ExportLayout::Bulk,
            &events,
        )
        .await
        .unwrap();
        assert_eq!(limited.exit_code, 3, "3 ad groups exceed a limit of 1");
    }

    #[test]
    fn io_errors_display_the_path_once() {
        let e = RunError::from(io::Error::new(
            io::ErrorKind::NotFound,
            "run directory not found: /x",
        ));
        let chain = format!("{:#}", anyhow_like(&e));
        assert_eq!(chain.matches("/x").count(), 1, "{chain}");
    }

    /// `{:#}` in anyhow prints the error followed by every `source()`.
    fn anyhow_like(e: &dyn std::error::Error) -> String {
        let mut out = e.to_string();
        let mut cur = e.source();
        while let Some(s) = cur {
            out.push_str(": ");
            out.push_str(&s.to_string());
            cur = s.source();
        }
        out
    }
}

#[cfg(test)]
mod image_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        agent::ScriptedDriver,
        events::Event,
        images::{SolidImageModel, solid_png},
        input::Input,
        testutil,
    };

    struct OkWeb;

    #[async_trait]
    impl Web for OkWeb {
        async fn check_url(&self, _url: &str) -> Result<u16, String> {
            Ok(200)
        }

        async fn fetch_bytes(&self, _url: &str, _limit: usize) -> Result<Vec<u8>, String> {
            Ok(solid_png(crate::google::AspectRatio::Square, [200, 10, 10]))
        }
    }

    fn texts(prefix: &str, n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("{prefix} numero {i}")).collect()
    }

    fn resp(calls: Vec<(&str, Value)>) -> Value {
        let tool_calls: Vec<Value> = calls
            .into_iter()
            .enumerate()
            .map(|(i, (n, a))| json!({"id": format!("c{i}"), "name": n, "arguments": a}))
            .collect();
        json!({"text": null, "tool_calls": tool_calls, "usage": {"input_tokens": 100, "output_tokens": 0, "cost_usd": null}})
    }

    fn script() -> Arc<dyn Driver> {
        let kit = json!({"headlines": texts("Titulo da marca", 10), "descriptions": texts("Descricao da marca com chamada pra acao", 3)});
        let plan = json!({"campaigns": [
            {"name": "Vinellu - Feed", "kind": "demand_gen", "intent": "generic", "daily_budget": 50.0,
             "bid_strategy": {"type": "maximize_clicks"}, "rationale": "criar demanda",
             "ad_groups": [{"name": "tintos", "theme": "tintos", "entity_ids": ["alamos-malbec"]}]}
        ]});
        let prompt = "Photo of friends sharing red wine at a dinner table, warm light";
        let group = json!({"name": "tintos", "business_name": "Vinellu", "headlines": texts("Titulo", 3), "descriptions": ["Reviews reais de vinhos"]});
        let briefs = json!({"asset_group": "tintos", "images": [
            {"id": "jantar", "ratio": "landscape", "prompt": prompt},
            {"id": "garrafa", "ratio": "square", "prompt": prompt, "reference": "alamos-malbec"},
            {"id": "story", "ratio": "vertical", "prompt": prompt}
        ]});
        let missions = json!({
            "plan": [resp(vec![("set_brand_kit", kit), ("set_account_plan", plan), ("finish", json!({}))])],
            "campaign:vinellu-feed": [resp(vec![("upsert_asset_group", group), ("set_image_briefs", briefs), ("finish", json!({}))])],
        });
        Arc::new(ScriptedDriver::from_json(&json!({"missions": missions}).to_string()).unwrap())
    }

    fn input(dir: &std::path::Path) -> Input {
        let logo = dir.join("logo.png");
        let png =
            crate::images::prepare_logo(&solid_png(crate::google::AspectRatio::Square, [1, 2, 3]))
                .unwrap();
        std::fs::write(&logo, png).unwrap();
        let mut i = testutil::input();
        i.logo = Some(logo.display().to_string());
        i.catalog[0].image = Some("https://cdn.vinellu.com/alamos.jpg".into());
        i
    }

    async fn generate_images(
        dir: &tempfile::TempDir,
        max_images: usize,
    ) -> (RunResult, Vec<Event>) {
        let mut cfg = RunConfig::new(dir.path().join("out"));
        cfg.provider = "replay".into();
        cfg.mission_retries = 0;
        cfg.max_images = max_images;
        let d = script();
        let drivers = Drivers {
            plan: d.clone(),
            campaign: d,
            image: Some(Arc::new(SolidImageModel)),
        };
        let (events, mut rx) = EventSink::channel();
        let r = generate(input(dir.path()), drivers, Arc::new(OkWeb), cfg, events)
            .await
            .unwrap();
        let mut all = Vec::new();
        while let Ok(e) = rx.try_recv() {
            all.push(e.event);
        }
        (r, all)
    }

    #[tokio::test]
    async fn an_image_campaign_gets_its_pictures_and_the_editor_csv() {
        let dir = tempfile::tempdir().unwrap();
        let (r, events) = generate_images(&dir, 40).await;
        assert_eq!(r.exit_code, 0, "{events:#?}");
        let editor = r.run_dir.join("google-ads/editor");
        for f in [
            "account.csv",
            "images/logo.png",
            "images/vinellu-feed/tintos-jantar.jpg",
            "images/vinellu-feed/tintos-garrafa.jpg",
            "images/vinellu-feed/tintos-story.jpg",
        ] {
            assert!(editor.join(f).is_file(), "missing {f}");
        }
        assert!(
            !r.run_dir.join("google-ads/1-campaign.csv").exists(),
            "no search campaign, no bulk files"
        );
        let rows = crate::google::read_editor(&std::fs::read(editor.join("account.csv")).unwrap())
            .unwrap();
        let csv: String = rows
            .iter()
            .flat_map(|r| r.values().cloned())
            .collect::<Vec<_>>()
            .join("|");
        assert!(
            csv.contains("Demand Gen") && !csv.contains(".jpg"),
            "pictures stay out of the file"
        );
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("`editor/images/vinellu-feed/tintos-story.jpg`"),
            "{report}"
        );
        assert!(report.contains("## Images") && report.contains("Model solid: 3 generated"));
        assert!(
            !report.contains("real import yet"),
            "Demand Gen went through a real Editor import: no warning"
        );
    }

    #[tokio::test]
    async fn export_reuses_the_images_and_fails_with_e20_when_one_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let (r, _) = generate_images(&dir, 40).await;
        let (events, _rx) = EventSink::channel();
        let again = export_run(&r.run_dir, &OkWeb, true, 50, ExportLayout::Bulk, &events)
            .await
            .unwrap();
        assert_eq!(again.exit_code, 0);
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("0 generated in this run, 3 reused"),
            "{report}"
        );

        std::fs::remove_file(
            r.run_dir
                .join("google-ads/editor/images/vinellu-feed/tintos-jantar.jpg"),
        )
        .unwrap();
        let broken = export_run(&r.run_dir, &OkWeb, true, 50, ExportLayout::Bulk, &events)
            .await
            .unwrap();
        assert_eq!(broken.exit_code, 3);
        assert!(
            !r.run_dir.join("google-ads/editor/account.csv").exists(),
            "stale csv removed"
        );
        assert!(
            r.run_dir
                .join("google-ads/editor/images/vinellu-feed/tintos-story.jpg")
                .exists(),
            "images kept"
        );
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(report.contains("E20"), "{report}");
    }

    #[tokio::test]
    async fn a_brief_over_the_cap_is_dropped_when_the_group_can_do_without_it() {
        let dir = tempfile::tempdir().unwrap();
        let (r, _) = generate_images(&dir, 2).await;
        assert_eq!(
            r.exit_code, 0,
            "landscape and square are enough for Demand Gen"
        );
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("story: dropped, over the limit of 2 new images"),
            "{report}"
        );
        assert!(report.contains("2 generated"));
        assert!(
            !report.contains("tintos-story.jpg"),
            "a dropped brief is not in the Images table"
        );
    }

    #[tokio::test]
    async fn the_cap_stops_the_export_when_a_group_would_be_left_without_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let (r, _) = generate_images(&dir, 0).await;
        assert_eq!(r.exit_code, 3);
        let report = std::fs::read_to_string(r.run_dir.join("report.md")).unwrap();
        assert!(
            report.contains("jantar: over the limit of 0 new images"),
            "{report}"
        );
        assert!(report.contains("E20"));
    }

    #[test]
    fn remove_stale_csvs_drops_an_empty_editor_dir_and_keeps_images() {
        let dir = tempfile::tempdir().unwrap();
        let platform = dir.path().join("google-ads");
        let editor = platform.join("editor");
        std::fs::create_dir_all(&editor).unwrap();
        std::fs::write(editor.join("account.csv"), b"x").unwrap();
        remove_stale_csvs(&platform);
        assert!(
            !editor.exists(),
            "an editor directory that only held the csv is removed"
        );

        let image = editor.join("images/vinellu-feed/tintos-story.jpg");
        std::fs::create_dir_all(image.parent().unwrap()).unwrap();
        std::fs::write(editor.join("account.csv"), b"x").unwrap();
        std::fs::write(&image, b"jpg").unwrap();
        remove_stale_csvs(&platform);
        assert!(!editor.join("account.csv").exists());
        assert!(image.is_file(), "pictures stay");
        assert!(editor.is_dir());
    }
}
