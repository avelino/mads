use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Seek, Write},
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

/// How many jobs run at once. The spec caps the server at two; the rest wait in a FIFO queue.
const MAX_RUNNING_JOBS: usize = 2;
/// Finished jobs kept in the registry before the oldest ones are dropped.
const JOB_HISTORY: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Cancelling,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn label(self) -> &'static str {
        match self {
            JobState::Queued => "Na fila",
            JobState::Running => "Em execução",
            JobState::Cancelling => "Parando",
            JobState::Done => "Concluído",
            JobState::Failed => "Falhou",
            JobState::Cancelled => "Cancelado",
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            JobState::Queued => "muted",
            JobState::Running => "running",
            JobState::Cancelling => "warn",
            JobState::Done => "ok",
            JobState::Failed => "error",
            JobState::Cancelled => "muted",
        }
    }
}

/// Why a job could not start. A conflict is a state clash, not a bad request, so
/// handlers answer it with 409.
#[derive(Debug, Clone)]
pub enum JobError {
    Conflict,
    Message(String),
}

impl From<String> for JobError {
    fn from(message: String) -> Self {
        JobError::Message(message)
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::Conflict => write!(f, "este projeto já tem uma execução em andamento"),
            JobError::Message(message) => write!(f, "{message}"),
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
    /// Runs once when the job leaves the running set. Attached by the gate when
    /// the job actually gets a slot, so a queued job cancelled before starting
    /// never releases a slot it never held.
    on_release: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
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

    fn set_state(&self, state: JobState) {
        if let Ok(mut slot) = self.state.lock() {
            *slot = state;
        }
    }

    fn finish(&self, state: JobState, error: Option<String>) {
        crate::serve::audit::append(
            &self.project_dir,
            "job_finished",
            &format!("{} {} {:?}", self.id, self.kind, state),
        );
        self.set_state(state);
        if let Ok(mut slot) = self.error.lock() {
            *slot = error;
        }
        if let Ok(mut slot) = self.finished.lock() {
            *slot = Some(clock());
        }
        if let Ok(mut lock) = self._lock.lock() {
            *lock = None;
        }
        if let Ok(mut hook) = self.on_release.lock()
            && let Some(release) = hook.take()
        {
            release();
        }
    }
}

/// How a queued job is started once a slot frees up.
type Spawn = Box<dyn FnOnce(Arc<Job>) + Send>;

/// Caps the server at `MAX_RUNNING_JOBS` live jobs and keeps the rest in a FIFO
/// queue, as the design spec requires.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
}

#[derive(Default)]
struct GateState {
    running: usize,
    pending: VecDeque<(Arc<Job>, Spawn)>,
}

impl Gate {
    fn admit(self: &Arc<Self>, job: Arc<Job>, spawn: Spawn) {
        let mut gate = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if gate.running >= MAX_RUNNING_JOBS {
            job.set_state(JobState::Queued);
            gate.pending.push_back((job, spawn));
            return;
        }
        gate.running += 1;
        drop(gate);
        self.launch(job, spawn);
    }

    /// Frees one slot and starts the first queued job that was not cancelled
    /// while waiting. Called from `Job::finish`, never while holding the gate.
    fn release(self: &Arc<Self>) {
        let next = {
            let mut gate = self.state.lock().unwrap_or_else(|e| e.into_inner());
            gate.running = gate.running.saturating_sub(1);
            loop {
                match gate.pending.pop_front() {
                    Some((job, spawn)) => {
                        let queued = job.state.lock().is_ok_and(|s| *s == JobState::Queued);
                        if queued {
                            gate.running += 1;
                            break Some((job, spawn));
                        }
                    }
                    None => break None,
                }
            }
        };
        if let Some((job, spawn)) = next {
            self.launch(job, spawn);
        }
    }

    fn launch(self: &Arc<Self>, job: Arc<Job>, spawn: Spawn) {
        job.set_state(JobState::Running);
        let gate = Arc::clone(self);
        if let Ok(mut hook) = job.on_release.lock() {
            *hook = Some(Box::new(move || gate.release()));
        }
        spawn(job);
    }
}

#[derive(Clone, Default)]
pub struct Jobs {
    inner: Arc<Mutex<HashMap<String, Arc<Job>>>>,
    gate: Arc<Gate>,
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
        jobs.truncate(JOB_HISTORY);
        jobs
    }

    pub fn cancel(&self, id: &str) -> bool {
        let Some(job) = self.inner.lock().ok().and_then(|map| map.get(id).cloned()) else {
            return false;
        };
        job.control.cancel();
        let queued = job
            .state
            .lock()
            .is_ok_and(|s| *s == JobState::Queued || *s == JobState::Cancelling);
        if let Ok(mut state) = job.state.lock()
            && *state == JobState::Running
        {
            *state = JobState::Cancelling;
        }
        // A queued job has no task to observe the control: finish it right away.
        // The pending entry stays in the queue and is skipped when it drains.
        if queued {
            job.finish(JobState::Cancelled, None);
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
                        JobState::Queued | JobState::Running | JobState::Cancelling
                    )
            })
        })
    }

    pub fn start_generate(
        &self,
        project: &Project,
        project_dir: &Path,
        options: GenerateOptions,
    ) -> Result<JobView, JobError> {
        if self.active(&project.id) {
            return Err(JobError::Conflict);
        }
        if !project_dir.join("business.toml").is_file() {
            return Err(JobError::Message(
                "o projeto não tem business.toml. Estude o site ou crie o rascunho antes de gerar."
                    .into(),
            ));
        }
        // The lock comes first: a losing submitter must not leave an orphan run
        // directory behind.
        let lock = lock_project(project_dir)?;
        let mut input = load_input(&project_dir.join("business.toml"))
            .map_err(|e| JobError::Message(format!("o rascunho não carrega: {e}")))?;
        let run = prepare_run(project_dir, &mut input)?;
        let run_id = run.id();
        let log = run.root().join("events.ndjson");
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
        let view = job.view();
        let project_dir = project_dir.to_path_buf();
        self.admit(
            job,
            Box::new(move |job| {
                start_job_thread(
                    job,
                    Box::new(move |job| {
                        Box::pin(run_generate_job(
                            job,
                            project_dir,
                            Some(run),
                            input,
                            options,
                        ))
                    }),
                );
            }),
        );
        Ok(view)
    }

    pub fn start_resume(
        &self,
        project: &Project,
        project_dir: &Path,
        run_id: &str,
        mut options: GenerateOptions,
    ) -> Result<JobView, JobError> {
        options.plan_review = false;
        options.layout = None;
        if self.active(&project.id) {
            return Err(JobError::Conflict);
        }
        let run_dir = project_dir.join("out").join(run_id);
        if !run_dir.join("workspace.json").is_file() {
            return Err(JobError::Message("execução não encontrada".into()));
        }
        let input = Workspace::load(&run_dir.join("workspace.json"))
            .map_err(|e| JobError::Message(format!("não foi possível abrir a execução: {e}")))?
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
        let view = job.view();
        let project_dir = project_dir.to_path_buf();
        let run_id = run_id.to_string();
        self.admit(
            job,
            Box::new(move |job| {
                start_job_thread(
                    job,
                    Box::new(move |job| {
                        Box::pin(run_resume_job(job, project_dir, run_id, input, options))
                    }),
                );
            }),
        );
        Ok(view)
    }

    pub fn start_init(
        &self,
        project: &Project,
        project_dir: &Path,
        options: InitOptions,
    ) -> Result<JobView, JobError> {
        if self.active(&project.id) {
            return Err(JobError::Conflict);
        }
        let job_id = Uuid::new_v4().simple().to_string();
        let job_dir = project_dir.join(".mads").join("jobs").join(&job_id);
        fs::create_dir_all(&job_dir).map_err(|e| JobError::Message(e.to_string()))?;
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
        let view = job.view();
        let project_dir = project_dir.to_path_buf();
        self.admit(
            job,
            Box::new(move |job| {
                start_job_thread(
                    job,
                    Box::new(move |job| Box::pin(run_init_job(job, project_dir, options))),
                );
            }),
        );
        Ok(view)
    }

    pub fn start_revision(
        &self,
        project: &Project,
        project_dir: &Path,
        base_run_id: &str,
        edits: &[Edit],
    ) -> Result<(JobView, String), RevisionError> {
        if self.active(&project.id) {
            return Err(RevisionError::Conflict);
        }
        let base = project_dir.join("out").join(base_run_id);
        let workspace = Workspace::load(&base.join("workspace.json")).map_err(|e| {
            RevisionError::Message(format!("não foi possível abrir a execução: {e}"))
        })?;
        let account = revisions::apply(&workspace.account, &workspace.input, edits, 50)?;
        let mut workspace = workspace;
        workspace.account = account;
        // The revision directory is numbered and created under the project lock,
        // so two concurrent requests cannot both win `-r1`.
        let lock = lock_project(project_dir).map_err(RevisionError::Message)?;
        let (run_id, _) = revisions::create(project_dir, base_run_id, &workspace, edits)
            .map_err(RevisionError::Message)?;
        crate::serve::audit::append(
            project_dir,
            "revision_created",
            &format!("{base_run_id} -> {run_id} ({} edits)", edits.len()),
        );
        match self.begin_export(
            project,
            project_dir,
            &run_id,
            ExportOptions {
                skip_url_check: false,
                layout: None,
                max_ad_groups: 50,
            },
            Some(lock),
        ) {
            Ok(job) => Ok((job, run_id)),
            Err(e) => {
                // The revision was created but its export cannot start; an empty
                // sibling directory would only confuse the runs list.
                let _ = fs::remove_dir_all(project_dir.join("out").join(&run_id));
                Err(match e {
                    JobError::Conflict => RevisionError::Conflict,
                    JobError::Message(m) => RevisionError::Message(m),
                })
            }
        }
    }

    pub fn start_export(
        &self,
        project: &Project,
        project_dir: &Path,
        run_id: &str,
        options: ExportOptions,
    ) -> Result<JobView, JobError> {
        if self.active(&project.id) {
            return Err(JobError::Conflict);
        }
        self.begin_export(project, project_dir, run_id, options, None)
    }

    fn begin_export(
        &self,
        project: &Project,
        project_dir: &Path,
        run_id: &str,
        options: ExportOptions,
        lock: Option<File>,
    ) -> Result<JobView, JobError> {
        let run_dir = project_dir.join("out").join(run_id);
        if !run_dir.join("workspace.json").is_file() {
            return Err(JobError::Message("execução não encontrada".into()));
        }
        let log = run_dir.join("events.ndjson");
        let lock = match lock {
            Some(lock) => lock,
            None => lock_project(project_dir)?,
        };
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
        let view = job.view();
        let project_dir = project_dir.to_path_buf();
        let run_id = run_id.to_string();
        self.admit(
            job,
            Box::new(move |job| {
                start_job_thread(
                    job,
                    Box::new(move |job| {
                        Box::pin(run_export_job(job, project_dir, run_id, options))
                    }),
                );
            }),
        );
        Ok(view)
    }

    fn admit(&self, job: Arc<Job>, spawn: Spawn) {
        self.gate.admit(job, spawn);
    }

    fn insert(&self, job: Arc<Job>) {
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(_) => return,
        };
        map.insert(job.id.clone(), job);
        if map.len() <= JOB_HISTORY * 2 {
            return;
        }
        // Drop the oldest finished jobs so a long-lived server does not grow the
        // registry forever.
        let mut finished: Vec<(String, String)> = map
            .values()
            .filter_map(|job| {
                let done = job.state.lock().is_ok_and(|s| {
                    matches!(*s, JobState::Done | JobState::Failed | JobState::Cancelled)
                });
                let finished = job.finished.lock().ok().and_then(|f| f.clone());
                done.then_some(finished)
                    .flatten()
                    .map(|f| (job.id.clone(), f))
            })
            .collect();
        finished.sort_by(|a, b| a.1.cmp(&b.1));
        let excess = map.len().saturating_sub(JOB_HISTORY);
        for (id, _) in finished.into_iter().take(excess) {
            map.remove(&id);
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

    let (events, rx) = EventSink::channel_from(log_base(&job.log));
    let pump = spawn_pump(job.clone(), rx);
    let result = run_init(cfg, drivers.plan, site, events).await;
    let _ = pump.await;
    if job.control.is_cancelled() {
        job.finish(JobState::Cancelled, None);
        return;
    }
    match result {
        Ok(result) if result.exit_code == 0 => job.finish(JobState::Done, None),
        Ok(_) => job.finish(
            JobState::Failed,
            Some("a inicialização terminou com erros; veja o registro do job".into()),
        ),
        Err(e) => job.finish(JobState::Failed, Some(e.to_string())),
    }
}

/// Starts a job task on its own thread and current-thread runtime, so blocking
/// core work never runs on the axum workers. The task must box its future at the
/// call site: rustc cannot prove `Send` for an async-fn future through a generic
/// bound (`implementation of Send is not general enough`).
fn start_job_thread(job: Arc<Job>, task: JobTask) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            job.finish(
                JobState::Failed,
                Some("cannot start the job runtime".into()),
            );
            return;
        };
        runtime.block_on(task(job));
    });
}

type JobTask =
    Box<dyn FnOnce(Arc<Job>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + Send>;

/// The seq the new job's events must continue from: the last one already in the
/// log. Without it a resume or export restarts at 1 and the SSE dedup on the
/// reader side silently drops every live event.
fn log_base(path: &Path) -> u64 {
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    match lines
        .last()
        .and_then(|last| serde_json::from_str::<EventEnvelope>(last).ok())
    {
        Some(envelope) => envelope.seq,
        None => lines.len() as u64,
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
        on_release: Mutex::new(None),
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

    let (events, rx) = EventSink::channel_from(log_base(&job.log));
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
            let error = (result.exit_code != 0)
                .then_some("a execução terminou com erros; veja o relatório".to_string());
            job.finish(state, error);
        }
        Err(e) => job.finish(JobState::Failed, Some(e.to_string())),
    }
}

/// A resume opens the existing run directory before the generate pipeline runs.
async fn run_resume_job(
    job: Arc<Job>,
    project_dir: PathBuf,
    run_id: String,
    input: Input,
    options: GenerateOptions,
) {
    let run = RunDir::open(&project_dir.join("out").join(run_id)).ok();
    run_generate_job(job, project_dir, run, input, options).await;
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
    let (events, rx) = EventSink::channel_from(log_base(&job.log));
    let pump = spawn_pump(job.clone(), rx);
    let result = export_run(
        &run_dir,
        web.as_ref(),
        options.skip_url_check,
        options.max_ad_groups,
        options.layout,
        &events,
        job.control.clone(),
    )
    .await;
    drop(events);
    let _ = pump.await;
    if job.control.is_cancelled() {
        job.finish(JobState::Cancelled, None);
        return;
    }
    match result {
        Ok(result) => {
            let state = if result.exit_code == 0 {
                JobState::Done
            } else {
                JobState::Failed
            };
            let error = (result.exit_code != 0)
                .then_some("a exportação terminou com erros; veja o relatório".to_string());
            job.finish(state, error);
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
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("lock"))
        .map_err(|e| e.to_string())?;
    file.try_lock_exclusive()
        .map_err(|_| "outra execução está usando este projeto".to_string())?;
    // Best-effort diagnostics: a contended lock tells whoever lost who holds it.
    let _ = file.set_len(0);
    let _ = file.seek(std::io::SeekFrom::Start(0));
    let _ = writeln!(
        &file,
        "{{\"pid\": {}, \"ts\": \"{}\"}}",
        std::process::id(),
        clock()
    );
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
