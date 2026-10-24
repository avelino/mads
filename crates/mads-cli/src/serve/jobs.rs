use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use fs2::FileExt;
use mads_core::{
    agent::RunControl,
    events::{Event, EventEnvelope, EventSink},
    google::ExportLayout,
    init::{InitConfig, run_init},
    input::{Input, load_input, parse_input_toml},
    money::Cents,
    run::{RunConfig, RunDir, export_run, generate},
    workspace::Workspace,
};
use mads_providers::{
    ProviderSelection, SiteClient, WebClient, build_drivers, build_image_model_with_keys, preflight,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::serve::{
    revisions::{self, Edit, RevisionError},
    store::Project,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Cancelling,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn label(self) -> &'static str {
        match self {
            JobState::Running => "Em execução",
            JobState::Cancelling => "Parando",
            JobState::Done => "Concluído",
            JobState::Failed => "Falhou",
            JobState::Cancelled => "Cancelado",
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            JobState::Running => "running",
            JobState::Cancelling => "warn",
            JobState::Done => "ok",
            JobState::Failed => "error",
            JobState::Cancelled => "muted",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub kind: String,
    pub state: JobState,
    pub state_label: String,
    pub state_class: String,
    pub run_id: Option<String>,
    pub run_id_text: String,
    pub has_run: bool,
    pub error: Option<String>,
    pub error_text: String,
    pub has_error: bool,
    pub started: String,
    pub finished: Option<String>,
    pub finished_text: String,
    pub has_finished: bool,
}

struct Job {
    id: String,
    project_id: String,
    project_name: String,
    project_dir: PathBuf,
    kind: String,
    state: Mutex<JobState>,
    run_id: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
    started: String,
    finished: Mutex<Option<String>>,
    control: RunControl,
    tx: broadcast::Sender<EventEnvelope>,
    log: PathBuf,
    _lock: Mutex<Option<File>>,
}

impl Job {
    fn view(&self) -> JobView {
        let state = self.state.lock().map(|s| *s).unwrap_or(JobState::Failed);
        let run_id = self.run_id.lock().ok().and_then(|r| r.clone());
        let error = self.error.lock().ok().and_then(|e| e.clone());
        let finished = self.finished.lock().ok().and_then(|f| f.clone());
        JobView {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            project_name: self.project_name.clone(),
            kind: self.kind.clone(),
            state,
            state_label: state.label().to_string(),
            state_class: state.class().to_string(),
            run_id_text: run_id.clone().unwrap_or_default(),
            has_run: run_id.is_some(),
            run_id,
            error_text: error.clone().unwrap_or_default(),
            has_error: error.is_some(),
            error,
            started: self.started.clone(),
            finished_text: finished.clone().unwrap_or_default(),
            has_finished: finished.is_some(),
            finished,
        }
    }

    fn finish(&self, state: JobState, error: Option<String>) {
        crate::serve::audit::append(
            &self.project_dir,
            "job_finished",
            &format!("{} {} {:?}", self.id, self.kind, state),
        );
        if let Ok(mut current) = self.state.lock() {
            *current = state;
        }
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
        if let Ok(mut slot) = self.finished.lock() {
            *slot = Some(clock());
        }
        if let Ok(mut lock) = self._lock.lock() {
            *lock = None;
        }
    }
}

#[derive(Clone, Default)]
pub struct Jobs {
    inner: Arc<Mutex<HashMap<String, Arc<Job>>>>,
}

impl Jobs {
    pub fn get(&self, id: &str) -> Option<JobView> {
        self.inner.lock().ok()?.get(id).map(|job| job.view())
    }

    pub fn views(&self) -> Vec<JobView> {
        let Ok(map) = self.inner.lock() else {
            return Vec::new();
        };
        let mut jobs: Vec<_> = map.values().map(|job| job.view()).collect();
        jobs.sort_by(|a, b| b.started.cmp(&a.started));
        jobs.truncate(100);
        jobs
    }

    pub fn cancel(&self, id: &str) -> bool {
        let Some(job) = self.inner.lock().ok().and_then(|map| map.get(id).cloned()) else {
            return false;
        };
        job.control.cancel();
        if let Ok(mut state) = job.state.lock()
            && *state == JobState::Running
        {
            *state = JobState::Cancelling;
        }
        true
    }

    pub fn subscribe(&self, id: &str) -> Option<(broadcast::Receiver<EventEnvelope>, PathBuf)> {
        let job = self.inner.lock().ok()?.get(id).cloned()?;
        Some((job.tx.subscribe(), job.log.clone()))
    }

    pub fn active(&self, project_id: &str) -> bool {
        self.inner.lock().is_ok_and(|map| {
            map.values().any(|job| {
                job.project_id == project_id
                    && matches!(
                        *job.state.lock().unwrap_or_else(|e| e.into_inner()),
                        JobState::Running | JobState::Cancelling
                    )
            })
        })
    }

    pub fn start_generate(
        &self,
        project: &Project,
        project_dir: &Path,
        options: GenerateOptions,
    ) -> Result<JobView, String> {
        if self.active(&project.id) {
            return Err("este projeto já tem uma execução em andamento".into());
        }
        if !project_dir.join("business.toml").is_file() {
            return Err(
                "o projeto não tem business.toml. Estude o site ou crie o rascunho antes de gerar."
                    .into(),
            );
        }
        let mut input = load_input(&project_dir.join("business.toml"))
            .map_err(|e| format!("o rascunho não carrega: {e}"))?;
        let run = prepare_run(project_dir, &mut input)?;
        let run_id = run.id();
        let log = run.root().join("events.ndjson");
        let lock = lock_project(project_dir)?;
        let job = new_job(
            project_dir,
            &project.id,
            &project.name,
            "generate",
            Some(run_id.clone()),
            log,
            Some(lock),
            options.control.clone(),
        )?;
        self.insert(job.clone());
        crate::serve::audit::append(project_dir, "job_started", &format!("generate {run_id}"));
        let job_for_task = job.clone();
        let project_dir = project_dir.to_path_buf();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                job_for_task.finish(
                    JobState::Failed,
                    Some("cannot start the job runtime".into()),
                );
                return;
            };
            runtime.block_on(run_generate_job(
                job_for_task,
                project_dir,
                Some(run),
                input,
                options,
            ));
        });
        Ok(job.view())
    }

    pub fn start_resume(
        &self,
        project: &Project,
        project_dir: &Path,
        run_id: &str,
        mut options: GenerateOptions,
    ) -> Result<JobView, String> {
        options.plan_review = false;
        options.layout = None;
        if self.active(&project.id) {
            return Err("este projeto já tem uma execução em andamento".into());
        }
        let run_dir = project_dir.join("out").join(run_id);
        if !run_dir.join("workspace.json").is_file() {
            return Err("execução não encontrada".into());
        }
        let input = Workspace::load(&run_dir.join("workspace.json"))
            .map_err(|e| format!("não foi possível abrir a execução: {e}"))?
            .input;
        let log = run_dir.join("events.ndjson");
        let lock = lock_project(project_dir)?;
        let job = new_job(
            project_dir,
            &project.id,
            &project.name,
            "resume",
            Some(run_id.to_string()),
            log,
            Some(lock),
            options.control.clone(),
        )?;
        self.insert(job.clone());
        crate::serve::audit::append(project_dir, "job_started", &format!("resume {run_id}"));
        let job_for_task = job.clone();
        let project_dir = project_dir.to_path_buf();
        let run_id = run_id.to_string();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                job_for_task.finish(
                    JobState::Failed,
                    Some("cannot start the job runtime".into()),
                );
                return;
            };
            let run = RunDir::open(&project_dir.join("out").join(run_id)).ok();
            runtime.block_on(run_generate_job(
                job_for_task,
                project_dir,
                run,
                input,
                options,
            ));
        });
        Ok(job.view())
    }

    pub fn start_init(
        &self,
        project: &Project,
        project_dir: &Path,
        options: InitOptions,
    ) -> Result<JobView, String> {
        if self.active(&project.id) {
            return Err("este projeto já tem uma execução em andamento".into());
        }
        let job_id = Uuid::new_v4().simple().to_string();
        let job_dir = project_dir.join(".mads").join("jobs").join(&job_id);
        fs::create_dir_all(&job_dir).map_err(|e| e.to_string())?;
        let log = job_dir.join("events.ndjson");
        let lock = lock_project(project_dir)?;
        let job = new_job(
            project_dir,
            &project.id,
            &project.name,
            "init",
            None,
            log,
            Some(lock),
            options.control.clone(),
        )?;
        self.insert(job.clone());
        crate::serve::audit::append(project_dir, "job_started", &format!("init {}", job.id));
        let job_for_task = job.clone();
        let project_dir = project_dir.to_path_buf();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                job_for_task.finish(
                    JobState::Failed,
                    Some("cannot start the job runtime".into()),
                );
                return;
            };
            runtime.block_on(run_init_job(job_for_task, project_dir, options));
        });
        Ok(job.view())
    }

    pub fn start_revision(
        &self,
        project: &Project,
        project_dir: &Path,
        base_run_id: &str,
        edits: &[Edit],
    ) -> Result<(JobView, String), RevisionError> {
        if self.active(&project.id) {
            return Err(RevisionError::Message(
                "este projeto já tem uma execução em andamento".into(),
            ));
        }
        let base = project_dir.join("out").join(base_run_id);
        let workspace = Workspace::load(&base.join("workspace.json")).map_err(|e| {
            RevisionError::Message(format!("não foi possível abrir a execução: {e}"))
        })?;
        let account = revisions::apply(&workspace.account, &workspace.input, edits, 50)?;
        let mut workspace = workspace;
        workspace.account = account;
        let (run_id, _) = revisions::create(project_dir, base_run_id, &workspace, edits)
            .map_err(RevisionError::Message)?;
        crate::serve::audit::append(
            project_dir,
            "revision_created",
            &format!("{base_run_id} -> {run_id} ({} edits)", edits.len()),
        );
        let job = self
            .start_export(
                project,
                project_dir,
                &run_id,
                ExportOptions {
                    skip_url_check: false,
                    layout: None,
                    max_ad_groups: 50,
                },
            )
            .map_err(RevisionError::Message)?;
        Ok((job, run_id))
    }

    pub fn start_export(
        &self,
        project: &Project,
        project_dir: &Path,
        run_id: &str,
        options: ExportOptions,
    ) -> Result<JobView, String> {
        if self.active(&project.id) {
            return Err("este projeto já tem uma execução em andamento".into());
        }
        let run_dir = project_dir.join("out").join(run_id);
        if !run_dir.join("workspace.json").is_file() {
            return Err("execução não encontrada".into());
        }
        let log = run_dir.join("events.ndjson");
        let lock = lock_project(project_dir)?;
        let job = new_job(
            project_dir,
            &project.id,
            &project.name,
            "export",
            Some(run_id.to_string()),
            log,
            Some(lock),
            RunControl::default(),
        )?;
        self.insert(job.clone());
        crate::serve::audit::append(project_dir, "job_started", &format!("export {run_id}"));
        let job_for_task = job.clone();
        let project_dir = project_dir.to_path_buf();
        let run_id = run_id.to_string();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                job_for_task.finish(
                    JobState::Failed,
                    Some("cannot start the job runtime".into()),
                );
                return;
            };
            runtime.block_on(run_export_job(job_for_task, project_dir, run_id, options));
        });
        Ok(job.view())
    }

    fn insert(&self, job: Arc<Job>) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(job.id.clone(), job);
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateRequest {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub plan_model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub image_provider: Option<String>,
    #[serde(default)]
    pub image_model: Option<String>,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub parallel: Option<usize>,
    #[serde(default)]
    pub max_turns: Option<usize>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub retries: Option<u32>,
    #[serde(default)]
    pub max_ad_groups: Option<usize>,
    #[serde(default)]
    pub max_images: Option<usize>,
    #[serde(default)]
    pub skip_url_check: Option<bool>,
    #[serde(default = "default_plan_review")]
    pub plan_review: bool,
    /// Hidden replay provider script, for tests and offline demos.
    #[serde(default, skip_serializing)]
    pub script: Option<PathBuf>,
}

fn default_plan_review() -> bool {
    true
}

#[derive(Clone)]
pub struct GenerateOptions {
    pub provider: String,
    pub model: Option<String>,
    pub plan_model: Option<String>,
    pub base_url: Option<String>,
    pub image_provider: String,
    pub image_model: Option<String>,
    pub layout: Option<ExportLayout>,
    pub parallel: usize,
    pub max_turns: usize,
    pub timeout: Duration,
    pub max_tokens: u64,
    pub retries: u32,
    pub max_ad_groups: usize,
    pub max_images: usize,
    pub skip_url_check: bool,
    pub plan_review: bool,
    pub script: Option<PathBuf>,
    pub keys: std::collections::BTreeMap<String, String>,
    pub control: RunControl,
}

impl GenerateOptions {
    pub fn from_request(request: &GenerateRequest) -> Result<Self, String> {
        let timeout = match &request.timeout {
            Some(text) => crate::cli::parse_duration(text)?,
            None => Duration::from_secs(15 * 60),
        };
        let layout = match request.layout.as_deref() {
            None | Some("") | Some("bulk") => Some(ExportLayout::Bulk),
            Some("drive-folders") => Some(ExportLayout::DriveFolders),
            Some(other) => return Err(format!("layout desconhecido: {other}")),
        };
        if request.provider.trim().is_empty() {
            return Err("escolha um provedor".into());
        }
        if request.parallel.is_some_and(|p| p == 0) {
            return Err("parallel precisa ser maior que zero".into());
        }
        Ok(Self {
            provider: request.provider.trim().to_string(),
            model: clean(request.model.clone()),
            plan_model: clean(request.plan_model.clone()),
            base_url: clean(request.base_url.clone()),
            image_provider: request
                .image_provider
                .clone()
                .filter(|p| !p.trim().is_empty())
                .unwrap_or_else(|| "auto".into()),
            image_model: clean(request.image_model.clone()),
            layout,
            parallel: request.parallel.unwrap_or(4),
            max_turns: request.max_turns.unwrap_or(40),
            timeout,
            max_tokens: request.max_tokens.unwrap_or(4_000_000),
            retries: request.retries.unwrap_or(1),
            max_ad_groups: request.max_ad_groups.unwrap_or(50),
            max_images: request.max_images.unwrap_or(40),
            skip_url_check: request.skip_url_check.unwrap_or(false),
            plan_review: request.plan_review,
            script: request.script.clone(),
            keys: std::collections::BTreeMap::new(),
            control: RunControl::default(),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportRequest {
    #[serde(default)]
    pub skip_url_check: Option<bool>,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub max_ad_groups: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub skip_url_check: bool,
    pub layout: Option<ExportLayout>,
    pub max_ad_groups: usize,
}

impl ExportOptions {
    pub fn from_request(request: &ExportRequest) -> Result<Self, String> {
        let layout = match request.layout.as_deref() {
            None | Some("") => None,
            Some("bulk") => Some(ExportLayout::Bulk),
            Some("drive-folders") => Some(ExportLayout::DriveFolders),
            Some(other) => return Err(format!("layout desconhecido: {other}")),
        };
        Ok(Self {
            skip_url_check: request.skip_url_check.unwrap_or(false),
            layout,
            max_ad_groups: request.max_ad_groups.unwrap_or(50),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitMeta {
    pub start_url: String,
    pub daily: f64,
    pub currency: String,
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_catalog_limit")]
    pub catalog_limit: usize,
    #[serde(default = "default_true")]
    pub web_search: bool,
    #[serde(default)]
    pub focus: bool,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_turns")]
    pub max_turns: usize,
    #[serde(default = "default_timeout")]
    pub timeout: String,
    #[serde(default = "default_tokens")]
    pub max_tokens: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

impl Default for InitMeta {
    fn default() -> Self {
        Self {
            start_url: String::new(),
            daily: 50.0,
            currency: "BRL".into(),
            provider: String::new(),
            model: None,
            base_url: None,
            catalog_limit: default_catalog_limit(),
            web_search: true,
            focus: false,
            force: false,
            max_turns: default_turns(),
            timeout: default_timeout(),
            max_tokens: default_tokens(),
            retries: default_retries(),
        }
    }
}

impl InitMeta {
    pub fn options(
        &self,
        keys: std::collections::BTreeMap<String, String>,
    ) -> Result<InitOptions, String> {
        if mads_core::input::normalize_url(&self.start_url).is_none() {
            return Err("a URL precisa ser http(s)".into());
        }
        let daily = Cents::from_f64(self.daily).ok_or_else(|| {
            "o orçamento precisa ser maior que zero, com até 2 decimais".to_string()
        })?;
        let currency = self.currency.trim().to_uppercase();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            return Err("a moeda precisa de 3 letras, como BRL".into());
        }
        let timeout = crate::cli::parse_duration(&self.timeout)?;
        if self.provider.trim().is_empty() {
            return Err("escolha um provedor".into());
        }
        Ok(InitOptions {
            start_url: self.start_url.trim().to_string(),
            daily,
            currency,
            provider: self.provider.trim().to_string(),
            model: self
                .model
                .clone()
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty()),
            base_url: self
                .base_url
                .clone()
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty()),
            catalog_limit: self.catalog_limit,
            web_search: self.web_search,
            focus: self.focus,
            force: self.force,
            max_turns: self.max_turns,
            timeout,
            max_tokens: self.max_tokens,
            retries: self.retries,
            keys,
            control: RunControl::default(),
        })
    }
}

fn default_catalog_limit() -> usize {
    50
}
fn default_true() -> bool {
    true
}
fn default_turns() -> usize {
    40
}
fn default_timeout() -> String {
    "15m".into()
}
fn default_tokens() -> u64 {
    4_000_000
}
fn default_retries() -> u32 {
    1
}

#[derive(Clone)]
pub struct InitOptions {
    pub start_url: String,
    pub daily: Cents,
    pub currency: String,
    pub provider: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub catalog_limit: usize,
    pub web_search: bool,
    pub focus: bool,
    pub force: bool,
    pub max_turns: usize,
    pub timeout: Duration,
    pub max_tokens: u64,
    pub retries: u32,
    pub keys: std::collections::BTreeMap<String, String>,
    pub control: RunControl,
}

fn key_hint(provider: &str) -> String {
    match crate::serve::secrets::env_for(provider) {
        Some(var) => format!(" Defina {var} ou salve a chave em Conexões."),
        None => String::new(),
    }
}

async fn run_init_job(job: Arc<Job>, project_dir: PathBuf, options: InitOptions) {
    let api_key = crate::serve::secrets::env_for(&options.provider)
        .and_then(|var| options.keys.get(var).cloned());
    let selection = ProviderSelection {
        provider: options.provider.clone(),
        model: options.model.clone(),
        plan_model: None,
        base_url: options.base_url.clone(),
        api_key,
        script: None,
    };
    let drivers = match build_drivers(&selection) {
        Ok(drivers) => drivers,
        Err(e) => {
            job.finish(JobState::Failed, Some(e.to_string()));
            return;
        }
    };
    if let Err(e) = preflight(&selection).await {
        job.finish(
            JobState::Failed,
            Some(format!("{e}{}", key_hint(&options.provider))),
        );
        return;
    }
    let allow_private = std::env::var_os("MADS_ALLOW_PRIVATE_HOSTS").is_some_and(|v| !v.is_empty());
    let site = match SiteClient::new(&options.start_url, allow_private) {
        Ok(site) => Arc::new(site),
        Err(e) => {
            job.finish(JobState::Failed, Some(e));
            return;
        }
    };
    let mut cfg = InitConfig::new(
        project_dir,
        options.start_url,
        options.daily,
        options.currency.clone(),
    );
    cfg.catalog_limit = options.catalog_limit;
    cfg.force = options.force;
    cfg.max_turns = options.max_turns;
    cfg.mission_timeout = options.timeout;
    cfg.max_tokens = options.max_tokens;
    cfg.mission_retries = options.retries;
    cfg.provider = options.provider;
    cfg.model = options.model;
    cfg.web_search = options.web_search;
    cfg.focus = options.focus;
    cfg.control = job.control.clone();

    let (events, rx) = EventSink::channel();
    let pump = spawn_pump(job.clone(), rx);
    let result = run_init(cfg, drivers.plan, site, events).await;
    let _ = pump.await;
    match result {
        Ok(result) if result.exit_code == 0 => job.finish(JobState::Done, None),
        Ok(_) => job.finish(JobState::Failed, None),
        Err(e) => job.finish(JobState::Failed, Some(e.to_string())),
    }
}

#[allow(clippy::too_many_arguments)]
fn new_job(
    project_dir: &Path,
    project_id: &str,
    project_name: &str,
    kind: &str,
    run_id: Option<String>,
    log: PathBuf,
    lock: Option<File>,
    control: RunControl,
) -> Result<Arc<Job>, String> {
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let (tx, _) = broadcast::channel(256);
    Ok(Arc::new(Job {
        id: Uuid::new_v4().simple().to_string(),
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        project_dir: project_dir.to_path_buf(),
        kind: kind.to_string(),
        state: Mutex::new(JobState::Running),
        run_id: Mutex::new(run_id),
        error: Mutex::new(None),
        started: clock(),
        finished: Mutex::new(None),
        control,
        tx,
        log,
        _lock: Mutex::new(lock),
    }))
}

async fn run_generate_job(
    job: Arc<Job>,
    project_dir: PathBuf,
    run: Option<RunDir>,
    input: Input,
    options: GenerateOptions,
) {
    let api_key = crate::serve::secrets::env_for(&options.provider)
        .and_then(|var| options.keys.get(var).cloned());
    let selection = ProviderSelection {
        provider: options.provider.clone(),
        model: options.model.clone(),
        plan_model: options.plan_model.clone(),
        base_url: options.base_url.clone(),
        api_key,
        script: options.script.clone(),
    };
    let mut drivers = match build_drivers(&selection) {
        Ok(drivers) => drivers,
        Err(e) => {
            job.finish(JobState::Failed, Some(e.to_string()));
            return;
        }
    };
    if let Err(e) = preflight(&selection).await {
        job.finish(
            JobState::Failed,
            Some(format!("{e}{}", key_hint(&options.provider))),
        );
        return;
    }
    let keys = options.keys.clone();
    drivers.image = match build_image_model_with_keys(
        &options.image_provider,
        options.image_model.as_deref(),
        &|name| keys.get(name).cloned(),
    ) {
        Ok(image) => image,
        Err(e) => {
            job.finish(JobState::Failed, Some(e.to_string()));
            return;
        }
    };
    let web = match WebClient::new() {
        Ok(web) => Arc::new(web),
        Err(e) => {
            job.finish(JobState::Failed, Some(e.to_string()));
            return;
        }
    };
    let run = match run {
        Some(run) => run,
        None => {
            job.finish(JobState::Failed, Some("execução não encontrada".into()));
            return;
        }
    };
    let mut cfg = RunConfig::new(project_dir.join("out"));
    cfg.run_dir = Some(run.root().to_path_buf());
    cfg.parallel = options.parallel;
    cfg.max_turns = options.max_turns;
    cfg.mission_timeout = options.timeout;
    cfg.max_tokens = options.max_tokens;
    cfg.mission_retries = options.retries;
    cfg.max_ad_groups = options.max_ad_groups;
    cfg.skip_url_check = options.skip_url_check;
    cfg.layout = options.layout;
    cfg.max_images = options.max_images;
    cfg.provider = options.provider.clone();
    cfg.model = options.model.clone();
    cfg.stop_after_plan = options.plan_review;
    cfg.control = job.control.clone();

    let (events, rx) = EventSink::channel();
    let pump = spawn_pump(job.clone(), rx);
    let result = generate(input, drivers, web, cfg, events).await;
    let _ = pump.await;
    match result {
        Ok(_) if job.control.is_cancelled() => {
            job.finish(JobState::Cancelled, None);
        }
        Ok(result) => {
            let state = if result.exit_code == 0 {
                JobState::Done
            } else {
                JobState::Failed
            };
            job.finish(state, None);
        }
        Err(e) => job.finish(JobState::Failed, Some(e.to_string())),
    }
}

async fn run_export_job(
    job: Arc<Job>,
    project_dir: PathBuf,
    run_id: String,
    options: ExportOptions,
) {
    let run_dir = project_dir.join("out").join(run_id);
    let web = match WebClient::new() {
        Ok(web) => Arc::new(web),
        Err(e) => {
            job.finish(JobState::Failed, Some(e.to_string()));
            return;
        }
    };
    let (events, rx) = EventSink::channel();
    let pump = spawn_pump(job.clone(), rx);
    let result = export_run(
        &run_dir,
        web.as_ref(),
        options.skip_url_check,
        options.max_ad_groups,
        options.layout,
        &events,
    )
    .await;
    drop(events);
    let _ = pump.await;
    match result {
        Ok(result) => {
            let state = if result.exit_code == 0 {
                JobState::Done
            } else {
                JobState::Failed
            };
            job.finish(state, None);
        }
        Err(e) => job.finish(JobState::Failed, Some(e.to_string())),
    }
}

fn spawn_pump(
    job: Arc<Job>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<EventEnvelope>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&job.log)
            .ok();
        while let Some(envelope) = rx.recv().await {
            if let Event::RunStarted { run_id, .. } = &envelope.event
                && let Ok(mut slot) = job.run_id.lock()
            {
                *slot = Some(run_id.clone());
            }
            if let Some(file) = file.as_mut()
                && let Ok(line) = serde_json::to_string(&envelope)
            {
                let _ = writeln!(file, "{line}");
            }
            let _ = job.tx.send(envelope);
        }
    })
}

fn prepare_run(project_dir: &Path, input: &mut Input) -> Result<RunDir, String> {
    let run = RunDir::create(&project_dir.join("out")).map_err(|e| e.to_string())?;
    let input_dir = run.root().join("input");
    fs::create_dir_all(&input_dir).map_err(|e| e.to_string())?;
    let business = project_dir.join("business.toml");
    fs::copy(&business, input_dir.join("business.toml")).map_err(|e| e.to_string())?;
    if let Ok(text) = fs::read_to_string(&business)
        && let Ok(file) = parse_input_toml(&text)
        && let Some(rel) = file.catalog_file
    {
        let src = project_dir.join(&rel);
        let name = Path::new(&rel)
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("catalog.csv"));
        fs::copy(&src, input_dir.join(name)).map_err(|e| e.to_string())?;
    }
    if let Some(logo) = input.logo.clone() {
        let src = PathBuf::from(&logo);
        let name = src
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("logo.png"));
        let dest = input_dir.join(name);
        if src != dest {
            fs::copy(&src, &dest).map_err(|e| e.to_string())?;
            input.logo = Some(dest.display().to_string());
        }
    }
    Ok(run)
}

fn lock_project(project_dir: &Path) -> Result<File, String> {
    let dir = project_dir.join(".mads");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("lock"))
        .map_err(|e| e.to_string())?;
    file.try_lock_exclusive()
        .map_err(|_| "outra execução está usando este projeto".to_string())?;
    Ok(file)
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn clock() -> String {
    OffsetDateTime::now_utc()
        .format(
            &time::format_description::parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]")
                .unwrap_or_default(),
        )
        .unwrap_or_default()
}

pub fn read_log(path: &Path) -> Vec<EventEnvelope> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_a_plan_review_and_bulk_layout() {
        let request = GenerateRequest {
            provider: "anthropic".into(),
            model: Some("m".into()),
            plan_model: None,
            base_url: None,
            image_provider: None,
            image_model: None,
            layout: None,
            parallel: None,
            max_turns: None,
            timeout: None,
            max_tokens: None,
            retries: None,
            max_ad_groups: None,
            max_images: None,
            skip_url_check: None,
            plan_review: true,
            script: None,
        };
        let options = GenerateOptions::from_request(&request).unwrap();
        assert_eq!(options.layout, Some(ExportLayout::Bulk));
        assert!(options.plan_review);
        assert_eq!(options.parallel, 4);
    }

    #[test]
    fn options_reject_an_unknown_layout_and_an_empty_provider() {
        let mut request = GenerateRequest {
            provider: "anthropic".into(),
            model: None,
            plan_model: None,
            base_url: None,
            image_provider: None,
            image_model: None,
            layout: Some("nope".into()),
            parallel: None,
            max_turns: None,
            timeout: None,
            max_tokens: None,
            retries: None,
            max_ad_groups: None,
            max_images: None,
            skip_url_check: None,
            plan_review: true,
            script: None,
        };
        assert!(GenerateOptions::from_request(&request).is_err());
        request.layout = None;
        request.provider = " ".into();
        assert!(GenerateOptions::from_request(&request).is_err());
    }

    #[test]
    fn export_options_parse_the_layout() {
        let request = ExportRequest {
            skip_url_check: Some(true),
            layout: Some("drive-folders".into()),
            max_ad_groups: Some(10),
        };
        let options = ExportOptions::from_request(&request).unwrap();
        assert!(options.skip_url_check);
        assert_eq!(options.layout, Some(ExportLayout::DriveFolders));
        assert_eq!(options.max_ad_groups, 10);
    }
}
