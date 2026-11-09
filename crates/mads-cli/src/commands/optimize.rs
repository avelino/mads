use mads_core::{
    google::Cents,
    input::apps_from_specs,
    optimize::{Overrides, prepare},
    perf::Performance,
    run::saved_layout,
    workspace::Workspace,
};

use crate::{
    cli::{CliError, Layout, OptimizeArgs, StatusArg},
    commands::generate::{drivers_for, execute},
    render::Format,
};

pub async fn run(args: OptimizeArgs, format: Format) -> anyhow::Result<i32> {
    let base = Workspace::load(&args.run_dir.join("workspace.json")).map_err(|e| CliError {
        code: 1,
        message: format!("cannot read run {}: {e}", args.run_dir.display()),
    })?;
    let money = |flag: &str, v: Option<f64>| {
        v.map(|v| {
            Cents::from_f64(v).ok_or_else(|| CliError {
                code: 2,
                message: format!("{flag} must be greater than 0 with at most 2 decimals"),
            })
        })
        .transpose()
    };
    let daily = money("--daily-budget", args.daily_budget)?;
    let target_cpa = money("--target-cpa", args.target_cpa)?;
    let target_cpi = money("--target-cpi", args.target_cpi)?;
    let apps = (!args.apps.is_empty())
        .then(|| apps_from_specs(&args.apps))
        .transpose()
        .map_err(|e| CliError {
            code: 2,
            message: e.to_string(),
        })?;
    let drivers = drivers_for(&args.run, &base.input).await?;
    let prepared = prepare(
        &args.run_dir,
        &args.reports,
        &args.run.out,
        &Overrides {
            daily_budget: daily,
            apps,
            export_status: args.status.map(StatusArg::core),
            target_cpa,
            target_cpi,
        },
    )
    .map_err(|e| CliError {
        code: 1,
        message: e.to_string(),
    })?;
    eprintln!("{}", summary(&prepared.performance));
    let ws = Workspace::load(&prepared.run_dir.join("workspace.json"))?;
    // An explicit --layout wins; without it the new run keeps the base run's layout.
    let layout = args
        .run
        .layout
        .map(Layout::core)
        .or_else(|| saved_layout(&args.run_dir));
    execute(
        &args.run,
        layout,
        ws.input,
        drivers,
        prepared.run_dir,
        format,
    )
    .await
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
