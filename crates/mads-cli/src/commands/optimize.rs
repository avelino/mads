use mads_core::{
    google::Cents,
    input::apps_from_specs,
    optimize::{Overrides, prepare},
    perf::{DRIFT_SOURCE, Performance},
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
    let target_cpa_in_app = money("--target-cps", args.target_cps)?;
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
            target_cpa_in_app,
            app_events: args.app_events,
            pause_outside_run: args.pause_outside_run,
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
    if let Some(file) = &p.app_events_file {
        lines.push(format!("app events: {file} read"));
    }
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
    if !p.drift.is_empty() {
        lines.push(format!("drift since the last run ({DRIFT_SOURCE}):"));
        lines.extend(p.drift.lines().into_iter().map(|l| format!("  {l}")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use mads_core::{
        input::AppStore,
        perf::{
            AppPerf, CampaignDrift, CampaignPerf, Drift, DriftKind, LiveStatus,
            NO_ACTION_FOR_STORE, Network, NetworkPerf, OutsideCampaign, StoreEvents,
        },
    };

    use super::*;

    #[test]
    fn the_summary_names_the_app_events_before_the_missions_start() {
        let app = AppPerf::new(
            AppStore::GooglePlay,
            StoreEvents {
                installs: 2000.0,
                in_app_actions: 100.0,
            },
            300.0,
            2400.0,
        );
        let p = Performance {
            app_events_file: Some("events.csv".into()),
            campaigns: vec![CampaignPerf {
                name: "App Android".into(),
                app: Some(app),
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = summary(&p);
        assert!(text.contains("app events: events.csv read"), "{text}");
        assert!(
            text.contains(
                "app events of App Android: 2000 installs seen by the app, 100 in-app actions (5% of the installs), 3.00 per in-app action, Google counts 1.2 conversions per install seen"
            ),
            "{text}"
        );
    }

    #[test]
    fn the_summary_names_an_app_campaign_with_no_conversion_action_for_its_store() {
        let p = Performance {
            campaigns: vec![
                CampaignPerf {
                    name: "App iOS".into(),
                    untracked: true,
                    untracked_reason: Some(NO_ACTION_FOR_STORE.into()),
                    ..Default::default()
                },
                CampaignPerf {
                    name: "App Android".into(),
                    accidental_display_clicks: true,
                    networks: vec![NetworkPerf {
                        network: Network::Display,
                        cost_share_pct: Some(60.0),
                        conversions_share_pct: None,
                        ctr_pct: Some(22.5),
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let text = summary(&p);
        assert!(
            text.contains("untracked, no conversion action for this store"),
            "{text}"
        );
        assert!(text.contains("App iOS"), "{text}");
        assert!(
            text.contains("Display CTR of App Android is 22.5%"),
            "{text}"
        );
    }

    #[test]
    fn without_app_events_the_summary_says_nothing_about_them() {
        let p = Performance {
            campaigns: vec![CampaignPerf {
                name: "App Android".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!summary(&p).contains("app events"));
    }

    #[test]
    fn the_summary_names_the_drift_and_the_spend_outside_the_run() {
        let p = Performance {
            drift: Drift {
                campaigns: vec![CampaignDrift {
                    name: "App Android".into(),
                    change: DriftKind::Budget {
                        was: 100.0,
                        now: 150.0,
                    },
                }],
                account_budget_pct: Some(140.0),
            },
            outside: vec![OutsideCampaign {
                name: "Old".into(),
                live_status: Some(LiveStatus::Enabled),
                cost_share_pct: Some(25.0),
                spends_blind: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = summary(&p);
        for needle in [
            "drift since the last run (changed outside mads (by hand or by auto-applied recommendations)):",
            "  App Android: daily budget is 50% higher than the last run exported",
            "add up to 140% of the daily budget of this run",
            "Old (25% of the account cost, 0 clicks)",
            "pass --pause-outside-run to pause them",
        ] {
            assert!(text.contains(needle), "missing {needle} in\n{text}");
        }
        assert!(!summary(&Performance::default()).contains("drift"));
    }
}
