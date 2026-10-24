use askama::Template;
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use mads_providers::{ProviderKind, ProviderStatus, list_image_providers, list_providers};

use crate::serve::{
    jobs::JobView,
    secrets::KeyStatus,
    store::{Project, RunDetail},
};

#[derive(Debug, Clone, Default)]
pub struct Chrome {
    pub lang: String,
    pub csrf: String,
    pub active: String,
    pub project_name: String,
    pub run_id: String,
}

impl Chrome {
    pub fn new(csrf: String, active: &str, lang: &str) -> Self {
        Self {
            lang: lang.into(),
            csrf,
            active: active.into(),
            project_name: String::new(),
            run_id: String::new(),
        }
    }

    pub fn t(&self, key: &str) -> String {
        crate::serve::i18n::t(&self.lang, key)
    }

    pub fn with_project(mut self, name: &str) -> Self {
        self.project_name = name.into();
        self
    }

    pub fn with_run(mut self, run_id: &str) -> Self {
        self.run_id = run_id.into();
        self
    }
}

pub struct Html<T: Template>(pub T);

impl<T: Template> IntoResponse for Html<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(body) => {
                let mut response = body.into_response();
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("text/html; charset=utf-8"),
                );
                response
            }
            Err(error) => {
                let mut response = (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("template error: {error}"),
                )
                    .into_response();
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("text/plain; charset=utf-8"),
                );
                response
            }
        }
    }
}

#[derive(Template)]
#[template(path = "bootstrap.html")]
pub struct BootstrapPage {
    pub title: String,
    pub lang: String,
}

#[derive(Template)]
#[template(path = "new_project.html")]
pub struct NewProjectPage {
    pub chrome: Chrome,
    pub title: String,
    pub providers: Vec<ProviderRow>,
    pub default_provider: String,
    pub default_model: String,
}

#[derive(Template)]
#[template(path = "projects.html")]
pub struct ProjectsPage {
    pub chrome: Chrome,
    pub title: String,
    pub projects: Vec<Project>,
}

#[derive(Template)]
#[template(path = "jobs.html")]
pub struct JobsPage {
    pub chrome: Chrome,
    pub title: String,
    pub jobs: Vec<crate::serve::jobs::JobView>,
}

#[derive(Template)]
#[template(path = "project.html")]
pub struct ProjectPage {
    pub chrome: Chrome,
    pub title: String,
    pub project: Project,
}

#[derive(Template)]
#[template(path = "init_project.html")]
pub struct InitProjectPage {
    pub chrome: Chrome,
    pub title: String,
    pub project: Project,
    pub meta: crate::serve::jobs::InitMeta,
    pub model_text: String,
    pub base_url_text: String,
}

#[derive(Template)]
#[template(path = "draft.html")]
pub struct DraftPage {
    pub chrome: Chrome,
    pub title: String,
    pub project: Project,
    pub draft: crate::serve::draft::DraftView,
}

#[derive(Template)]
#[template(path = "new_run.html")]
pub struct NewRunPage {
    pub chrome: Chrome,
    pub title: String,
    pub project: Project,
    pub providers: Vec<ProviderRow>,
    pub default_provider: String,
    pub default_model: String,
}

#[derive(Template)]
#[template(path = "edit.html")]
pub struct EditPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "optimize.html")]
pub struct OptimizePage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "optimize_review.html")]
pub struct OptimizeReviewPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
    pub performance_json: String,
    pub default_provider: String,
    pub default_model: String,
}

#[derive(Template)]
#[template(path = "job.html")]
pub struct JobPage {
    pub chrome: Chrome,
    pub title: String,
    pub job: JobView,
}

#[derive(Template)]
#[template(path = "run.html")]
pub struct RunPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "validation.html")]
pub struct ValidationPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "report.html")]
pub struct ReportPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "files.html")]
pub struct FilesPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Template)]
#[template(path = "events.html")]
pub struct EventsPage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
}

#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub ready: bool,
}

#[derive(Debug, Clone)]
pub struct ImageProviderRow {
    pub name: String,
    pub status: String,
    pub ready: bool,
}

#[derive(Template)]
#[template(path = "connections.html")]
pub struct ConnectionsPage {
    pub chrome: Chrome,
    pub title: String,
    pub providers: Vec<ProviderRow>,
    pub image_providers: Vec<ImageProviderRow>,
    pub key_file: String,
    pub keys: Vec<KeyStatus>,
    pub secrets_error: String,
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorPage {
    pub chrome: Chrome,
    pub title: String,
    pub message: String,
}

pub fn providers_with(
    secrets: &crate::serve::secrets::Secrets,
) -> (Vec<ProviderRow>, Vec<ImageProviderRow>) {
    let (mut providers, mut images) = providers();
    let keys = secrets.statuses();
    for provider in &mut providers {
        if let Some(status) = keys.iter().find(|status| status.provider == provider.name)
            && status.ready
            && provider.status.starts_with("missing ")
        {
            provider.status = status.source_label.clone();
            provider.ready = true;
        }
    }
    for provider in &mut images {
        if let Some(status) = keys.iter().find(|status| status.provider == provider.name)
            && status.ready
            && provider.status.starts_with("missing ")
        {
            provider.status = status.source_label.clone();
            provider.ready = true;
        }
    }
    (providers, images)
}

pub fn providers() -> (Vec<ProviderRow>, Vec<ImageProviderRow>) {
    let providers = list_providers()
        .into_iter()
        .map(|p| ProviderRow {
            name: p.name.to_string(),
            kind: match p.kind {
                ProviderKind::Api => "api".into(),
                ProviderKind::Cli => "cli".into(),
            },
            status: describe(&p.status),
            ready: p.status == ProviderStatus::Ready,
        })
        .collect();
    let images = list_image_providers()
        .into_iter()
        .map(|(name, status)| ImageProviderRow {
            name: name.to_string(),
            status: describe(&status),
            ready: status == ProviderStatus::Ready,
        })
        .collect();
    (providers, images)
}

fn describe(status: &ProviderStatus) -> String {
    status.describe()
}
