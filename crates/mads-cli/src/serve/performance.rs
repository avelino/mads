//! The performance digest of an optimization run: the reports read, the campaigns in them and
//! the flags that decide where the next budget goes.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use mads_core::{
    perf::{CampaignPerf, LiveStatus, Performance, ReportFile, ReportKind},
    workspace::Workspace,
};

use crate::serve::{
    AppState,
    auth::Identity,
    store::{self, RunDetail},
    views::{self, Chrome, Html},
};

/// Where a number is missing because the reports did not carry it.
const DASH: &str = "—";

#[derive(Template)]
#[template(path = "performance.html")]
pub struct PerformancePage {
    pub chrome: Chrome,
    pub title: String,
    pub run: RunDetail,
    pub view: PerformanceView,
}

/// The page route, merged by `serve::app`.
pub fn router() -> axum::Router<crate::serve::AppState> {
    axum::Router::new().route("/runs/{id}/performance", get(performance_page))
}

pub async fn performance_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found(&identity, "Execução não encontrada");
    };
    let workspace = Workspace::load(&run.dir.join("workspace.json")).ok();
    let performance = workspace
        .as_ref()
        .and_then(|ws| ws.live.as_ref())
        .map(|live| &live.performance);
    let currency = workspace
        .as_ref()
        .map(|ws| ws.input.budget.currency.as_str())
        .unwrap_or_default();
    let view = build_view(performance, currency);
    let chrome = Chrome::new(identity.csrf.clone(), "performance", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    Html(PerformancePage {
        chrome,
        title: format!("Performance {}", run.summary.id),
        run,
        view,
    })
    .into_response()
}

fn not_found(identity: &Identity, message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(views::ErrorPage {
            chrome: Chrome::new(identity.csrf.clone(), "performance", &identity.lang),
            title: "Não encontrado".into(),
            message: message.into(),
        }),
    )
        .into_response()
}

/// Everything the template reads, with the numbers already formatted and the optional report
/// fields resolved to text. Pure, so the tests need no filesystem or store.
#[derive(Debug, Clone, Default)]
pub struct PerformanceView {
    pub has_performance: bool,
    pub currency: String,
    pub has_window: bool,
    pub window_start: String,
    pub window_end: String,
    pub window_days: String,
    pub reports: Vec<ReportRow>,
    pub unknown_files: Vec<String>,
    pub ignored_campaigns: Vec<String>,
    pub missing_reports: Vec<String>,
    pub mixed_windows: bool,
    pub idle_budget: String,
    pub campaigns: Vec<CampaignRow>,
    pub flagged: usize,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReportRow {
    pub file: String,
    pub kind: String,
    pub rows: String,
    pub window: String,
}

#[derive(Debug, Clone)]
pub struct CampaignRow {
    pub name: String,
    pub live_status: String,
    pub status_class: String,
    pub has_status_reasons: bool,
    pub status_reasons: String,
    pub budget: String,
    pub impressions: String,
    pub clicks: String,
    pub cost: String,
    pub avg_cpc: String,
    pub ctr: String,
    pub conversions: String,
    pub conversion_value: String,
    pub cost_per_conversion: String,
    pub lost_to_budget: String,
    pub lost_to_rank: String,
    pub budget_use: String,
    pub days_running: String,
    pub avg_daily_cost: String,
    pub has_hidden_terms_cost: bool,
    pub hidden_terms_cost: String,
    pub flags: Vec<String>,
    pub ad_groups: Vec<String>,
}

fn build_view(performance: Option<&Performance>, currency: &str) -> PerformanceView {
    let Some(performance) = performance else {
        return PerformanceView {
            currency: currency.to_string(),
            ..PerformanceView::default()
        };
    };
    let window = performance.window.as_ref();
    let campaigns: Vec<CampaignRow> = performance.campaigns.iter().map(campaign_row).collect();
    let flagged = campaigns.iter().filter(|row| !row.flags.is_empty()).count();
    PerformanceView {
        has_performance: true,
        currency: currency.to_string(),
        has_window: window.is_some(),
        window_start: window.map(|w| w.start.clone()).unwrap_or_default(),
        window_end: window.map(|w| w.end.clone()).unwrap_or_default(),
        window_days: window
            .map(|w| w.days.to_string())
            .unwrap_or_else(|| DASH.into()),
        reports: performance.reports.iter().map(report_row).collect(),
        unknown_files: performance.unknown_files.clone(),
        ignored_campaigns: performance.ignored_campaigns.clone(),
        missing_reports: performance
            .missing_reports
            .iter()
            .map(|kind| report_kind(*kind).to_string())
            .collect(),
        mixed_windows: performance.mixed_windows,
        idle_budget: performance
            .idle_budget
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| DASH.into()),
        campaigns,
        flagged,
        findings: performance.findings(),
    }
}

fn report_row(report: &ReportFile) -> ReportRow {
    ReportRow {
        file: report.file.clone(),
        kind: report_kind(report.kind).to_string(),
        rows: report.rows.to_string(),
        window: report
            .window
            .as_ref()
            .map(|w| format!("{} → {}", w.start, w.end))
            .unwrap_or_else(|| DASH.into()),
    }
}

fn campaign_row(campaign: &CampaignPerf) -> CampaignRow {
    let mut flags = Vec::new();
    if campaign.thin {
        flags.push("thin".to_string());
    }
    if campaign.stalled {
        flags.push("stalled".to_string());
    }
    if campaign.untracked {
        flags.push("untracked".to_string());
    }
    if campaign.budget_use_pct.is_some_and(|pct| pct >= 100.0) {
        flags.push("no limite".to_string());
    }
    if campaign.hidden_terms_cost.is_some_and(|cost| cost > 0.0) {
        flags.push("termos ocultos".to_string());
    }
    let (live_status, status_class) = live_status(campaign.live_status);
    CampaignRow {
        name: campaign.name.clone(),
        live_status,
        status_class,
        has_status_reasons: !campaign.status_reasons.trim().is_empty(),
        status_reasons: campaign.status_reasons.clone(),
        budget: money_opt(campaign.budget),
        impressions: campaign.metrics.impressions.to_string(),
        clicks: campaign.metrics.clicks.to_string(),
        cost: money(campaign.metrics.cost),
        avg_cpc: money_opt(campaign.metrics.avg_cpc),
        ctr: percent(campaign.metrics.ctr_pct),
        conversions: format!("{:.2}", campaign.metrics.conversions),
        conversion_value: money(campaign.metrics.conversion_value),
        cost_per_conversion: money_opt(campaign.metrics.cost_per_conversion),
        lost_to_budget: percent(campaign.lost_to_budget_pct),
        lost_to_rank: percent(campaign.lost_to_rank_pct),
        budget_use: campaign
            .budget_use_pct
            .map(|pct| format!("{pct:.0}%"))
            .unwrap_or_else(|| DASH.into()),
        days_running: campaign
            .days_running
            .map(|days| days.to_string())
            .unwrap_or_else(|| DASH.into()),
        avg_daily_cost: money_opt(campaign.avg_daily_cost),
        has_hidden_terms_cost: campaign.hidden_terms_cost.is_some(),
        hidden_terms_cost: money_opt(campaign.hidden_terms_cost),
        flags,
        ad_groups: campaign.ad_groups.iter().map(|g| g.name.clone()).collect(),
    }
}

fn live_status(status: Option<LiveStatus>) -> (String, String) {
    let (label, class) = match status {
        Some(LiveStatus::Enabled) => ("Ativa", "ok"),
        Some(LiveStatus::Paused) => ("Pausada", "warn"),
        Some(LiveStatus::Removed) => ("Removida", "muted"),
        None => (DASH, "muted"),
    };
    (label.into(), class.into())
}

fn report_kind(kind: ReportKind) -> &'static str {
    match kind {
        ReportKind::SearchTerms => "Termos de busca",
        ReportKind::Keywords => "Palavras-chave",
        ReportKind::Campaigns => "Campanhas",
        ReportKind::CampaignsByDay => "Campanhas por dia",
        ReportKind::Assets => "Anúncios",
    }
}

fn money(value: f64) -> String {
    format!("{value:.2}")
}

fn money_opt(value: Option<f64>) -> String {
    value.map(money).unwrap_or_else(|| DASH.into())
}

fn percent(value: Option<f64>) -> String {
    value
        .map(|pct| format!("{pct:.2}%"))
        .unwrap_or_else(|| DASH.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_performance_is_an_empty_digest() {
        let performance = Performance::default();
        let view = build_view(Some(&performance), "BRL");
        assert!(view.has_performance);
        assert_eq!(view.currency, "BRL");
        assert!(!view.has_window);
        assert_eq!(view.window_start, "");
        assert_eq!(view.window_days, DASH);
        assert_eq!(view.idle_budget, DASH);
        assert!(!view.mixed_windows);
        assert!(view.reports.is_empty());
        assert!(view.unknown_files.is_empty());
        assert!(view.ignored_campaigns.is_empty());
        assert!(view.missing_reports.is_empty());
        assert!(view.campaigns.is_empty());
        assert_eq!(view.flagged, 0);
        assert!(view.findings.is_empty());
    }

    #[test]
    fn a_run_without_live_performance_is_not_an_optimization() {
        let view = build_view(None, "");
        assert!(!view.has_performance);
        assert!(view.campaigns.is_empty());
        assert_eq!(view.flagged, 0);
        assert!(view.findings.is_empty());
    }
}
