use mads_core::{optimize::prepare, perf::Performance, workspace::Workspace};

use crate::{
    cli::{CliError, Layout, OptimizeArgs, usage},
    commands::generate::{drivers_for, execute},
    render::Format,
};

pub async fn run(args: OptimizeArgs, format: Format) -> anyhow::Result<i32> {
    if args.run.layout == Some(Layout::DriveFolders) {
        return Err(usage(
            "--layout drive-folders creates a new account and cannot pause or edit a live one: optimize with --layout bulk",
        ));
    }
    let base = Workspace::load(&args.run_dir.join("workspace.json")).map_err(|e| CliError {
        code: 1,
        message: format!("cannot read run {}: {e}", args.run_dir.display()),
    })?;
    let drivers = drivers_for(&args.run, &base.input).await?;
    let prepared = prepare(&args.run_dir, &args.reports, &args.run.out).map_err(|e| CliError {
        code: 1,
        message: e.to_string(),
    })?;
    eprintln!("{}", summary(&prepared.performance));
    let ws = Workspace::load(&prepared.run_dir.join("workspace.json"))?;
    execute(&args.run, ws.input, drivers, prepared.run_dir, format).await
}

/// One line per thing the reports told, before the missions start.
fn summary(p: &Performance) -> String {
    let mut lines = Vec::new();
    let window = p.window.as_ref().map_or_else(
        || "no date range".to_string(),
        |w| format!("{} to {}, {} days", w.start, w.end, w.days),
    );
    lines.push(format!("reports: {} read ({window})", p.reports.len()));
    lines.extend(p.unknown_files.iter().map(|u| format!("skipped: {u}")));
    if p.mixed_windows {
        lines.push(
            "warning: the reports cover different dates, export them for the same dates".into(),
        );
    }
    if !p.ignored_campaigns.is_empty() {
        lines.push(format!(
            "not in this run: {}",
            p.ignored_campaigns.join(", ")
        ));
    }
    let thin: Vec<&str> = p
        .campaigns
        .iter()
        .filter(|c| c.thin)
        .map(|c| c.name.as_str())
        .collect();
    if !thin.is_empty() {
        lines.push(format!(
            "too little data, structure only: {}",
            thin.join(", ")
        ));
    }
    lines.extend(p.findings());
    lines.join("\n")
}
