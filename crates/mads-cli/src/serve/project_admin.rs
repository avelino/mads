//! Project management in mads studio: archive, delete, import and manual creation.
//!
//! Delete never calls `remove_dir_all`: the project directory moves into
//! `<root>/.trash/<id>-<utc>`, so an accidental click is recoverable from the
//! terminal. Import only copies regular files and directories, never symlinks.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use askama::Template;
use axum::{
    Json,
    extract::{Path as RoutePath, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use mads_core::{
    input::{normalize_url, parse_input_toml, slugify},
    money::Cents,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;

use crate::serve::{
    AppState, audit,
    auth::Identity,
    draft::{self, DraftPayload},
    handlers::{api_not_found, bad_request, mutation_identity},
    store::{self, Project},
    views::{Chrome, Html},
};

/// The project management routes, merged by `serve::app`.
pub fn router() -> axum::Router<crate::serve::AppState> {
    axum::Router::new()
        .route("/projects/import", get(import_page))
        .route("/projects/manual", get(manual_page))
        .route("/projects/{id}/settings", get(settings_page))
        .route("/api/projects/import", post(import_project))
        .route("/api/projects/manual", post(manual_project))
        .route("/api/projects/{id}/archive", post(archive_project))
        .route("/api/projects/{id}/delete", post(delete_project))
}

#[derive(Template)]
#[template(path = "project_settings.html")]
pub struct SettingsPage {
    pub chrome: Chrome,
    pub title: String,
    pub project: Project,
    pub path: String,
    pub trash: String,
}

#[derive(Template)]
#[template(path = "project_import.html")]
pub struct ImportPage {
    pub chrome: Chrome,
    pub title: String,
}

#[derive(Template)]
#[template(path = "project_manual.html")]
pub struct ManualPage {
    pub chrome: Chrome,
    pub title: String,
}

pub async fn settings_page(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return redirect_home();
    };
    let Some(project) = store::project(state.root(), &id) else {
        return not_found_page(&identity, "Projeto não encontrado");
    };
    let dir = state.root().join(&project.id);
    let path = dir.canonicalize().unwrap_or(dir).display().to_string();
    let trash = state.root().join(".trash").display().to_string();
    Html(SettingsPage {
        chrome: Chrome::new(identity.csrf.clone(), "project", &identity.lang)
            .with_project(&project.name),
        title: format!("Configurações · {}", project.name),
        project,
        path,
        trash,
    })
    .into_response()
}

pub async fn import_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return redirect_home();
    };
    Html(ImportPage {
        chrome: Chrome::new(identity.csrf.clone(), "new-project", &identity.lang),
        title: "Importar projeto".into(),
    })
    .into_response()
}

pub async fn manual_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return redirect_home();
    };
    Html(ManualPage {
        chrome: Chrome::new(identity.csrf.clone(), "new-project", &identity.lang),
        title: "Novo projeto manual".into(),
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ArchiveRequest {
    archived: bool,
}

pub async fn archive_project(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    headers: HeaderMap,
    payload: Result<Json<ArchiveRequest>, JsonRejection>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let request = match json_body(payload) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let Some(dir) = safe_project_dir(state.root(), &id) else {
        return api_not_found();
    };
    if let Err(message) = set_archived(&dir, request.archived) {
        return bad_request(message);
    }
    audit::append(
        &dir,
        "project_archived",
        if request.archived {
            "archived=true"
        } else {
            "archived=false"
        },
    );
    Json(json!({
        "ok": true,
        "project_id": id,
        "archived": request.archived,
    }))
    .into_response()
}

pub async fn delete_project(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    if let Err(response) = json_body(payload) {
        return *response;
    }
    let Some(dir) = safe_project_dir(state.root(), &id) else {
        return api_not_found();
    };
    let trash = state.root().join(".trash");
    if let Err(e) = fs::create_dir_all(&trash) {
        return bad_request(format!("não foi possível criar a lixeira: {e}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&trash, fs::Permissions::from_mode(0o700));
    }
    let target = trash_path(&trash, &id);
    if let Err(e) = fs::rename(&dir, &target) {
        return bad_request(format!(
            "não foi possível mover o projeto para a lixeira: {e}"
        ));
    }
    audit::append(
        state.root(),
        "project_deleted",
        &format!("{id} -> {}", target.display()),
    );
    Json(json!({"ok": true})).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    path: String,
}

pub async fn import_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<ImportRequest>, JsonRejection>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let request = match json_body(payload) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let raw = request.path.trim();
    if raw.is_empty() {
        return bad_request("informe o caminho da pasta para importar".into());
    }
    let source = match Path::new(raw).canonicalize() {
        Ok(path) => path,
        Err(e) => return bad_request(format!("não foi possível abrir a pasta: {e}")),
    };
    if !source.is_dir() {
        return bad_request("o caminho precisa ser uma pasta".into());
    }
    let root = match state.root().canonicalize() {
        Ok(path) => path,
        Err(e) => return bad_request(format!("não foi possível abrir a raiz do estúdio: {e}")),
    };
    if root.starts_with(&source) {
        return bad_request("importe uma pasta que esteja fora da raiz do estúdio".into());
    }
    let business = source.join("business.toml");
    let metadata = match fs::symlink_metadata(&business) {
        Ok(metadata) => metadata,
        Err(_) => return bad_request("a pasta precisa conter um business.toml".into()),
    };
    if metadata.file_type().is_symlink() {
        return bad_request("business.toml não pode ser um link simbólico".into());
    }
    if !metadata.is_file() {
        return bad_request("a pasta precisa conter um business.toml".into());
    }
    let text = match fs::read_to_string(&business) {
        Ok(text) => text,
        Err(e) => return bad_request(format!("não foi possível ler business.toml: {e}")),
    };
    let parsed = match parse_input_toml(&text) {
        Ok(parsed) => parsed,
        Err(e) => return bad_request(format!("business.toml inválido: {e}")),
    };
    let name = parsed.business.name.trim().to_string();
    let base = slugify(&name);
    if base.is_empty() {
        return bad_request("o nome do negócio precisa de letras ou números".into());
    }
    let Some(id) = next_project_id(state.root(), &base) else {
        return bad_request("já existem 99 projetos com este nome; escolha outro nome".into());
    };
    let target = match create_project_dir(&root, &id) {
        Ok(dir) => dir,
        Err(message) => return bad_request(message),
    };
    if let Err(message) = copy_tree(&source, &target) {
        let _ = fs::remove_dir_all(&target);
        return bad_request(message);
    }
    if let Err(message) = write_project_json(&target, json!({"name": name})) {
        let _ = fs::remove_dir_all(&target);
        return bad_request(message);
    }
    audit::append(
        &root,
        "project_imported",
        &format!("{} -> {id}", source.display()),
    );
    Json(json!({"project_id": id, "name": name})).into_response()
}

/// A manual project is a `DraftPayload` without the agent: `draft::save` writes
/// `business.toml` and validates it with the real parser before the project
/// appears in the list.
#[derive(Debug, Deserialize)]
pub struct ManualRequest {
    name: String,
    url: String,
    daily: f64,
    currency: String,
    language: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    goal: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    conversion_tracking: bool,
    #[serde(default)]
    brand_terms: String,
    #[serde(default)]
    competitors: String,
    #[serde(default)]
    avoid: String,
    #[serde(default)]
    restricted: Vec<String>,
    #[serde(default)]
    max_cpc: Option<f64>,
    #[serde(default)]
    export_status: Option<String>,
    #[serde(default)]
    url_suffix: String,
    #[serde(default)]
    eu_political_ads: bool,
    #[serde(default)]
    decimal_comma: bool,
    #[serde(default)]
    formats: Vec<String>,
    #[serde(default)]
    catalog_text: String,
}

impl ManualRequest {
    fn into_payload(self) -> DraftPayload {
        DraftPayload {
            name: self.name,
            url: self.url,
            language: self.language,
            location: self.location,
            goal: self.goal,
            description: self.description,
            conversion_tracking: self.conversion_tracking,
            brand_terms: self.brand_terms,
            competitors: self.competitors,
            avoid: self.avoid,
            restricted: self.restricted,
            daily: self.daily,
            currency: self.currency,
            max_cpc: self.max_cpc,
            export_status: self.export_status.unwrap_or_else(|| "Paused".into()),
            url_suffix: self.url_suffix,
            eu_political_ads: self.eu_political_ads,
            decimal_comma: self.decimal_comma,
            formats: self.formats,
            catalog_text: self.catalog_text,
            ..DraftPayload::default()
        }
    }
}

pub async fn manual_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<ManualRequest>, JsonRejection>,
) -> Response {
    if let Err(response) = mutation_identity(&state, &headers) {
        return *response;
    }
    let request = match json_body(payload) {
        Ok(request) => request,
        Err(response) => return *response,
    };
    let name = request.name.trim().to_string();
    if !(1..=80).contains(&name.chars().count()) {
        return bad_request("o nome precisa de 1 a 80 caracteres".into());
    }
    if normalize_url(&request.url).is_none() {
        return bad_request("a URL precisa ser http(s)".into());
    }
    if Cents::from_f64(request.daily).is_none() {
        return bad_request("o orçamento diário precisa ser positivo, com até 2 casas".into());
    }
    let currency = request.currency.trim().to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return bad_request("a moeda precisa de 3 letras, como BRL".into());
    }
    let base = slugify(&name);
    if base.is_empty() {
        return bad_request("o nome precisa de letras ou números".into());
    }
    let Some(id) = next_project_id(state.root(), &base) else {
        return bad_request("já existem 99 projetos com este nome; escolha outro nome".into());
    };
    let dir = match create_project_dir(state.root(), &id) {
        Ok(dir) => dir,
        Err(message) => return bad_request(message),
    };
    let mut payload = request.into_payload();
    payload.name = name.clone();
    payload.currency = currency;
    if let Err(message) = draft::save(&dir, &payload) {
        let _ = fs::remove_dir_all(&dir);
        return bad_request(message);
    }
    if let Err(message) = write_project_json(&dir, json!({"name": name})) {
        let _ = fs::remove_dir_all(&dir);
        return bad_request(message);
    }
    audit::append(&dir, "project_created", &name);
    Json(json!({"project_id": id})).into_response()
}

fn json_body<T: serde::de::DeserializeOwned>(
    payload: Result<Json<T>, JsonRejection>,
) -> Result<T, Box<Response>> {
    match payload {
        Ok(Json(value)) => Ok(value),
        Err(rejection) => Err(Box::new(bad_request(format!(
            "corpo JSON inválido: {}",
            rejection.body_text()
        )))),
    }
}

fn redirect_home() -> Response {
    Redirect::to("/").into_response()
}

fn not_found_page(identity: &Identity, message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(crate::serve::views::ErrorPage {
            chrome: Chrome::new(identity.csrf.clone(), "not-found", &identity.lang),
            title: "Não encontrado".into(),
            message: message.into(),
        }),
    )
        .into_response()
}

/// The id of a project directory: one normal path component, never hidden.
fn valid_project_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A project directory that really is a direct child of the root. Symlinks are
/// refused, so a rename can never move something outside the studio.
fn safe_project_dir(root: &Path, id: &str) -> Option<PathBuf> {
    if !valid_project_id(id) {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let dir = root.join(id);
    let metadata = fs::symlink_metadata(&dir).ok()?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return None;
    }
    let canonical = dir.canonicalize().ok()?;
    (canonical.parent() == Some(root.as_path())).then_some(dir)
}

/// The first free `base`, `base-2` … `base-99` id. `None` means the range is full.
fn next_project_id(root: &Path, base: &str) -> Option<String> {
    let first = base.to_string();
    if !root.join(&first).exists() {
        return Some(first);
    }
    for attempt in 2..=99 {
        let candidate = format!("{base}-{attempt}");
        if !root.join(&candidate).exists() {
            return Some(candidate);
        }
    }
    None
}

fn create_project_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !valid_project_id(id) {
        return Err("id de projeto inválido".into());
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("não foi possível abrir a raiz do estúdio: {e}"))?;
    let dir = root.join(id);
    if dir.exists() {
        return Err("já existe um projeto com este id".into());
    }
    fs::create_dir(&dir).map_err(|e| format!("não foi possível criar o projeto: {e}"))?;
    let canonical = match dir.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            let _ = fs::remove_dir(&dir);
            return Err(format!("não foi possível abrir o projeto: {e}"));
        }
    };
    if canonical.parent() != Some(root.as_path()) {
        let _ = fs::remove_dir(&dir);
        return Err("o projeto precisa ser filho direto da raiz".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Updates only `archived` in `project.json`, keeping every other key (`name`,
/// `init` and anything a future version wrote).
fn set_archived(dir: &Path, archived: bool) -> Result<(), String> {
    let path = dir.join("project.json");
    let mut object = match fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(object)) => object,
            Ok(_) => return Err("project.json precisa ser um objeto JSON".into()),
            Err(e) => return Err(format!("project.json inválido: {e}")),
        },
        Ok(_) => Map::new(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(format!("não foi possível ler project.json: {e}")),
    };
    object.insert("archived".into(), Value::Bool(archived));
    let text = serde_json::to_vec_pretty(&Value::Object(object)).map_err(|e| e.to_string())?;
    write_atomic(&path, &text)
}

fn write_project_json(dir: &Path, value: Value) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
    write_atomic(&dir.join("project.json"), &text)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, bytes)
        .map_err(|e| format!("não foi possível escrever {}: {e}", path.display()))?;
    fs::rename(&temporary, path).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("não foi possível salvar {}: {e}", path.display())
    })
}

fn utc_stamp() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn trash_path(trash: &Path, id: &str) -> PathBuf {
    let stamp = utc_stamp();
    for attempt in 1..=1000 {
        let name = if attempt == 1 {
            format!("{id}-{stamp}")
        } else {
            format!("{id}-{stamp}-{attempt}")
        };
        let candidate = trash.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    trash.join(format!("{id}-{stamp}-overflow"))
}

/// Copies regular files and directories, skipping symlinks and the `.git` and
/// `.mads` directories at any depth. Permissions are copied where cheap.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let entries =
        fs::read_dir(from).map_err(|e| format!("não foi possível ler {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("não foi possível ler {}: {e}", from.display()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == ".mads" {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|e| format!("não foi possível ler {}: {e}", entry.path().display()))?;
        if kind.is_symlink() {
            continue;
        }
        let source = entry.path();
        let target = to.join(name.as_ref());
        if kind.is_dir() {
            fs::create_dir(&target)
                .map_err(|e| format!("não foi possível criar {}: {e}", target.display()))?;
            copy_tree(&source, &target)?;
            copy_permissions(&source, &target);
        } else if kind.is_file() {
            fs::copy(&source, &target)
                .map_err(|e| format!("não foi possível copiar {}: {e}", source.display()))?;
            copy_permissions(&source, &target);
        }
    }
    Ok(())
}

fn copy_permissions(from: &Path, to: &Path) {
    if let Ok(metadata) = fs::metadata(from) {
        let _ = fs::set_permissions(to, metadata.permissions());
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use tower::ServiceExt;

    use super::*;

    const BUSINESS: &str = r#"
[business]
name = "Acme"
url = "https://acme.com"
language = "pt-BR"
locations = ["Brasil"]
goal = "vender"
description = "Uma descrição com mais de vinte caracteres para o teste."

[budget]
daily = 50
currency = "BRL"
"#;

    fn test_state(root: &Path) -> AppState {
        AppState::new(crate::serve::Config {
            root: root.to_path_buf(),
            bind: "127.0.0.1".parse().unwrap(),
            port: 4780,
            token: "a".repeat(64),
            allow_remote: false,
            open: false,
            print_token: true,
        })
    }

    async fn send(state: AppState, method: &str, uri: &str, body: Option<&str>) -> Response {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", "a".repeat(64)));
        if body.is_some() {
            builder = builder
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::HOST, "127.0.0.1:4780");
        }
        let body = body.unwrap_or_default().to_string();
        let request = builder.body(Body::from(body)).unwrap();
        axum::Router::new()
            .merge(router())
            .with_state(state)
            .oneshot(request)
            .await
            .unwrap()
    }

    async fn body_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn the_archive_endpoint_preserves_name_and_init() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("acme");
        fs::create_dir(&project).unwrap();
        let init = json!({"start_url": "https://acme.com"});
        fs::write(
            project.join("project.json"),
            json!({"name": "Acme", "init": init}).to_string(),
        )
        .unwrap();

        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/acme/archive",
            Some(r#"{"archived":true}"#),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let value: Value =
            serde_json::from_str(&fs::read_to_string(project.join("project.json")).unwrap())
                .unwrap();
        assert_eq!(value["archived"], true);
        assert_eq!(value["name"], "Acme");
        assert_eq!(value["init"], init);
        let audit = fs::read_to_string(project.join(".mads/audit.ndjson")).unwrap();
        assert!(audit.contains("project_archived"));
    }

    #[tokio::test]
    async fn the_archive_endpoint_needs_json() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("acme")).unwrap();
        let request = Request::builder()
            .method("POST")
            .uri("/api/projects/acme/archive")
            .header(header::AUTHORIZATION, format!("Bearer {}", "a".repeat(64)))
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("archived=true"))
            .unwrap();
        let response = axum::Router::new()
            .merge(router())
            .with_state(test_state(root.path()))
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn the_delete_endpoint_moves_the_project_to_trash() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("acme");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("business.toml"), BUSINESS).unwrap();

        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/acme/delete",
            Some("{}"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!project.exists());
        let trash: Vec<String> = fs::read_dir(root.path().join(".trash"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(trash.len(), 1);
        assert!(trash[0].starts_with("acme-"));
        let audit = fs::read_to_string(root.path().join(".mads/audit.ndjson")).unwrap();
        assert!(audit.contains("project_deleted"));
    }

    #[tokio::test]
    async fn the_import_endpoint_copies_a_folder_with_business_toml() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("business.toml"), BUSINESS).unwrap();
        fs::create_dir_all(source.path().join(".git")).unwrap();
        fs::write(source.path().join(".git/config"), "secret").unwrap();
        let body = json!({"path": source.path().to_string_lossy().into_owned()}).to_string();

        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/import",
            Some(&body),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = body_json(response).await;
        let id = payload["project_id"].as_str().unwrap();
        assert_eq!(id, "acme");
        let target = root.path().join(id);
        assert!(target.join("business.toml").is_file());
        assert!(!target.join(".git").exists());
        let meta: Value =
            serde_json::from_str(&fs::read_to_string(target.join("project.json")).unwrap())
                .unwrap();
        assert_eq!(meta["name"], "Acme");
    }

    #[tokio::test]
    async fn the_import_endpoint_rejects_a_folder_without_business_toml() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("README.md"), "nada").unwrap();
        let body = json!({"path": source.path().to_string_lossy().into_owned()}).to_string();

        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/import",
            Some(&body),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!root.path().join("acme").exists());
    }

    #[tokio::test]
    async fn the_manual_endpoint_writes_a_valid_business_toml() {
        let root = tempfile::tempdir().unwrap();
        let body = json!({
            "name": "Vinellu",
            "url": "https://vinellu.com",
            "daily": 50.0,
            "currency": "BRL",
            "language": "pt-BR",
            "location": "Brasil",
            "goal": "vender vinho",
            "description": "Uma loja de vinhos com entrega no mesmo dia e curadoria.",
        })
        .to_string();

        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/manual",
            Some(&body),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = body_json(response).await;
        assert_eq!(payload["project_id"], "vinellu");
        let dir = root.path().join("vinellu");
        assert!(dir.join("business.toml").is_file());
        assert!(mads_core::input::load_input(&dir.join("business.toml")).is_ok());
        let meta: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        assert_eq!(meta["name"], "Vinellu");
    }

    #[tokio::test]
    async fn a_missing_project_is_a_404() {
        let root = tempfile::tempdir().unwrap();
        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/nope/archive",
            Some(r#"{"archived":true}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = send(
            test_state(root.path()),
            "POST",
            "/api/projects/nope/delete",
            Some("{}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn the_router_registers_static_and_dynamic_project_routes() {
        let _: axum::Router<AppState> = router();
    }

    #[test]
    fn a_taken_slug_gets_a_suffix() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_project_id(dir.path(), "acme").as_deref(), Some("acme"));
        fs::create_dir(dir.path().join("acme")).unwrap();
        assert_eq!(
            next_project_id(dir.path(), "acme").as_deref(),
            Some("acme-2")
        );
        fs::create_dir(dir.path().join("acme-2")).unwrap();
        assert_eq!(
            next_project_id(dir.path(), "acme").as_deref(),
            Some("acme-3")
        );
    }

    #[test]
    fn a_full_slug_range_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("acme")).unwrap();
        for attempt in 2..=99 {
            fs::create_dir(dir.path().join(format!("acme-{attempt}"))).unwrap();
        }
        assert!(next_project_id(dir.path(), "acme").is_none());
    }

    #[test]
    fn a_project_id_never_escapes_the_root() {
        assert!(valid_project_id("acme"));
        assert!(valid_project_id("acme-2"));
        assert!(!valid_project_id(""));
        assert!(!valid_project_id(".hidden"));
        assert!(!valid_project_id("../acme"));
        assert!(!valid_project_id("a/b"));
        assert!(!valid_project_id("a\\b"));
    }

    #[test]
    fn archiving_keeps_name_and_init() {
        let dir = tempfile::tempdir().unwrap();
        let init = json!({"start_url": "https://acme.com", "provider": "replay"});
        fs::write(
            dir.path().join("project.json"),
            json!({
                "name": "Acme",
                "init": init,
                "archived": false,
                "extra": "keep me",
            })
            .to_string(),
        )
        .unwrap();
        set_archived(dir.path(), true).unwrap();
        let value: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("project.json")).unwrap())
                .unwrap();
        assert_eq!(value["name"], "Acme");
        assert_eq!(value["init"], init);
        assert_eq!(value["archived"], true);
        assert_eq!(value["extra"], "keep me");

        set_archived(dir.path(), false).unwrap();
        let value: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("project.json")).unwrap())
                .unwrap();
        assert_eq!(value["archived"], false);
        assert_eq!(value["init"], init);
    }

    #[test]
    fn archiving_creates_project_json_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        set_archived(dir.path(), true).unwrap();
        let value: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("project.json")).unwrap())
                .unwrap();
        assert_eq!(value["archived"], true);
    }

    #[test]
    fn a_broken_project_json_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("project.json"), "{not json").unwrap();
        assert!(set_archived(dir.path(), true).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("project.json")).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn a_copy_skips_symlinks_and_private_directories() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir_all(source.path().join("assets")).unwrap();
        fs::write(source.path().join("assets/logo.txt"), "logo").unwrap();
        fs::write(source.path().join("business.toml"), "[business]").unwrap();
        fs::create_dir_all(source.path().join(".git")).unwrap();
        fs::write(source.path().join(".git/config"), "secret").unwrap();
        fs::create_dir_all(source.path().join(".mads")).unwrap();
        fs::write(source.path().join(".mads/audit.ndjson"), "{}").unwrap();
        fs::create_dir_all(source.path().join("out/run")).unwrap();
        fs::write(source.path().join("out/run/workspace.json"), "{}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            source.path().join("business.toml"),
            source.path().join("linked.toml"),
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            source.path().join("assets"),
            source.path().join("linked-dir"),
        )
        .unwrap();

        copy_tree(source.path(), target.path()).unwrap();

        assert!(target.path().join("business.toml").is_file());
        assert!(target.path().join("assets/logo.txt").is_file());
        assert!(target.path().join("out/run/workspace.json").is_file());
        assert!(!target.path().join(".git").exists());
        assert!(!target.path().join(".mads").exists());
        #[cfg(unix)]
        {
            assert!(!target.path().join("linked.toml").exists());
            assert!(!target.path().join("linked-dir").exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_keeps_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let file = source.path().join("business.toml");
        fs::write(&file, "[business]").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();

        copy_tree(source.path(), target.path()).unwrap();

        let mode = fs::metadata(target.path().join("business.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[test]
    fn the_trash_stamp_is_a_utc_timestamp() {
        let stamp = utc_stamp();
        assert_eq!(stamp.len(), 15);
        assert_eq!(stamp.as_bytes()[8], b'-');
        assert!(stamp.chars().all(|c| c.is_ascii_digit() || c == '-'));
    }
}
