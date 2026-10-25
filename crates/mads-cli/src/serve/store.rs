use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use mads_core::{
    events::{Event, EventEnvelope},
    google::{Account, BrandKit, Issue, MatchType, Rules, merge_rsa},
    input::{Input, parse_input_toml},
    workspace::{MissionStatus, Workspace},
};
use pulldown_cmark::{Options, Parser, html};
use serde::Deserialize;
use serde_json::Value;
use time::{OffsetDateTime, format_description};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RunSummary {
    pub id: String,
    pub status: String,
    pub label: String,
    pub class: String,
    pub provider: String,
    pub model: String,
    pub origin: String,
    pub revision: u32,
    pub parent: String,
    pub has_parent: bool,
    pub when: String,
    pub tokens: String,
    pub cost: String,
    pub missions: usize,
    pub failed: usize,
    pub layout: String,
}

#[derive(Debug, Clone, Default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub url: String,
    pub daily: String,
    pub currency: String,
    pub archived: bool,
    pub error_text: String,
    pub has_error: bool,
    pub updated: String,
    pub has_runs: bool,
    pub runs: Vec<RunSummary>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IssueRow {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct IssueGroup {
    pub code: String,
    pub count: usize,
    pub items: Vec<IssueRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UrlRow {
    pub url: String,
    pub status: String,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct EventRow {
    pub seq: u64,
    pub time: String,
    pub mission: String,
    pub kind: String,
    pub text: String,
    pub class: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRow {
    pub path: String,
    pub size: String,
    pub kind: String,
    pub image: bool,
    pub url: String,
    pub preview: String,
    pub has_preview: bool,
}

#[derive(Debug, Clone)]
pub struct FileData {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub name: String,
    pub image: bool,
}

#[derive(Debug, Clone)]
pub struct TextRow {
    pub text: String,
    pub len: usize,
    pub limit: usize,
    pub over: bool,
}

#[derive(Debug, Clone)]
pub struct KeywordRow {
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct ImageRow {
    pub id: String,
    pub ratio: String,
    pub prompt: String,
    pub status: String,
    pub preview_url: String,
    pub has_preview: bool,
}

#[derive(Debug, Clone)]
pub struct AdGroupRow {
    pub name: String,
    pub cpc: String,
    pub final_url: String,
    pub keywords: Vec<KeywordRow>,
    pub negatives: Vec<String>,
    pub headlines: Vec<TextRow>,
    pub descriptions: Vec<TextRow>,
    pub path1: String,
    pub path2: String,
}

#[derive(Debug, Clone)]
pub struct AssetGroupRow {
    pub name: String,
    pub final_url: String,
    pub business_name: String,
    pub headlines: Vec<TextRow>,
    pub long_headlines: Vec<TextRow>,
    pub descriptions: Vec<TextRow>,
    pub search_themes: Vec<String>,
    pub images: Vec<ImageRow>,
}

#[derive(Debug, Clone)]
pub struct SitelinkRow {
    pub text: String,
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct SnippetRow {
    pub header: String,
    pub values: String,
}

#[derive(Debug, Clone)]
pub struct CampaignRow {
    pub name: String,
    pub kind: String,
    pub intent: String,
    pub budget: String,
    pub bid: String,
    pub rationale: String,
    pub ad_groups: Vec<AdGroupRow>,
    pub asset_groups: Vec<AssetGroupRow>,
    pub negatives: Vec<String>,
    pub keyword_count: usize,
    pub sitelinks: Vec<SitelinkRow>,
    pub callouts: Vec<String>,
    pub snippets: Vec<SnippetRow>,
    pub has_assets: bool,
}

#[derive(Debug, Clone)]
pub struct EditField {
    pub path: String,
    pub label: String,
    pub value: String,
    pub kind: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct AccountView {
    pub fields: Vec<EditField>,
    pub campaigns: Vec<CampaignRow>,
    pub brand_headlines: usize,
    pub brand_descriptions: usize,
    pub campaign_count: usize,
    pub ad_group_count: usize,
    pub keyword_count: usize,
    pub budget_total: String,
}

#[derive(Debug, Clone)]
pub struct RunDetail {
    pub summary: RunSummary,
    pub project_name: String,
    pub dir: PathBuf,
    pub report_html: String,
    pub has_report: bool,
    pub errors: Vec<IssueRow>,
    pub warnings: Vec<IssueRow>,
    pub error_groups: Vec<IssueGroup>,
    pub warning_groups: Vec<IssueGroup>,
    pub urls: Vec<UrlRow>,
    pub events: Vec<EventRow>,
    pub last_seq: u64,
    pub files: Vec<FileRow>,
    pub account: AccountView,
    pub has_account: bool,
    /// Hash of the account for optimistic concurrency (`If-Match` on edits).
    pub account_hash: String,
}

#[derive(Debug, Deserialize, Default)]
struct ProjectMeta {
    name: Option<String>,
    #[serde(default)]
    archived: bool,
}

/// Every project under the root. Hidden directories and symlinks are ignored.
/// A directory the studio treats as a project: no symlink, with the
/// `business.toml` or `project.json` marker.
fn is_project_dir(path: &Path) -> bool {
    path.join("business.toml").is_file() || path.join("project.json").is_file()
}

pub fn projects(root: &Path) -> Vec<Project> {
    let mut projects = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return projects;
    };
    for entry in entries.flatten() {
        // Symlinked directories are ignored: a link must not pull a path
        // outside the root into the project list.
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !is_project_dir(&path) {
            continue;
        }
        let Some(project) = load_project(&path, name) else {
            continue;
        };
        if !project.archived {
            projects.push(project);
        }
    }
    projects.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.name.cmp(&b.name)));
    projects
}

/// A single project by directory id.
pub fn project(root: &Path, id: &str) -> Option<Project> {
    let dir = project_dir(root, id)?;
    load_project(&dir, id)
}

/// Finds a run by id across projects. Returns the project id and name with the run.
pub fn find_run(root: &Path, run_id: &str) -> Option<(String, String, RunDetail)> {
    if !valid_run_id(run_id) {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let Some(project_id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if project_id.starts_with('.') || !is_project_dir(&path) {
            continue;
        }
        let run_dir = path.join("out").join(run_id);
        if !run_dir.is_dir() {
            continue;
        }
        let project = load_project(&path, project_id)?;
        let summary = run_summary(&run_dir, run_id);
        let detail = run_detail_from(&project, &run_dir, summary);
        return Some((project.id, project.name, detail));
    }
    None
}

fn project_dir(root: &Path, id: &str) -> Option<PathBuf> {
    if !valid_project_id(id) {
        return None;
    }
    let dir = root.join(id);
    let canonical = dir.canonicalize().ok()?;
    let root_canonical = root.canonicalize().ok()?;
    (canonical.parent()? == root_canonical).then_some(canonical)
}

fn load_project(dir: &Path, id: &str) -> Option<Project> {
    let meta: ProjectMeta = fs::read_to_string(dir.join("project.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let business = fs::read_to_string(dir.join("business.toml")).ok();
    let (name, url, daily, currency, error_text) = match business.as_deref() {
        Some(text) => match parse_input_toml(text) {
            Ok(file) => (
                meta.name.clone().unwrap_or(file.business.name),
                file.business.url,
                file.budget.daily.format_budget(file.export.decimal_comma),
                file.budget.currency,
                String::new(),
            ),
            Err(e) => (
                meta.name.clone().unwrap_or_else(|| pretty_slug(id)),
                String::new(),
                String::new(),
                String::new(),
                e.to_string(),
            ),
        },
        None => (
            meta.name.clone().unwrap_or_else(|| pretty_slug(id)),
            String::new(),
            String::new(),
            String::new(),
            "business.toml is missing".into(),
        ),
    };
    let mut runs = list_runs(dir);
    runs.sort_by(|a, b| b.id.cmp(&a.id));
    let updated = runs
        .first()
        .map(|r| r.when.clone())
        .unwrap_or_else(|| modified(dir));
    Some(Project {
        id: id.to_string(),
        name,
        url,
        daily,
        currency,
        archived: meta.archived,
        has_error: !error_text.is_empty(),
        error_text,
        updated,
        has_runs: !runs.is_empty(),
        runs,
    })
}

fn list_runs(project_dir: &Path) -> Vec<RunSummary> {
    let out = project_dir.join("out");
    let Ok(entries) = fs::read_dir(out) else {
        return Vec::new();
    };
    let mut runs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if id.starts_with('.') || !valid_run_id(id) {
            continue;
        }
        if !path.join("workspace.json").is_file() && !path.join("run.json").is_file() {
            continue;
        }
        runs.push(run_summary(&path, id));
    }
    runs
}

fn run_summary(dir: &Path, id: &str) -> RunSummary {
    let run_json: Option<Value> = fs::read_to_string(dir.join("run.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let mut summary = RunSummary {
        id: id.to_string(),
        provider: "-".into(),
        model: "-".into(),
        origin: "agent".into(),
        when: when_from_id(id),
        ..RunSummary::default()
    };
    let workspace = Workspace::load(&dir.join("workspace.json")).ok();
    if let Some(ws) = workspace.as_ref() {
        if ws.live.is_some() {
            summary.origin = "optimize".into();
        }
        let all: Vec<&mads_core::workspace::MissionState> = ws.missions.values().collect();
        summary.missions = all.len();
        summary.failed = all
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Failed { .. }))
            .count();
        summary.tokens = tokens(
            all.iter()
                .map(|m| m.usage.input_tokens + m.usage.output_tokens),
        );
        summary.cost = cost_of(
            all.iter().filter_map(|m| m.usage.cost_usd).sum::<f64>(),
            all.iter().any(|m| m.usage.cost_usd.is_some()),
        );
    }
    if let Some(value) = run_json {
        summary.status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("incomplete")
            .to_string();
        summary.provider = value
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string();
        summary.model = value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string();
        summary.layout = value
            .get("layout")
            .and_then(Value::as_str)
            .unwrap_or("bulk")
            .to_string();
        summary.origin = value
            .get("origin")
            .and_then(Value::as_str)
            .unwrap_or(&summary.origin)
            .to_string();
        summary.parent = value
            .get("parent_run")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        summary.has_parent = !summary.parent.is_empty();
        summary.revision = value.get("revision").and_then(Value::as_u64).unwrap_or(0) as u32;
        if let Some(totals) = value.get("totals") {
            summary.missions = totals
                .get("missions")
                .and_then(Value::as_u64)
                .unwrap_or(summary.missions as u64) as usize;
            summary.failed = totals
                .get("failed_missions")
                .and_then(Value::as_u64)
                .unwrap_or(summary.failed as u64) as usize;
            let input = totals
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let output = totals
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            summary.tokens = tokens([input + output]);
            summary.cost = match totals.get("cost_usd").and_then(Value::as_f64) {
                Some(value) => cost_of(value, true),
                None => "n/a".into(),
            };
        }
    } else if let Some(ws) = workspace.as_ref() {
        summary.status = derived_status(ws);
    } else {
        summary.status = "unknown".into();
    }
    if summary.status == "unknown" {
        summary.status = "incomplete".into();
    }
    let (label, class) = status_label(&summary.status);
    summary.label = label;
    summary.class = class;
    summary
}

fn derived_status(ws: &Workspace) -> String {
    let plan_done = ws
        .missions
        .get("plan")
        .is_some_and(|m| m.status == MissionStatus::Finished);
    let all: Vec<&mads_core::workspace::MissionState> = ws.missions.values().collect();
    let any_running = all.iter().any(|m| m.status == MissionStatus::Running);
    let any_failed = all
        .iter()
        .any(|m| matches!(m.status, MissionStatus::Failed { .. }));
    let campaigns: Vec<&mads_core::workspace::MissionState> = ws
        .missions
        .iter()
        .filter(|(id, _)| id.starts_with("campaign:"))
        .map(|(_, m)| m)
        .collect();
    let campaigns_done = !campaigns.is_empty()
        && campaigns
            .iter()
            .all(|m| m.status == MissionStatus::Finished);
    if ws.live.is_some() && ws.missions.is_empty() {
        return "prepared".into();
    }
    if any_running {
        return "running".into();
    }
    if plan_done && !ws.account.campaigns.is_empty() && !campaigns_done && !any_failed {
        return "planned".into();
    }
    if any_failed {
        return "interrupted".into();
    }
    "incomplete".into()
}

fn run_detail_from(project: &Project, run_dir: &Path, summary: RunSummary) -> RunDetail {
    let workspace = Workspace::load(&run_dir.join("workspace.json")).ok();
    let events = read_events(&run_dir.join("events.ndjson"));
    let report_raw = fs::read_to_string(run_dir.join("report.md")).ok();
    let has_report = report_raw.is_some();
    let report_html = report_raw
        .as_deref()
        .map(render_markdown)
        .unwrap_or_default();
    let (errors, warnings) = match last_validation(&events) {
        Some(pair) => pair,
        None => match workspace.as_ref() {
            Some(ws) => {
                let issues = Rules::new(&ws.input, 50).account(&ws.account, true);
                let (errors, warnings): (Vec<Issue>, Vec<Issue>) =
                    issues.into_iter().partition(Issue::is_error);
                (errors, warnings)
            }
            None => (Vec::new(), Vec::new()),
        },
    };
    let urls = last_urls(&events);
    let (account, has_account, account_hash) = match workspace.as_ref() {
        Some(ws) => (
            account_view(&ws.account, &ws.input, &summary.id),
            true,
            account_hash(&ws.account),
        ),
        None => (AccountView::default(), false, String::new()),
    };
    let last_seq = events.last().map_or(0, |e| e.seq);
    let run_id = summary.id.clone();
    RunDetail {
        summary,
        project_name: project.name.clone(),
        dir: run_dir.to_path_buf(),
        report_html,
        has_report,
        errors: errors.iter().map(issue_row).collect(),
        warnings: warnings.iter().map(issue_row).collect(),
        error_groups: group_issues(&errors),
        warning_groups: group_issues(&warnings),
        urls,
        events: events.iter().map(event_row).collect(),
        last_seq,
        files: list_files(run_dir, &run_id),
        account,
        has_account,
        account_hash,
    }
}

/// A preview needs the first lines, never the whole file in memory.
const PREVIEW_BYTES: u64 = 64 * 1024;

/// Deterministic hash of the serialized account. Clients send it back in
/// `If-Match`; a mismatch answers 409 instead of composing stale edits.
pub fn account_hash(account: &Account) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match serde_json::to_value(account) {
        Ok(value) => value.hash(&mut hasher),
        Err(_) => return String::new(),
    }
    format!("{:016x}", hasher.finish())
}

fn read_events(path: &Path) -> Vec<EventEnvelope> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<EventEnvelope>(line).ok())
        .collect()
}

fn last_validation(events: &[EventEnvelope]) -> Option<(Vec<Issue>, Vec<Issue>)> {
    events.iter().rev().find_map(|event| match &event.event {
        Event::Validation { errors, warnings } => Some((errors.clone(), warnings.clone())),
        _ => None,
    })
}

fn last_urls(events: &[EventEnvelope]) -> Vec<UrlRow> {
    let mut rows: BTreeMap<String, (Option<u16>, bool)> = BTreeMap::new();
    for event in events {
        if let Event::UrlChecked { url, status, ok } = &event.event {
            rows.insert(url.clone(), (*status, *ok));
        }
    }
    rows.into_iter()
        .map(|(url, (status, ok))| UrlRow {
            url,
            status: status.map_or("sem resposta".into(), |s| s.to_string()),
            ok,
        })
        .collect()
}

fn issue_row(issue: &Issue) -> IssueRow {
    IssueRow {
        path: issue.path.clone(),
        message: issue.message.clone(),
    }
}

fn group_issues(issues: &[Issue]) -> Vec<IssueGroup> {
    let mut groups: BTreeMap<String, Vec<IssueRow>> = BTreeMap::new();
    for issue in issues {
        groups
            .entry(issue.code.clone())
            .or_default()
            .push(issue_row(issue));
    }
    groups
        .into_iter()
        .map(|(code, items)| IssueGroup {
            code,
            count: items.len(),
            items,
        })
        .collect()
}

fn event_row(envelope: &EventEnvelope) -> EventRow {
    let (mission, kind, text, class) = match &envelope.event {
        Event::RunStarted {
            run_id,
            provider,
            model,
            ..
        } => (
            "run".to_string(),
            "run_started".to_string(),
            format!(
                "início da execução {run_id} ({provider}, {})",
                model.as_deref().unwrap_or("modelo padrão")
            ),
            "muted",
        ),
        Event::MissionStarted { mission, attempt } => (
            mission.clone(),
            "mission_started".to_string(),
            format!("início da missão (tentativa {attempt})"),
            "running",
        ),
        Event::AgentText { mission, text } => (
            mission.clone(),
            "agent_text".to_string(),
            truncate(text, 240),
            "muted",
        ),
        Event::ToolCalled {
            mission,
            tool,
            summary,
        } => (
            mission.clone(),
            "tool_called".to_string(),
            format!("> {tool} {summary}"),
            "muted",
        ),
        Event::ToolFinished {
            mission,
            tool,
            ok,
            summary,
        } => (
            mission.clone(),
            "tool_finished".to_string(),
            format!("< {} {tool} {summary}", if *ok { "ok" } else { "erro" }),
            if *ok { "ok" } else { "error" },
        ),
        Event::Usage {
            mission,
            input_tokens,
            output_tokens,
            cost_usd,
        } => (
            mission.clone(),
            "usage".to_string(),
            format!(
                "{input_tokens} in, {output_tokens} out, custo {}",
                cost_usd.map_or("n/a".into(), |c| format!("${c:.4}"))
            ),
            "muted",
        ),
        Event::MissionFinished {
            mission,
            ok,
            reason,
        } => (
            mission.clone(),
            "mission_finished".to_string(),
            match reason {
                Some(reason) => format!("falhou: {reason}"),
                None => "concluída".into(),
            },
            if *ok { "ok" } else { "error" },
        ),
        Event::Step { name, detail } => (
            "run".into(),
            "step".to_string(),
            format!("{name}: {detail}"),
            "muted",
        ),
        Event::Validation { errors, warnings } => (
            "run".into(),
            "validation".to_string(),
            format!("{} erros, {} avisos", errors.len(), warnings.len()),
            if errors.is_empty() { "ok" } else { "error" },
        ),
        Event::UrlChecked { url, status, ok } => (
            "run".into(),
            "url_checked".to_string(),
            format!(
                "{url} {}",
                status.map_or("sem resposta".into(), |s| s.to_string())
            ),
            if *ok { "ok" } else { "error" },
        ),
        Event::ArtifactWritten { path } => (
            "run".into(),
            "artifact_written".to_string(),
            path.clone(),
            "ok",
        ),
        Event::RunPaused { totals } => (
            "run".into(),
            "run_paused".to_string(),
            format!(
                "plano pronto, campanhas pendentes: {} in, {} out",
                totals.input_tokens, totals.output_tokens
            ),
            "warn",
        ),
        Event::RunFinished {
            ok,
            exit_code,
            totals,
        } => (
            "run".into(),
            "run_finished".to_string(),
            format!(
                "fim (exit {exit_code}): {} in, {} out, {} missões",
                totals.input_tokens, totals.output_tokens, totals.missions
            ),
            if *ok { "ok" } else { "error" },
        ),
    };
    EventRow {
        seq: envelope.seq,
        time: short_time(&envelope.ts),
        mission,
        kind,
        text,
        class: class.into(),
    }
}

fn list_files(run_dir: &Path, run_id: &str) -> Vec<FileRow> {
    let mut files = Vec::new();
    for root in ["google-ads", "transcripts", "input"] {
        walk(&run_dir.join(root), run_dir, run_id, &mut files);
    }
    let report = run_dir.join("report.md");
    if let Ok(meta) = fs::metadata(&report)
        && meta.is_file()
    {
        files.push(file_row(
            "report.md".to_string(),
            &report,
            meta.len(),
            run_id,
        ));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn walk(dir: &Path, run_dir: &Path, run_id: &str, files: &mut Vec<FileRow>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            walk(&path, run_dir, run_id, files);
        } else if meta.is_file() {
            let Ok(rel) = path.strip_prefix(run_dir) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            files.push(file_row(rel.clone(), &path, meta.len(), run_id));
        }
    }
}

fn file_row(path: String, full: &Path, size: u64, run_id: &str) -> FileRow {
    let ext = full
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_uppercase();
    let image = matches!(ext.as_str(), "PNG" | "JPG" | "JPEG" | "WEBP" | "GIF");
    let text_like = matches!(
        ext.as_str(),
        "CSV" | "MD" | "TXT" | "JSON" | "NDJSON" | "JSONL" | "TOML"
    );
    let preview = if text_like {
        // A preview needs the first lines, not a multi-megabyte CSV in memory.
        fs::File::open(full)
            .ok()
            .and_then(|file| {
                use std::io::Read;
                let mut bytes = Vec::new();
                file.take(PREVIEW_BYTES).read_to_end(&mut bytes).ok()?;
                Some(bytes)
            })
            .and_then(|bytes| decode_text(&bytes))
            .map(|text| text.lines().take(12).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    FileRow {
        url: format!("/api/runs/{run_id}/files/{}", encode_path(&path)),
        path,
        size: bytes_label(size),
        kind: if ext.is_empty() { "FILE".into() } else { ext },
        image,
        has_preview: !preview.is_empty(),
        preview,
    }
}

/// The performance digest of an optimized run, pretty printed for the review page.
pub fn performance_json(run_dir: &Path) -> Option<String> {
    let ws = Workspace::load(&run_dir.join("workspace.json")).ok()?;
    let live = ws.live?;
    serde_json::to_string_pretty(&live.performance).ok()
}

/// Reads one run file after the path sandbox. Only known folders are served.
/// Every event of a run, in order. Corrupt lines are skipped.
pub fn read_run_events(run_dir: &Path) -> Vec<EventEnvelope> {
    read_events(&run_dir.join("events.ndjson"))
}

pub fn read_file(run_dir: &Path, rel: &str) -> Option<FileData> {
    let allowed = rel == "report.md"
        || rel.starts_with("google-ads/")
        || rel.starts_with("transcripts/")
        || rel.starts_with("input/");
    if !allowed {
        return None;
    }
    let path = safe_join(run_dir, rel)?;
    let meta = fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > 20 * 1024 * 1024 {
        return None;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bytes = fs::read(&path).ok()?;
    let image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");
    let content_type = if image {
        image_mime(&ext).to_string()
    } else if ext == "md" {
        "text/markdown; charset=utf-8".to_string()
    } else {
        "text/plain; charset=utf-8".to_string()
    };
    Some(FileData {
        bytes,
        content_type,
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string(),
        image,
    })
}

fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains('\0') {
        return None;
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return None;
    }
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let full = base.join(path);
    let canonical = full.canonicalize().ok()?;
    let base_canonical = base.canonicalize().ok()?;
    canonical.starts_with(base_canonical).then_some(canonical)
}

fn account_view(account: &Account, input: &Input, run_id: &str) -> AccountView {
    let comma = input.export.decimal_comma;
    let kit = account.brand_kit.clone();
    let empty_kit = BrandKit {
        headlines: Vec::new(),
        descriptions: Vec::new(),
    };
    let merge_kit = kit.as_ref().unwrap_or(&empty_kit);
    let mut fields = Vec::new();
    if let Some(kit) = &kit {
        for (i, text) in kit.headlines.iter().enumerate() {
            fields.push(edit_field(
                format!("brand_kit.headlines[{i}]"),
                format!("Brand kit · título {}", i + 1),
                text,
                "text",
                30,
            ));
        }
        for (i, text) in kit.descriptions.iter().enumerate() {
            fields.push(edit_field(
                format!("brand_kit.descriptions[{i}]"),
                format!("Brand kit · descrição {}", i + 1),
                text,
                "text",
                90,
            ));
        }
    }
    let mut campaigns = Vec::new();
    for (ci, campaign) in account.campaigns.iter().enumerate() {
        let mut keywords = 0;
        fields.push(edit_field(
            format!("campaigns[{ci}].rationale"),
            format!("{} · por que", campaign.name),
            &campaign.rationale,
            "text",
            0,
        ));
        fields.push(edit_field(
            format!("campaigns[{ci}].daily_budget"),
            format!("{} · orçamento diário", campaign.name),
            &format!("{:.2}", campaign.daily_budget.0 as f64 / 100.0),
            "number",
            0,
        ));
        for (ni, negative) in campaign.negatives.iter().enumerate() {
            fields.push(edit_field(
                format!("campaigns[{ci}].negatives[{ni}].text"),
                format!("{} · negativa {}", campaign.name, ni + 1),
                &negative.text,
                "text",
                80,
            ));
        }
        let mut ad_groups = Vec::new();
        for (gi, group) in campaign.ad_groups.iter().enumerate() {
            keywords += group.keywords.len();
            fields.push(edit_field(
                format!("campaigns[{ci}].ad_groups[{gi}].default_cpc"),
                format!("{} / {} · CPC", campaign.name, group.name),
                &format!("{:.2}", group.default_cpc.0 as f64 / 100.0),
                "number",
                0,
            ));
            fields.push(edit_field(
                format!("campaigns[{ci}].ad_groups[{gi}].final_url"),
                format!("{} / {} · URL final", campaign.name, group.name),
                &group.final_url,
                "url",
                0,
            ));
            for (ki, keyword) in group.keywords.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].keywords[{ki}].text"),
                    format!("{} / {} · palavra {}", campaign.name, group.name, ki + 1),
                    &keyword.text,
                    "text",
                    80,
                ));
            }
            for (ni, negative) in group.negatives.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].negatives[{ni}].text"),
                    format!("{} / {} · negativa {}", campaign.name, group.name, ni + 1),
                    &negative.text,
                    "text",
                    80,
                ));
            }
            for (hi, text) in group.rsa.headlines.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].rsa.headlines[{hi}]"),
                    format!("{} / {} · título {}", campaign.name, group.name, hi + 1),
                    text,
                    "text",
                    30,
                ));
            }
            for (di, text) in group.rsa.descriptions.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].rsa.descriptions[{di}]"),
                    format!("{} / {} · descrição {}", campaign.name, group.name, di + 1),
                    text,
                    "text",
                    90,
                ));
            }
            if let Some(path) = &group.rsa.path1 {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].rsa.path1"),
                    format!("{} / {} · path1", campaign.name, group.name),
                    path,
                    "text",
                    15,
                ));
            }
            if let Some(path) = &group.rsa.path2 {
                fields.push(edit_field(
                    format!("campaigns[{ci}].ad_groups[{gi}].rsa.path2"),
                    format!("{} / {} · path2", campaign.name, group.name),
                    path,
                    "text",
                    15,
                ));
            }
            let merged = merge_rsa(&group.rsa, merge_kit);
            ad_groups.push(AdGroupRow {
                name: group.name.clone(),
                cpc: group.default_cpc.format_cpc(comma),
                final_url: group.final_url.clone(),
                keywords: group
                    .keywords
                    .iter()
                    .map(|k| KeywordRow {
                        text: k.text.clone(),
                        kind: match k.match_type {
                            MatchType::Phrase => "frase".into(),
                            MatchType::Exact => "exata".into(),
                        },
                    })
                    .collect(),
                negatives: group.negatives.iter().map(|k| k.text.clone()).collect(),
                headlines: merged.headlines.iter().map(|t| text_row(t, 30)).collect(),
                descriptions: merged
                    .descriptions
                    .iter()
                    .map(|t| text_row(t, 90))
                    .collect(),
                path1: merged.path1.unwrap_or_default(),
                path2: merged.path2.unwrap_or_default(),
            });
        }
        for (gi, group) in campaign.asset_groups.iter().enumerate() {
            fields.push(edit_field(
                format!("campaigns[{ci}].asset_groups[{gi}].business_name"),
                format!("{} / {} · nome da empresa", campaign.name, group.name),
                &group.business_name,
                "text",
                0,
            ));
            fields.push(edit_field(
                format!("campaigns[{ci}].asset_groups[{gi}].final_url"),
                format!("{} / {} · URL final", campaign.name, group.name),
                &group.final_url,
                "url",
                0,
            ));
            for (hi, text) in group.headlines.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].asset_groups[{gi}].headlines[{hi}]"),
                    format!("{} / {} · título {}", campaign.name, group.name, hi + 1),
                    text,
                    "text",
                    30,
                ));
            }
            for (hi, text) in group.long_headlines.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].asset_groups[{gi}].long_headlines[{hi}]"),
                    format!(
                        "{} / {} · título longo {}",
                        campaign.name,
                        group.name,
                        hi + 1
                    ),
                    text,
                    "text",
                    90,
                ));
            }
            for (di, text) in group.descriptions.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].asset_groups[{gi}].descriptions[{di}]"),
                    format!("{} / {} · descrição {}", campaign.name, group.name, di + 1),
                    text,
                    "text",
                    90,
                ));
            }
            for (ti, text) in group.search_themes.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].asset_groups[{gi}].search_themes[{ti}]"),
                    format!("{} / {} · tema {}", campaign.name, group.name, ti + 1),
                    text,
                    "text",
                    0,
                ));
            }
        }
        if let Some(assets) = &campaign.assets {
            for (si, link) in assets.sitelinks.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].assets.sitelinks[{si}].text"),
                    format!("{} · sitelink {}", campaign.name, si + 1),
                    &link.text,
                    "text",
                    25,
                ));
                fields.push(edit_field(
                    format!("campaigns[{ci}].assets.sitelinks[{si}].url"),
                    format!("{} · URL do sitelink {}", campaign.name, si + 1),
                    &link.url,
                    "url",
                    0,
                ));
                if let Some(description) = &link.description1 {
                    fields.push(edit_field(
                        format!("campaigns[{ci}].assets.sitelinks[{si}].description1"),
                        format!("{} · descrição do sitelink {}", campaign.name, si + 1),
                        description,
                        "text",
                        35,
                    ));
                }
                if let Some(description) = &link.description2 {
                    fields.push(edit_field(
                        format!("campaigns[{ci}].assets.sitelinks[{si}].description2"),
                        format!("{} · descrição 2 do sitelink {}", campaign.name, si + 1),
                        description,
                        "text",
                        35,
                    ));
                }
            }
            for (ci2, callout) in assets.callouts.iter().enumerate() {
                fields.push(edit_field(
                    format!("campaigns[{ci}].assets.callouts[{ci2}]"),
                    format!("{} · callout {}", campaign.name, ci2 + 1),
                    callout,
                    "text",
                    25,
                ));
            }
            for (si, snippet) in assets.snippets.iter().enumerate() {
                for (vi, value) in snippet.values.iter().enumerate() {
                    fields.push(edit_field(
                        format!("campaigns[{ci}].assets.snippets[{si}].values[{vi}]"),
                        format!("{} · snippet {} valor {}", campaign.name, si + 1, vi + 1),
                        value,
                        "text",
                        25,
                    ));
                }
            }
        }
        let asset_groups = campaign
            .asset_groups
            .iter()
            .map(|group| AssetGroupRow {
                name: group.name.clone(),
                final_url: group.final_url.clone(),
                business_name: group.business_name.clone(),
                headlines: group.headlines.iter().map(|t| text_row(t, 30)).collect(),
                long_headlines: group
                    .long_headlines
                    .iter()
                    .map(|t| text_row(t, 90))
                    .collect(),
                descriptions: group.descriptions.iter().map(|t| text_row(t, 90)).collect(),
                search_themes: group.search_themes.clone(),
                images: group
                    .images
                    .iter()
                    .map(|image| {
                        let file = image.file.clone().unwrap_or_default();
                        ImageRow {
                            id: image.id.clone(),
                            ratio: image.ratio.label().to_string(),
                            prompt: image.prompt.clone(),
                            status: if file.is_empty() {
                                "faltando".into()
                            } else {
                                "gerada".into()
                            },
                            preview_url: if file.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "/api/runs/{run_id}/files/{}",
                                    encode_path(&format!("google-ads/editor/{file}"))
                                )
                            },
                            has_preview: !file.is_empty(),
                        }
                    })
                    .collect(),
            })
            .collect();
        let (sitelinks, callouts, snippets) = match &campaign.assets {
            Some(assets) => (
                assets
                    .sitelinks
                    .iter()
                    .map(|link| SitelinkRow {
                        text: link.text.clone(),
                        url: link.url.clone(),
                        description: link
                            .description1
                            .clone()
                            .or_else(|| link.description2.clone())
                            .unwrap_or_default(),
                    })
                    .collect(),
                assets.callouts.clone(),
                assets
                    .snippets
                    .iter()
                    .map(|snippet| SnippetRow {
                        header: snippet.header.label().to_string(),
                        values: snippet.values.join(", "),
                    })
                    .collect(),
            ),
            None => (Vec::new(), Vec::new(), Vec::new()),
        };
        let has_assets = !sitelinks.is_empty() || !callouts.is_empty() || !snippets.is_empty();
        campaigns.push(CampaignRow {
            name: campaign.name.clone(),
            kind: campaign.kind.label().to_string(),
            intent: intent_label(campaign.intent),
            budget: format!(
                "{} {}",
                campaign.daily_budget.format_budget(comma),
                input.budget.currency
            ),
            bid: campaign.bid_strategy.label().to_string(),
            rationale: campaign.rationale.clone(),
            ad_groups,
            asset_groups,
            negatives: campaign.negatives.iter().map(|k| k.text.clone()).collect(),
            keyword_count: keywords,
            sitelinks,
            callouts,
            snippets,
            has_assets,
        });
    }
    let total = account
        .campaigns
        .iter()
        .map(|c| c.daily_budget.0)
        .sum::<u64>();
    AccountView {
        brand_headlines: kit.as_ref().map_or(0, |kit| kit.headlines.len()),
        brand_descriptions: kit.as_ref().map_or(0, |kit| kit.descriptions.len()),
        campaign_count: campaigns.len(),
        ad_group_count: campaigns.iter().map(|c| c.ad_groups.len()).sum(),
        keyword_count: campaigns.iter().map(|c| c.keyword_count).sum(),
        budget_total: format!(
            "{} {}",
            mads_core::money::Cents(total).format_budget(comma),
            input.budget.currency
        ),
        fields,
        campaigns,
    }
}

fn edit_field(path: String, label: String, value: &str, kind: &str, limit: usize) -> EditField {
    EditField {
        path,
        label,
        value: value.to_string(),
        kind: kind.to_string(),
        limit,
    }
}

fn text_row(text: &str, limit: usize) -> TextRow {
    let len = text.chars().count();
    TextRow {
        text: text.to_string(),
        len,
        limit,
        over: len > limit,
    }
}

fn intent_label(intent: mads_core::google::Intent) -> String {
    match intent {
        mads_core::google::Intent::Brand => "marca".into(),
        mads_core::google::Intent::Catalog => "catálogo".into(),
        mads_core::google::Intent::Generic => "genérica".into(),
        mads_core::google::Intent::Competitor => "concorrente".into(),
    }
}

pub fn render_markdown(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    let mut builder = ammonia::Builder::default();
    builder.add_tags(
        ["table", "thead", "tbody", "tr", "th", "td"]
            .into_iter()
            .collect::<BTreeSet<_>>(),
    );
    builder.link_rel(Some("noopener noreferrer"));
    builder.clean(&output).to_string()
}

fn status_label(status: &str) -> (String, String) {
    match status {
        "success" => ("Pronto".into(), "ok".into()),
        "invalid" => ("Inválido".into(), "error".into()),
        "incomplete" => ("Incompleto".into(), "warn".into()),
        "cancelled" => ("Cancelado".into(), "muted".into()),
        "planned" => ("Plano pronto".into(), "planned".into()),
        "prepared" => ("Preparado".into(), "planned".into()),
        "running" => ("Em execução".into(), "running".into()),
        "interrupted" => ("Interrompido".into(), "warn".into()),
        other => (other.to_string(), "muted".into()),
    }
}

fn when_from_id(id: &str) -> String {
    let fmt = format_description::parse_borrowed::<2>("[year][month][day]-[hour][minute][second]");
    let out = format_description::parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]");
    if let (Ok(fmt), Ok(out)) = (fmt, out)
        && let Ok(dt) = OffsetDateTime::parse(id, &fmt)
    {
        return dt.format(&out).unwrap_or_else(|_| id.to_string());
    }
    id.to_string()
}

fn short_time(ts: &str) -> String {
    let fmt = format_description::parse_borrowed::<2>("[hour]:[minute]:[second]");
    if let Ok(fmt) = fmt
        && let Ok(dt) = OffsetDateTime::parse(ts, &time::format_description::well_known::Rfc3339)
    {
        return dt.format(&fmt).unwrap_or_else(|_| ts.to_string());
    }
    ts.get(11..19).unwrap_or(ts).to_string()
}

fn modified(dir: &Path) -> String {
    let Ok(meta) = fs::metadata(dir) else {
        return String::new();
    };
    let Ok(time) = meta.modified() else {
        return String::new();
    };
    let dt: OffsetDateTime = time.into();
    dt.format(
        &format_description::parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]")
            .unwrap_or_default(),
    )
    .unwrap_or_default()
}

fn tokens(values: impl IntoIterator<Item = u64>) -> String {
    let total: u64 = values.into_iter().sum();
    compact(total)
}

fn compact(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn cost_of(value: f64, known: bool) -> String {
    if known {
        format!("${value:.4}")
    } else {
        "n/a".into()
    }
}

fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

fn bytes_label(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.1} kB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

fn image_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn decode_text(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16le(&bytes[2..]);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16be(&bytes[2..]);
    }
    String::from_utf8(bytes.to_vec()).ok()
}

fn decode_utf16le(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16(&units).ok()
}

fn decode_utf16be(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes(*c))
        .collect();
    String::from_utf16(&units).ok()
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn valid_project_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub(crate) fn valid_run_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn pretty_slug(id: &str) -> String {
    id.split('-')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Marks the missions of runs from a previous server life as interrupted. The
/// in-memory job registry dies with the process, so a run left with `Running`
/// missions would otherwise show "Em execução" forever, with no stop and no
/// resume button. Core already counts anything not `Finished` as unfinished, so
/// the resume path picks these up.
pub fn sweep_interrupted_runs(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for project in entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
    {
        let Ok(runs) = fs::read_dir(project.path().join("out")) else {
            continue;
        };
        for run in runs.flatten() {
            let workspace_path = run.path().join("workspace.json");
            let Ok(mut workspace) = Workspace::load(&workspace_path) else {
                continue;
            };
            let mut changed = false;
            for mission in workspace.missions.values_mut() {
                if mission.status == MissionStatus::Running {
                    mission.status = MissionStatus::Failed {
                        reason: "interrupted".into(),
                    };
                    changed = true;
                }
            }
            if changed {
                let _ = workspace.save(&workspace_path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_ids_reject_traversal_and_absolute_paths() {
        assert!(!valid_run_id("../x"));
        assert!(!valid_run_id("/tmp"));
        assert!(!valid_run_id("a/b"));
        assert!(valid_run_id("20261024-153012-a1b2c3"));
    }

    #[test]
    fn project_ids_avoid_hidden_directories() {
        assert!(valid_project_id("vinellu"));
        assert!(valid_project_id("buser-bh-sp"));
        assert!(!valid_project_id(".trash"));
        assert!(!valid_project_id("../etc"));
    }

    #[test]
    fn text_rows_report_the_limit() {
        let row = text_row(&"a".repeat(31), 30);
        assert!(row.over);
        assert_eq!(row.len, 31);
    }

    #[test]
    fn the_editor_csv_preview_decodes_utf16le() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "Campanha\tCusto\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let text = decode_text(&bytes).unwrap();
        assert!(text.contains("Campanha"));
    }

    #[test]
    fn markdown_tables_survive_sanitization() {
        let html = render_markdown("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table"));
        assert!(html.contains("<td"));
    }

    #[test]
    fn paths_are_encoded_for_urls() {
        assert_eq!(encode_segment("a b"), "a%20b");
        assert_eq!(encode_path("google-ads/a b.csv"), "google-ads/a%20b.csv");
    }

    #[test]
    fn compact_numbers_are_short() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_500), "1.5k");
        assert_eq!(compact(2_000_000), "2.0M");
    }
}
