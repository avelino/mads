pub mod assets;
pub mod audit;
pub mod auth;
pub mod diff;
pub mod draft;
pub mod handlers;
pub mod i18n;
pub mod jobs;
pub mod limit;
pub mod performance;
pub mod project_admin;
pub mod revisions;
pub mod secrets;
pub mod store;
pub mod views;

#[cfg(test)]
mod web_tests;

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tokio::net::TcpListener;

pub struct Config {
    pub root: PathBuf,
    pub bind: IpAddr,
    pub port: u16,
    pub token: String,
    pub allow_remote: bool,
    pub open: bool,
    pub print_token: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub auth: Arc<auth::Auth>,
    pub jobs: jobs::Jobs,
    pub secrets: Arc<secrets::Secrets>,
    pub limit: Arc<limit::RateLimiter>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let auth = Arc::new(auth::Auth::new(config.token.clone()));
        Self {
            config: Arc::new(config),
            auth,
            jobs: jobs::Jobs::default(),
            secrets: Arc::new(secrets::Secrets::new()),
            limit: Arc::new(limit::RateLimiter::default()),
        }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.config.root
    }
}

pub fn app(state: AppState) -> Router {
    let secured = Router::new()
        .route("/", get(handlers::index))
        .route("/projects/new", get(handlers::new_project_page))
        .route("/projects/{id}", get(handlers::project_page))
        .route("/projects/{id}/draft", get(handlers::draft_page))
        .route("/projects/{id}/init", get(handlers::init_project_page))
        .route("/projects/{id}/run", get(handlers::new_run_page))
        .route("/jobs", get(handlers::jobs_page))
        .route("/jobs/{id}", get(handlers::job_page))
        .route("/runs/{id}", get(handlers::run_page))
        .route("/runs/{id}/account", get(handlers::account_page))
        .route("/runs/{id}/edit", get(handlers::edit_page))
        .route("/runs/{id}/validation", get(handlers::validation_page))
        .route("/runs/{id}/report", get(handlers::report_page))
        .route("/runs/{id}/files", get(handlers::files_page))
        .route("/runs/{id}/events", get(handlers::events_page))
        .route("/runs/{id}/optimize", get(handlers::optimize_page))
        .route(
            "/runs/{id}/optimize/review",
            get(handlers::optimize_review_page),
        )
        .route("/settings/connections", get(handlers::connections_page))
        .route("/settings/language", get(handlers::set_language))
        .route("/api/health", get(handlers::health))
        .route(
            "/api/session",
            post(handlers::session_create).delete(handlers::session_delete),
        )
        .route("/api/secrets", get(handlers::api_secrets))
        .route(
            "/api/secrets/{name}",
            axum::routing::put(handlers::set_secret).delete(handlers::delete_secret),
        )
        .route(
            "/api/projects",
            get(handlers::api_projects).post(handlers::create_project),
        )
        .route(
            "/api/projects/{id}/draft",
            get(handlers::api_draft).put(handlers::save_draft),
        )
        .route(
            "/api/projects/{id}/research",
            axum::routing::put(handlers::save_research),
        )
        .route(
            "/api/projects/{id}/design",
            axum::routing::put(handlers::save_design),
        )
        .route(
            "/api/projects/{id}/logo",
            get(handlers::project_logo).post(handlers::upload_logo),
        )
        .route(
            "/api/projects/{id}/init",
            post(handlers::start_init_project),
        )
        .route("/api/projects/{id}/runs", post(handlers::create_run))
        .route("/api/jobs/{id}", get(handlers::api_job))
        .route("/api/jobs/{id}/cancel", post(handlers::cancel_job))
        .route("/api/jobs/{id}/events", get(handlers::job_stream))
        .route("/api/runs/{id}", get(handlers::api_run))
        .route("/api/runs/{id}/account", get(handlers::api_account))
        .route("/api/runs/{id}/validation", get(handlers::api_validation))
        .route("/api/runs/{id}/report", get(handlers::api_report))
        .route("/api/runs/{id}/files", get(handlers::api_files))
        .route(
            "/api/runs/{id}/account/validate",
            post(handlers::api_validate_edits),
        )
        .route("/api/runs/{id}/resume", post(handlers::resume_run))
        .route("/api/runs/{id}/continue", post(handlers::continue_run))
        .route("/api/runs/{id}/export", post(handlers::export_run_now))
        .route("/api/runs/{id}/revisions", post(handlers::create_revision))
        .route("/api/runs/{id}/revert", post(handlers::revert_revision))
        .route(
            "/api/runs/{id}/optimize/prepare",
            post(handlers::optimize_prepare),
        )
        .route("/api/runs/{id}/events", get(handlers::api_events))
        .route("/api/runs/{id}/stream", get(handlers::stream))
        .route("/api/runs/{id}/files/{*path}", get(handlers::file))
        .route("/assets/{*path}", get(assets::file))
        .merge(performance::router())
        .merge(diff::router())
        .merge(project_admin::router())
        .fallback(handlers::not_found);
    secured
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    // Runs from a previous server life cannot still be running: mark their
    // missions interrupted so the UI offers resume instead of a fake "running".
    store::sweep_interrupted_runs(&config.root);
    let addr = SocketAddr::new(config.bind, config.port);
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot bind {addr}: {e}. Port {} is fixed by default, use --port for another one",
            config.port
        )
    })?;
    let actual = listener.local_addr().unwrap_or(addr);
    let url = public_url(&config, actual);
    if config.print_token {
        println!("mads studio: {url}");
    } else {
        println!("mads studio: {url}");
        println!("use the token in --token-file to authenticate");
    }
    if config.open && actual.ip().is_loopback() {
        open_browser(&url);
    }
    let state = AppState::new(config);
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

fn public_url(config: &Config, actual: SocketAddr) -> String {
    let host = if actual.ip().is_loopback() {
        "127.0.0.1".to_string()
    } else {
        actual.ip().to_string()
    };
    let base = format!("http://{host}:{}", actual.port());
    if config.print_token {
        format!("{base}/#{}", config.token)
    } else {
        base
    }
}

fn open_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };
    let mut command = std::process::Command::new(opener);
    if cfg!(target_os = "windows") {
        command.args(["/C", "start", ""]);
    }
    let _ = command.arg(url).spawn();
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn security(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !host_allowed(&state, request.headers()) {
        return (
            StatusCode::BAD_REQUEST,
            "host not allowed: bind mads serve to loopback or use --allow-remote",
        )
            .into_response();
    }
    if is_mutation(request.method()) && !origin_allowed(request.headers()) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    if is_mutation(request.method())
        && !state
            .limit
            .allow("studio", 120, std::time::Duration::from_secs(60))
    {
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            "too many requests: wait a minute",
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        return response;
    }
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        axum::http::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if !path.starts_with("/assets/") {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn host_allowed(state: &AppState, headers: &HeaderMap) -> bool {
    if state.config.allow_remote {
        return true;
    }
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return true;
    };
    let port = state.config.port;
    let allowed = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ];
    allowed.iter().any(|a| a.eq_ignore_ascii_case(host))
}

fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) else {
        return true;
    };
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let origin_host = rest.split('/').next().unwrap_or(rest);
    match headers.get(header::HOST).and_then(|h| h.to_str().ok()) {
        Some(host) => origin_host.eq_ignore_ascii_case(host),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(bind: IpAddr, allow_remote: bool) -> AppState {
        AppState::new(Config {
            root: PathBuf::from("."),
            bind,
            port: 4780,
            token: "a".repeat(64),
            allow_remote,
            open: false,
            print_token: true,
        })
    }

    #[test]
    fn loopback_hosts_are_allowed() {
        let state = state("127.0.0.1".parse().unwrap(), false);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4780"));
        assert!(host_allowed(&state, &headers));
        headers.insert(header::HOST, HeaderValue::from_static("localhost:4780"));
        assert!(host_allowed(&state, &headers));
    }

    #[test]
    fn another_host_is_rejected() {
        let state = state("127.0.0.1".parse().unwrap(), false);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("evil.example:4780"));
        assert!(!host_allowed(&state, &headers));
    }

    #[test]
    fn remote_mode_skips_the_host_allowlist() {
        let state = state("0.0.0.0".parse().unwrap(), true);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("ads.example:4780"));
        assert!(host_allowed(&state, &headers));
    }

    #[test]
    fn a_cross_origin_post_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4780"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!origin_allowed(&headers));
    }

    #[test]
    fn a_same_origin_post_is_allowed() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4780"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:4780"),
        );
        assert!(origin_allowed(&headers));
    }

    #[test]
    fn the_public_url_contains_the_token_in_the_fragment() {
        let config = Config {
            root: PathBuf::from("."),
            bind: "127.0.0.1".parse().unwrap(),
            port: 4780,
            token: "t".repeat(64),
            allow_remote: false,
            open: false,
            print_token: true,
        };
        let url = public_url(&config, "127.0.0.1:4780".parse().unwrap());
        assert_eq!(url, format!("http://127.0.0.1:4780/#{}", "t".repeat(64)));
        assert!(!url.contains("?"));
    }
}
