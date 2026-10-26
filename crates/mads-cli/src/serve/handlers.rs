use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
};
use mads_core::{
    events::Event,
    input::{normalize_url, slugify},
    optimize,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::serve::{
    AppState,
    auth::{Auth, Identity},
    draft::{self, DraftPayload},
    jobs::{self, ExportOptions, ExportRequest, GenerateOptions, GenerateRequest, InitMeta},
    revisions::{self, Edit, RevisionError},
    store,
    views::{self, Chrome, Html},
};

const CSRF_HEADER: &str = "x-mads-csrf";

/// The page title in the session language. The key is the Portuguese string.
fn tt(lang: &str, pt: &str) -> String {
    crate::serve::i18n::t(lang, pt)
}

pub async fn index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let projects = store::projects(state.root());
    Html(views::ProjectsPage {
        chrome: Chrome::new(identity.csrf.clone(), "projects", &identity.lang),
        title: tt(&identity.lang, "Projetos"),
        projects,
    })
    .into_response()
}

pub async fn project_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some(project) = store::project(state.root(), &id) else {
        return not_found_page(&identity, "Projeto não encontrado");
    };
    Html(views::ProjectPage {
        chrome: Chrome::new(identity.csrf.clone(), "project", &identity.lang)
            .with_project(&project.name),
        title: project.name.clone(),
        project,
    })
    .into_response()
}

pub async fn run_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "run", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::RunPage {
        chrome,
        title: run.summary.id.clone(),
        run,
    })
    .into_response()
}

pub async fn account_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "account", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::AccountPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Conta"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn edit_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "edit", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::EditPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Corrigir"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn validation_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "validation", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::ValidationPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Validação"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn report_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "report", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::ReportPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Relatório"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn files_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "files", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::FilesPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Arquivos"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn events_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "events", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::EventsPage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Eventos"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn init_project_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some(project) = store::project(state.root(), &id) else {
        return not_found_page(&identity, "Projeto não encontrado");
    };
    let dir = state.root().join(&project.id);
    let mut meta = read_init_meta(&dir).unwrap_or_default();
    if meta.start_url.trim().is_empty() {
        meta.start_url = project.url.clone();
    }
    if meta.provider.trim().is_empty() {
        meta.provider = std::env::var("MADS_PROVIDER").unwrap_or_default();
    }
    if meta.model.as_deref().unwrap_or_default().trim().is_empty() {
        meta.model = std::env::var("MADS_MODEL").ok().filter(|m| !m.is_empty());
    }
    let model_text = meta.model.clone().unwrap_or_default();
    let base_url_text = meta.base_url.clone().unwrap_or_default();
    Html(views::InitProjectPage {
        chrome: Chrome::new(identity.csrf.clone(), "init", &identity.lang)
            .with_project(&project.name),
        title: format!("{} · {}", tt(&identity.lang, "Estudar site"), project.name),
        project,
        meta,
        model_text,
        base_url_text,
    })
    .into_response()
}

pub async fn start_init_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(meta): Json<InitMeta>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    let dir = state.root().join(&project.id);
    let options = match meta.options(state.secrets.resolved()) {
        Ok(options) => options,
        Err(message) => return bad_request(message),
    };
    match state.jobs.start_init(&project, &dir, options) {
        Ok(job) => {
            if let Err(e) = write_init_meta(&dir, &meta) {
                crate::serve::audit::append(&dir, "init_meta_write_failed", &e);
            }
            (
                StatusCode::ACCEPTED,
                Json(json!({"job_id": job.id, "project_id": project.id})),
            )
                .into_response()
        }
        Err(error) => job_error(error),
    }
}

fn read_init_meta(dir: &std::path::Path) -> Option<InitMeta> {
    let text = std::fs::read_to_string(dir.join("project.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(value.get("init")?.clone()).ok()
}

fn write_init_meta(dir: &std::path::Path, meta: &InitMeta) -> Result<(), String> {
    // Read-modify-write so `name` and any future key survive; atomic rename so a
    // crash cannot leave a torn project.json (which would un-archive the project).
    let path = dir.join("project.json");
    let mut object = match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value @ serde_json::Value::Object(_)) => value,
                _ => return Err("project.json precisa ser um objeto JSON".into()),
            }
        }
        Ok(_) => json!({}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.to_string()),
    };
    let value = serde_json::to_value(meta).map_err(|e| e.to_string())?;
    object["init"] = value;
    let text = serde_json::to_vec_pretty(&object).map_err(|e| e.to_string())?;
    crate::serve::project_admin::write_atomic(&path, &text)
}

pub async fn draft_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some(project) = store::project(state.root(), &id) else {
        return not_found_page(&identity, "Projeto não encontrado");
    };
    let draft = draft::load(&state.root().join(&project.id));
    Html(views::DraftPage {
        chrome: Chrome::new(identity.csrf.clone(), "draft", &identity.lang)
            .with_project(&project.name),
        title: format!("{} · {}", tt(&identity.lang, "Rascunho"), project.name),
        project,
        draft,
    })
    .into_response()
}

pub async fn save_draft(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<DraftPayload>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    match draft::save(&state.root().join(&project.id), &payload) {
        Ok(()) => {
            crate::serve::audit::append(
                &state.root().join(&project.id),
                "draft_saved",
                "business.toml",
            );
            Json(json!({"ok": true})).into_response()
        }
        Err(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": {"code": "INVALID", "message": message}})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct TextAssetRequest {
    text: String,
}

pub async fn save_research(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TextAssetRequest>,
) -> Response {
    save_text_asset(state, id, headers, request.text, true).await
}

pub async fn save_design(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<TextAssetRequest>,
) -> Response {
    save_text_asset(state, id, headers, request.text, false).await
}

async fn save_text_asset(
    state: AppState,
    id: String,
    headers: HeaderMap,
    text: String,
    research: bool,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    let dir = state.root().join(&project.id);
    let result = if research {
        draft::save_research(&dir, &text)
    } else {
        draft::save_design(&dir, &text)
    };
    match result {
        Ok(()) => {
            crate::serve::audit::append(
                &dir,
                if research {
                    "research_saved"
                } else {
                    "design_saved"
                },
                "markdown",
            );
            Json(json!({"ok": true})).into_response()
        }
        Err(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": {"code": "INVALID", "message": message}})),
        )
            .into_response(),
    }
}

pub async fn project_logo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    let path = state.root().join(&project.id).join("brand/logo.png");
    match std::fs::read(&path) {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/png")], bytes).into_response(),
        Err(_) => api_not_found(),
    }
}

pub async fn upload_logo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    let mut bytes = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => return bad_request(format!("upload inválido: {e}")),
        };
        if field.name() != Some("logo") {
            continue;
        }
        let data = match field.bytes().await {
            Ok(data) => data,
            Err(e) => return bad_request(format!("não foi possível ler o logo: {e}")),
        };
        if data.len() > 2 * 1024 * 1024 {
            return bad_request("o logo passa de 2 MB".into());
        }
        bytes = Some(data);
        break;
    }
    let Some(bytes) = bytes else {
        return bad_request("envie um arquivo de logo".into());
    };
    let dir = state.root().join(&project.id);
    match draft::save_logo(&dir, &bytes) {
        Ok(()) => {
            crate::serve::audit::append(&dir, "logo_saved", "brand/logo.png");
            Json(json!({"ok": true})).into_response()
        }
        Err(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": {"code": "INVALID", "message": message}})),
        )
            .into_response(),
    }
}

pub async fn new_project_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let (providers, _) = views::providers_with(&state.secrets);
    Html(views::NewProjectPage {
        chrome: Chrome::new(identity.csrf.clone(), "new-project", &identity.lang),
        title: tt(&identity.lang, "Novo projeto"),
        providers,
        default_provider: std::env::var("MADS_PROVIDER").unwrap_or_default(),
        default_model: std::env::var("MADS_MODEL").unwrap_or_default(),
        default_base_url: std::env::var("MADS_BASE_URL").unwrap_or_default(),
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
    start_url: String,
    daily: f64,
    currency: String,
    provider: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    catalog_limit: Option<usize>,
    #[serde(default)]
    web_search: Option<bool>,
    #[serde(default)]
    focus: Option<bool>,
    #[serde(default)]
    max_turns: Option<usize>,
    #[serde(default)]
    timeout: Option<String>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    retries: Option<u32>,
    #[serde(default)]
    force: Option<bool>,
}

pub async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let name = request.name.trim().to_string();
    let count = name.chars().count();
    if !(1..=80).contains(&count) {
        return bad_request("o nome precisa de 1 a 80 caracteres".into());
    }
    if normalize_url(&request.start_url).is_none() {
        return bad_request("a URL precisa ser http(s)".into());
    }
    let currency = request.currency.trim().to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return bad_request("a moeda precisa de 3 letras, como BRL".into());
    }
    if request.provider.trim().is_empty() {
        return bad_request("escolha um provedor".into());
    }
    let base = slugify(&name);
    if base.is_empty() {
        return bad_request("o nome precisa de letras ou números".into());
    }
    let Some(id) = crate::serve::project_admin::next_project_id(state.root(), &base) else {
        return bad_request("já existem 99 projetos com este nome; escolha outro nome".into());
    };
    // Exclusive create: two simultaneous submissions cannot share one
    // directory, so the cleanup below can never delete the other's project.
    let dir = match crate::serve::project_admin::create_project_dir(state.root(), &id) {
        Ok(dir) => dir,
        Err(message) => return bad_request(message),
    };
    let meta = InitMeta {
        start_url: request.start_url.trim().to_string(),
        daily: request.daily,
        currency: currency.clone(),
        provider: request.provider.trim().to_string(),
        model: request
            .model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty()),
        base_url: request
            .base_url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty()),
        catalog_limit: request.catalog_limit.unwrap_or(50),
        web_search: request.web_search.unwrap_or(true),
        focus: request.focus.unwrap_or(false),
        force: request.force.unwrap_or(false),
        max_turns: request.max_turns.unwrap_or(40),
        timeout: request.timeout.clone().unwrap_or_else(|| "15m".into()),
        max_tokens: request.max_tokens.unwrap_or(4_000_000),
        retries: request.retries.unwrap_or(1),
    };
    if let Err(e) =
        crate::serve::project_admin::write_project_json(&dir, json!({"name": name.clone()}))
    {
        let _ = std::fs::remove_dir_all(&dir);
        return bad_request(e);
    }
    crate::serve::audit::append(&dir, "project_created", &name);
    let project = store::Project {
        id: id.clone(),
        name: name.clone(),
        ..store::Project::default()
    };
    let options = match meta.options(state.secrets.resolved()) {
        Ok(options) => options,
        Err(message) => {
            let _ = std::fs::remove_dir_all(&dir);
            return bad_request(message);
        }
    };
    match state.jobs.start_init(&project, &dir, options) {
        Ok(job) => {
            // Persisted only once the job is accepted: a rejected init must
            // leave no half-written metadata behind.
            if let Err(e) = write_init_meta(&dir, &meta) {
                crate::serve::audit::append(&dir, "init_meta_write_failed", &e);
            }
            (
                StatusCode::ACCEPTED,
                Json(json!({"job_id": job.id, "project_id": id})),
            )
                .into_response()
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            job_error(error)
        }
    }
}

pub async fn new_run_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some(project) = store::project(state.root(), &id) else {
        return not_found_page(&identity, "Projeto não encontrado");
    };
    if !state
        .root()
        .join(&project.id)
        .join("business.toml")
        .is_file()
    {
        return axum::response::Redirect::to(&format!("/projects/{}/init", project.id))
            .into_response();
    }
    let (providers, _) = views::providers_with(&state.secrets);
    Html(views::NewRunPage {
        chrome: Chrome::new(identity.csrf.clone(), "project", &identity.lang)
            .with_project(&project.name),
        title: format!("{} · {}", tt(&identity.lang, "Nova execução"), project.name),
        project,
        providers,
        default_provider: std::env::var("MADS_PROVIDER").unwrap_or_default(),
        default_model: std::env::var("MADS_MODEL").unwrap_or_default(),
        default_base_url: std::env::var("MADS_BASE_URL").unwrap_or_default(),
    })
    .into_response()
}

pub async fn jobs_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    Html(views::JobsPage {
        chrome: Chrome::new(identity.csrf.clone(), "jobs", &identity.lang),
        title: tt(&identity.lang, "Jobs"),
        jobs: state.jobs.views(),
    })
    .into_response()
}

pub async fn job_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some(job) = state.jobs.get(&id) else {
        return not_found_page(&identity, "Execução de trabalho não encontrada");
    };
    Html(views::JobPage {
        chrome: Chrome::new(identity.csrf.clone(), "job", &identity.lang)
            .with_project(&job.project_name),
        title: format!("{} {}", tt(&identity.lang, "Job"), job.id),
        job,
    })
    .into_response()
}

pub async fn create_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<GenerateRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    let mut options = match GenerateOptions::from_request(&request) {
        Ok(options) => options,
        Err(message) => return bad_request(message),
    };
    options.keys = state.secrets.resolved();
    let project_dir = state.root().join(&project.id);
    match state.jobs.start_generate(&project, &project_dir, options) {
        Ok(job) => (StatusCode::ACCEPTED, Json(job)).into_response(),
        Err(error) => job_error(error),
    }
}

pub async fn resume_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(mut request): Json<GenerateRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((project_id, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    if request.provider.trim().is_empty() {
        request.provider = run.summary.provider.clone();
    }
    if request.model.is_none() && run.summary.model != "-" {
        request.model = Some(run.summary.model.clone());
    }
    let mut options = match GenerateOptions::from_request(&request) {
        Ok(options) => options,
        Err(message) => return bad_request(message),
    };
    options.keys = state.secrets.resolved();
    let project = store::Project {
        id: project_id.clone(),
        name: project_name,
        ..store::Project::default()
    };
    let project_dir = state.root().join(&project_id);
    match state
        .jobs
        .start_resume(&project, &project_dir, &run.summary.id, options)
    {
        Ok(job) => (StatusCode::ACCEPTED, Json(job)).into_response(),
        Err(error) => job_error(error),
    }
}

pub async fn continue_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<GenerateRequest>,
) -> Response {
    resume_run(State(state), Path(id), headers, Json(request)).await
}

pub async fn export_run_now(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ExportRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((project_id, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let options = match ExportOptions::from_request(&request) {
        Ok(options) => options,
        Err(message) => return bad_request(message),
    };
    let project = store::Project {
        id: project_id.clone(),
        name: project_name,
        ..store::Project::default()
    };
    let project_dir = state.root().join(&project_id);
    match state
        .jobs
        .start_export(&project, &project_dir, &run.summary.id, options)
    {
        Ok(job) => (StatusCode::ACCEPTED, Json(job)).into_response(),
        Err(error) => job_error(error),
    }
}

#[derive(Deserialize)]
pub struct RevisionRequest {
    edits: Vec<Edit>,
}

pub async fn create_revision(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RevisionRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((project_id, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    // Optimistic concurrency: a stale tab must not compose its edits over a
    // workspace another tab or export already rewrote.
    if let Some(expected) = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let current = mads_core::workspace::Workspace::load(&run.dir.join("workspace.json"))
            .ok()
            .map(|ws| store::account_hash(&ws.account))
            .unwrap_or_default();
        if !crate::serve::auth::same(expected, &current) {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": {"code": "CONFLICT", "message": "a conta mudou desde que você abriu; recarregue a página"},
                    "current_hash": current,
                })),
            )
                .into_response();
        }
    }
    let project = store::Project {
        id: project_id.clone(),
        name: project_name,
        ..store::Project::default()
    };
    let project_dir = state.root().join(&project_id);
    match state
        .jobs
        .start_revision(&project, &project_dir, &run.summary.id, &request.edits)
    {
        Ok((job, run_id)) => (
            StatusCode::ACCEPTED,
            Json(json!({"job_id": job.id, "run_id": run_id})),
        )
            .into_response(),
        Err(RevisionError::Invalid(issues)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": {"code": "INVALID", "message": "a conta ficaria inválida"},
                "issues": issues,
            })),
        )
            .into_response(),
        Err(error) => bad_request(error.message()),
    }
}

pub async fn revert_revision(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((project_id, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    if !run.summary.has_parent {
        return bad_request("esta execução não é uma revisão".into());
    }
    let parent_id = run.summary.parent.clone();
    if !store::valid_run_id(&parent_id) {
        return bad_request("a execução anterior tem um id inválido".into());
    }
    let parent_dir = run.dir.parent().map(|dir| dir.join(&parent_id));
    let Some(parent_dir) = parent_dir.filter(|dir| dir.is_dir()) else {
        return api_not_found();
    };
    let Ok(parent) = mads_core::workspace::Workspace::load(&parent_dir.join("workspace.json"))
    else {
        return bad_request("o workspace da execução anterior não carrega".into());
    };
    let project = store::Project {
        id: project_id.clone(),
        name: project_name,
        ..store::Project::default()
    };
    let project_dir = state.root().join(&project_id);
    let (run_id, _) = match revisions::create(&project_dir, &parent_id, &parent, &[]) {
        Ok(value) => value,
        Err(message) => return bad_request(message),
    };
    crate::serve::audit::append(
        &project_dir,
        "revision_reverted",
        &format!("{id} -> {run_id}"),
    );
    let options = ExportOptions {
        skip_url_check: false,
        layout: None,
        max_ad_groups: 50,
    };
    match state
        .jobs
        .start_export(&project, &project_dir, &run_id, options)
    {
        Ok(job) => (
            StatusCode::ACCEPTED,
            Json(json!({"job_id": job.id, "run_id": run_id})),
        )
            .into_response(),
        Err(error) => {
            // An export that cannot start leaves an empty -rN sibling behind.
            let _ = std::fs::remove_dir_all(project_dir.join("out").join(&run_id));
            job_error(error)
        }
    }
}

pub async fn api_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    match state.jobs.get(&id) {
        Some(job) => Json(job).into_response(),
        None => api_not_found(),
    }
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    if !state.jobs.cancel(&id) {
        return api_not_found();
    }
    Json(json!({"cancelled": true})).into_response()
}

pub async fn job_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some(view) = state.jobs.get(&id) else {
        return api_not_found();
    };
    let Some((mut live, log)) = state.jobs.subscribe(&id) else {
        return api_not_found();
    };
    let jobs = state.jobs.clone();
    let job_id = id.clone();
    let start = headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let (tx, rx) = mpsc::channel::<Result<SseEvent, Infallible>>(128);
    let done = matches!(
        view.state,
        jobs::JobState::Done | jobs::JobState::Failed | jobs::JobState::Cancelled
    );
    tokio::spawn(async move {
        let mut sent = start;
        let replay = jobs::read_log(&log);
        for envelope in &replay {
            if envelope.seq <= sent {
                continue;
            }
            sent = envelope.seq;
            let data = serde_json::to_string(envelope).unwrap_or_default();
            let event = SseEvent::default().id(envelope.seq.to_string()).data(data);
            if tx.send(Ok(event)).await.is_err() {
                return;
            }
        }
        if done {
            return;
        }
        loop {
            let next = tokio::time::timeout(Duration::from_secs(2), live.recv()).await;
            match next {
                Ok(Ok(envelope)) => {
                    if envelope.seq <= sent {
                        continue;
                    }
                    sent = envelope.seq;
                    let finished = matches!(envelope.event, Event::RunFinished { .. });
                    let data = serde_json::to_string(&envelope).unwrap_or_default();
                    let event = SseEvent::default().id(envelope.seq.to_string()).data(data);
                    if tx.send(Ok(event)).await.is_err() {
                        return;
                    }
                    if finished {
                        return;
                    }
                }
                Ok(Err(_)) => return,
                Err(_) => {
                    if jobs.get(&job_id).is_some_and(|job| {
                        matches!(
                            job.state,
                            jobs::JobState::Done
                                | jobs::JobState::Failed
                                | jobs::JobState::Cancelled
                        )
                    }) {
                        return;
                    }
                }
            }
        }
    });
    Sse::new(ReceiverStream::new(rx))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

pub async fn optimize_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "optimize", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::OptimizePage {
        chrome,
        title: format!("{} {}", tt(&identity.lang, "Otimizar"), run.summary.id),
        run,
    })
    .into_response()
}

pub async fn optimize_review_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let performance = store::performance_json(&run.dir).unwrap_or_else(|| "{}".into());
    let chrome = Chrome::new(identity.csrf.clone(), "optimize-review", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(views::OptimizeReviewPage {
        chrome,
        title: format!(
            "{} {}",
            tt(&identity.lang, "Revisar otimização"),
            run.summary.id
        ),
        run,
        performance_json: performance,
        default_provider: std::env::var("MADS_PROVIDER").unwrap_or_default(),
        default_model: std::env::var("MADS_MODEL").unwrap_or_default(),
    })
    .into_response()
}

pub async fn optimize_prepare(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((project_id, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let project_dir = state.root().join(&project_id);
    let upload = project_dir
        .join(".mads")
        .join("uploads")
        .join(uuid::Uuid::new_v4().simple().to_string());
    if let Err(e) = std::fs::create_dir_all(&upload) {
        return bad_request(format!("não foi possível preparar o upload: {e}"));
    }
    let mut files = 0usize;
    let mut size = 0usize;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&upload);
                return bad_request(format!("upload inválido: {e}"));
            }
        };
        let name = field
            .file_name()
            .map(str::to_string)
            .unwrap_or_else(|| format!("report-{files}.csv"));
        if !name.to_ascii_lowercase().ends_with(".csv") {
            let _ = std::fs::remove_dir_all(&upload);
            return bad_request("envie arquivos .csv exportados do Google Ads".into());
        }
        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&upload);
                return bad_request(format!("não foi possível ler o arquivo: {e}"));
            }
        };
        size += bytes.len();
        files += 1;
        if files > 20 || size > 20 * 1024 * 1024 {
            let _ = std::fs::remove_dir_all(&upload);
            return bad_request("envie no máximo 20 arquivos e 20 MB".into());
        }
        let safe = name
            .rsplit('/')
            .next()
            .unwrap_or("report.csv")
            .replace(['\\', '"'], "_");
        if let Err(e) = std::fs::write(upload.join(safe), bytes) {
            let _ = std::fs::remove_dir_all(&upload);
            return bad_request(format!("não foi possível salvar o arquivo: {e}"));
        }
    }
    if files == 0 {
        let _ = std::fs::remove_dir_all(&upload);
        return bad_request("envie pelo menos um CSV".into());
    }
    let out = project_dir.join("out");
    let prepared = match optimize::prepare(&run.dir, &upload, &out) {
        Ok(prepared) => prepared,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&upload);
            return bad_request(e.to_string());
        }
    };
    let _ = std::fs::remove_dir_all(&upload);
    crate::serve::audit::append(
        &project_dir,
        "optimize_prepared",
        &prepared.run_dir.display().to_string(),
    );
    let run_id = prepared
        .run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    (
        StatusCode::CREATED,
        Json(json!({"run_id": run_id, "project_id": project_id})),
    )
        .into_response()
}

pub async fn connections_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let (providers, image_providers) = views::providers_with(&state.secrets);
    let keys = state.secrets.statuses();
    Html(views::ConnectionsPage {
        chrome: Chrome::new(identity.csrf.clone(), "connections", &identity.lang),
        title: tt(&identity.lang, "Conexões"),
        providers,
        image_providers,
        key_file: state.secrets.path().display().to_string(),
        keys,
        secrets_error: state.secrets.error().unwrap_or_default(),
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct SecretRequest {
    value: String,
}

pub async fn api_secrets(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    Json(json!({
        "file": state.secrets.path().display().to_string(),
        "error": state.secrets.error(),
        "keys": state.secrets.statuses().iter().map(|status| json!({
            "provider": status.provider,
            "source": status.source_label,
            "ready": status.ready,
            "needs_key": status.needs_key,
        })).collect::<Vec<_>>()
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct ModelsQuery {
    #[serde(default)]
    base_url: Option<String>,
}

/// The model ids a provider serves today. The forms load it into a datalist and
/// still accept a free text id.
pub async fn api_models(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Query(query): Query<ModelsQuery>,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let known = mads_providers::list_providers()
        .iter()
        .any(|provider| provider.name == name);
    if !known {
        return api_not_found();
    }
    // Every request here becomes one call to the provider's API with the
    // operator's key: keep a tab or a script from hammering it.
    if !state
        .limit
        .allow("provider-models", 30, std::time::Duration::from_secs(60))
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": {"code": "RATE_LIMITED", "message": "muitas consultas de modelos; aguarde um minuto"}})),
        )
            .into_response();
    }
    let key = crate::serve::secrets::env_for(&name)
        .and_then(|var| state.secrets.resolved().get(var).cloned());
    // The endpoint falls back to the server's own variable, then the user edits
    // it in the form.
    let base_url = query
        .base_url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("MADS_BASE_URL").ok());
    match mads_providers::list_models(&name, key.as_deref(), base_url.as_deref()).await {
        Ok(models) => Json(json!({"provider": name, "models": models})).into_response(),
        Err(message) => bad_request(message),
    }
}

pub async fn set_secret(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SecretRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    match state.secrets.set(&name, &request.value) {
        Ok(()) => {
            crate::serve::audit::append(state.root(), "key_saved", &name);
            Json(json!({"ok": true})).into_response()
        }
        Err(message) => bad_request(message),
    }
}

pub async fn delete_secret(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    match state.secrets.remove(&name) {
        Ok(()) => {
            crate::serve::audit::append(state.root(), "key_removed", &name);
            Json(json!({"ok": true})).into_response()
        }
        Err(message) => bad_request(message),
    }
}

#[derive(Deserialize)]
pub struct LangQuery {
    lang: String,
    #[serde(default)]
    next: Option<String>,
}

pub async fn set_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LangQuery>,
) -> Response {
    let lang = if query.lang == crate::serve::i18n::EN {
        crate::serve::i18n::EN
    } else {
        crate::serve::i18n::PT_BR
    };
    state.auth.set_language(&headers, lang);
    let next = query
        .next
        .filter(|value| {
            value.starts_with('/') && !value.starts_with("//") && !value.starts_with("/\\")
        })
        .unwrap_or_else(|| "/".into());
    let mut response = axum::response::Redirect::to(&next).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&crate::serve::i18n::cookie(lang)) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

pub async fn health(State(state): State<AppState>) -> Response {
    Json(json!({
        "name": "mads studio",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.started.elapsed().as_secs(),
        "active_jobs": state.jobs.active_count(),
        "address": format!("{}:{}", state.config.bind, state.config.port),
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct SessionRequest {
    token: String,
}

pub async fn session_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SessionRequest>,
) -> Response {
    let Some((id, csrf)) = state
        .auth
        .create_session_with_lang(request.token.trim(), &crate::serve::i18n::lang_of(&headers))
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "UNAUTHORIZED", "message": "token inválido"}})),
        )
            .into_response();
    };
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"));
    let mut response = Json(json!({"csrf": csrf})).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&Auth::cookie(&id, secure)) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

pub async fn session_delete(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.auth.destroy_session(&headers);
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Ok(cookie) = HeaderValue::from_str(&Auth::clear_cookie()) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

pub async fn api_projects(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let projects: Vec<serde_json::Value> = store::projects(state.root())
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "url": p.url,
                "daily": p.daily,
                "currency": p.currency,
                "error": p.error_text,
                "has_error": p.has_error,
                "runs": p.runs.iter().map(|r| json!({
                    "id": r.id,
                    "status": r.status,
                    "label": r.label,
                    "provider": r.provider,
                    "model": r.model,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Json(json!({"projects": projects})).into_response()
}

pub async fn api_draft(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some(project) = store::project(state.root(), &id) else {
        return api_not_found();
    };
    Json(draft::load(&state.root().join(&project.id))).into_response()
}

pub async fn api_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let workspace = mads_core::workspace::Workspace::load(&run.dir.join("workspace.json"));
    match workspace {
        Ok(workspace) => Json(json!({
            "project": project_name,
            "run": run.summary,
            "account": workspace.account,
            "account_hash": store::account_hash(&workspace.account),
        }))
        .into_response(),
        Err(error) => bad_request(format!("workspace não carrega: {error}")),
    }
}

pub async fn api_validation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    Json(json!({
        "errors": run.errors,
        "warnings": run.warnings,
        "urls": run.urls,
    }))
    .into_response()
}

pub async fn api_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    match std::fs::read_to_string(run.dir.join("report.md")) {
        Ok(text) => (
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            text,
        )
            .into_response(),
        Err(_) => api_not_found(),
    }
}

pub async fn api_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    Json(json!({"files": run.files})).into_response()
}

pub async fn api_validate_edits(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RevisionRequest>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let Ok(workspace) = mads_core::workspace::Workspace::load(&run.dir.join("workspace.json"))
    else {
        return bad_request("workspace não carrega".into());
    };
    match revisions::apply(&workspace.account, &workspace.input, &request.edits, 50) {
        Ok(_) => Json(json!({"ok": true})).into_response(),
        Err(RevisionError::Invalid(issues)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"ok": false, "issues": issues})),
        )
            .into_response(),
        Err(error) => bad_request(error.message()),
    }
}

pub async fn api_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    Json(json!({
        "project": project_name,
        "run": {
            "id": run.summary.id,
            "status": run.summary.status,
            "label": run.summary.label,
            "provider": run.summary.provider,
            "model": run.summary.model,
            "origin": run.summary.origin,
            "revision": run.summary.revision,
            "when": run.summary.when,
            "tokens": run.summary.tokens,
            "cost": run.summary.cost,
            "errors": run.errors.len(),
            "warnings": run.warnings.len(),
        }
    }))
    .into_response()
}

pub async fn api_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.auth.authenticate(&headers).is_none() {
        return api_not_authorized();
    }
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let events = store::read_run_events(&run.dir);
    Json(json!({
        "events": events.iter().map(|e| serde_json::to_value(e).unwrap_or_default()).collect::<Vec<_>>()
    }))
    .into_response()
}

pub async fn stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return api_not_authorized();
    };
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let start = headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let _ = identity;
    let (tx, rx) = mpsc::channel::<Result<SseEvent, Infallible>>(128);
    let dir = run.dir.clone();
    tokio::spawn(async move {
        let mut sent = start;
        let mut idle = 0u32;
        loop {
            let events = store::read_run_events(&dir);
            for envelope in &events {
                if envelope.seq <= sent {
                    continue;
                }
                sent = envelope.seq;
                // Unnamed events reach `onmessage`, which is all the frontend
                // listens to; the type stays inside the JSON payload.
                let data = serde_json::to_string(envelope).unwrap_or_default();
                let event = SseEvent::default().id(envelope.seq.to_string()).data(data);
                if tx.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            if events
                .iter()
                .any(|e| matches!(e.event, Event::RunFinished { .. }))
            {
                return;
            }
            if idle >= 120 {
                return;
            }
            idle += 1;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    Sse::new(ReceiverStream::new(rx))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

pub async fn file(
    State(state): State<AppState>,
    Path((id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return api_not_authorized();
    };
    let _ = identity;
    let Some((_, _, run)) = store::find_run(state.root(), &id) else {
        return api_not_found();
    };
    let Some(data) = store::read_file(&run.dir, &path) else {
        return api_not_found();
    };
    let mut response = data.bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&data.content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if !data.image {
        let disposition = format!("attachment; filename=\"{}\"", data.name.replace('"', ""));
        if let Ok(value) = HeaderValue::from_str(&disposition) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

pub async fn not_found(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let identity = state.auth.authenticate(&headers);
    let chrome = identity
        .as_ref()
        .map(|i| Chrome::new(i.csrf.clone(), "not-found", &i.lang))
        .unwrap_or_else(|| {
            let lang = crate::serve::i18n::lang_of(&headers);
            Chrome::new(String::new(), "not-found", &lang)
        });
    (
        StatusCode::NOT_FOUND,
        Html(views::ErrorPage {
            chrome,
            title: "Não encontrado".into(),
            message: "Esta página não existe.".into(),
        }),
    )
        .into_response()
}

fn bootstrap(headers: &HeaderMap) -> Response {
    // The sign-in page is a page, not an API error: a 200 keeps the browser
    // console clean on first visit.
    Html(views::BootstrapPage {
        title: "mads studio".into(),
        lang: crate::serve::i18n::lang_of(headers),
    })
    .into_response()
}

fn not_found_page(identity: &Identity, message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(views::ErrorPage {
            chrome: Chrome::new(identity.csrf.clone(), "not-found", &identity.lang),
            title: "Não encontrado".into(),
            message: message.into(),
        }),
    )
        .into_response()
}

pub(crate) fn mutation_identity(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Identity, Box<Response>> {
    let Some(identity) = state.auth.authenticate(headers) else {
        return Err(Box::new(api_not_authorized()));
    };
    if identity.bearer {
        return Ok(identity);
    }
    let token = headers
        .get(CSRF_HEADER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    if crate::serve::auth::same(token, &identity.csrf) {
        Ok(identity)
    } else {
        Err(Box::new(
            (
                StatusCode::FORBIDDEN,
                Json(json!({"error": {"code": "CSRF", "message": "token CSRF inválido"}})),
            )
                .into_response(),
        ))
    }
}

pub(crate) fn bad_request(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": "BAD_REQUEST", "message": message}})),
    )
        .into_response()
}

pub(crate) fn conflict(message: String) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({"error": {"code": "CONFLICT", "message": message}})),
    )
        .into_response()
}

/// A job that could not start: a conflict answers 409, anything else 400.
pub(crate) fn job_error(error: crate::serve::jobs::JobError) -> Response {
    match error {
        crate::serve::jobs::JobError::Conflict => {
            conflict("este projeto já tem uma execução em andamento".into())
        }
        crate::serve::jobs::JobError::Message(message) => bad_request(message),
    }
}

pub(crate) fn api_not_authorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"code": "UNAUTHORIZED", "message": "sessão inválida"}})),
    )
        .into_response()
}

pub(crate) fn api_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": {"code": "NOT_FOUND", "message": "execução não encontrada"}})),
    )
        .into_response()
}
