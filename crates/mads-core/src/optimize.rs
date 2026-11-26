//! `mads optimize`: a new run that starts from a finished one and the reports of its live account.
//! The missions run again with the reports in their tools, through the same `generate`.

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    google::{Account, CampaignKind, Cents},
    images::EDITOR_DIR,
    input::{App, ExportStatus, Input},
    perf::{
        AppEvents, Base, Live, Performance, ReportInput, attach_app_events, digest, drift_since,
        read_table,
    },
    run::{PLATFORM_DIR, RunDir},
    workspace::{MissionStatus, Workspace},
};

#[derive(Debug, thiserror::Error)]
pub enum OptimizeError {
    #[error("cannot read run {0}: {1}")]
    Base(String, String),
    #[error(
        "run has unfinished missions ({0}); finish it with `mads generate --resume <run-dir>` first"
    )]
    Unfinished(String),
    #[error("cannot read reports folder {0}: {1}")]
    Reports(String, String),
    #[error(
        "no Google Ads report in {dir}: export search terms, keywords or campaigns as CSV ({details})"
    )]
    NoReports { dir: String, details: String },
    #[error("cannot read app events {0}: {1}")]
    AppEvents(String, String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct Prepared {
    pub run_dir: PathBuf,
    pub performance: Performance,
}

/// What the command line changes in the input the new run copies from the base run. The live
/// baseline keeps the budgets and apps it runs with.
#[derive(Debug, Default)]
pub struct Overrides {
    /// Replaces the account budget of the new plan.
    pub daily_budget: Option<Cents>,
    /// Replaces the apps App campaigns promote.
    pub apps: Option<Vec<App>>,
    /// Replaces the status campaigns without a live status are exported with.
    pub export_status: Option<ExportStatus>,
    /// Replaces the most the advertiser pays for one conversion.
    pub target_cpa: Option<Cents>,
    /// Replaces the most the advertiser pays for one app install.
    pub target_cpi: Option<Cents>,
    /// Replaces the most the advertiser pays for one in-app action.
    pub target_cpa_in_app: Option<Cents>,
    /// CSV of the installs and in-app actions the advertiser's own analytics counted.
    pub app_events: Option<PathBuf>,
    /// Writes a `Paused` row for each enabled campaign outside the run that spends without
    /// converting. Without it the campaigns outside the run are left as they are.
    pub pause_outside_run: bool,
}

/// Creates the optimization run under `out` and returns its directory. Nothing is written when
/// the base run or the reports are not usable.
pub fn prepare(
    base: &Path,
    reports: &Path,
    out: &Path,
    overrides: &Overrides,
) -> Result<Prepared, OptimizeError> {
    let shown = base.display().to_string();
    let old = Workspace::load(&base.join("workspace.json"))
        .map_err(|e| OptimizeError::Base(shown.clone(), e.to_string()))?;
    unfinished(&old)?;
    let files = read_reports(reports)?;
    let events = overrides
        .app_events
        .as_deref()
        .map(read_events)
        .transpose()?;
    // The digest reads the store of each App campaign, so it starts from the account that names them.
    let mut account = old.account.clone();
    name_app_stores(&mut account, &old.input);
    let mut performance = digest(&files, &account);

    let run = RunDir::create(out)?;
    copy_dir(&base.join("input"), &run.root().join("input"))?;
    let images = Path::new(PLATFORM_DIR).join(EDITOR_DIR).join("images");
    copy_dir(&base.join(&images), &run.root().join(&images))?;

    if let Some((file, events)) = &events {
        attach_app_events(&mut performance, &account, events);
        performance.app_events_file = Some(file.clone());
    }
    let mut ws = Workspace::new(old.input.clone());
    if let Some(daily) = overrides.daily_budget {
        ws.input.budget.daily = daily;
    }
    // Before the budgets follow the live ones: drift compares them with what the base run exported.
    let exported = Base {
        account: &old.account,
        live: old.live.as_ref(),
        export_status: old.input.export.status,
    };
    performance.drift = drift_since(&exported, &performance, ws.input.budget.daily);
    if overrides.pause_outside_run {
        performance.pause_outside_run = performance
            .outside_blind()
            .into_iter()
            .map(|o| o.name.clone())
            .collect();
    }
    follow_live_budgets(&mut account, &performance);
    ws.input.budget.target_cpa = overrides.target_cpa.or(ws.input.budget.target_cpa);
    ws.input.budget.target_cpi = overrides.target_cpi.or(ws.input.budget.target_cpi);
    ws.input.budget.target_cpa_in_app = overrides
        .target_cpa_in_app
        .or(ws.input.budget.target_cpa_in_app);
    if let Some(status) = overrides.export_status {
        ws.input.export.status = status;
    }
    if let Some(apps) = &overrides.apps {
        ws.input.apps = keep_goals(apps.clone(), &old.input.apps);
    }
    ws.input.logo = old.input.logo.as_deref().map(|logo| {
        Path::new(logo).strip_prefix(base).map_or_else(
            |_| logo.to_string(),
            |rel| run.root().join(rel).display().to_string(),
        )
    });
    ws.account = account.clone();
    ws.live = Some(Live {
        baseline: account,
        performance: performance.clone(),
    });
    ws.save(&run.workspace_path())?;
    Ok(Prepared {
        run_dir: run.root().to_path_buf(),
        performance,
    })
}

/// `--app` gives a store and an id and says nothing of what the app optimizes for: an app that
/// stays, same store and same id, keeps the goal it had.
fn keep_goals(mut apps: Vec<App>, old: &[App]) -> Vec<App> {
    for app in &mut apps {
        if let Some(was) = old.iter().find(|o| o.store == app.store && o.id == app.id) {
            app.optimize_for = was.optimize_for;
            app.conversion_action = was.conversion_action.clone();
        }
    }
    apps
}

/// A run made with one app leaves `app_store` empty on its App campaigns. The new run can have two
/// apps, so each live App campaign names the store it runs on and cannot be moved to the other.
fn name_app_stores(account: &mut Account, input: &Input) {
    let only = input.app_for(None).map(|a| a.store);
    for c in &mut account.campaigns {
        if c.kind == CampaignKind::AppInstalls && c.app_store.is_none() {
            c.app_store = only;
        }
    }
}

/// A budget changed in Google Ads after the import is the one the campaign ran with: caps and
/// the changes of the report start from it, not from what the base run exported.
fn follow_live_budgets(account: &mut Account, p: &Performance) {
    for c in &mut account.campaigns {
        let live = p.campaign(&c.name).and_then(|l| l.budget);
        if let Some(budget) = live.and_then(Cents::from_f64) {
            c.daily_budget = budget;
        }
    }
}

fn unfinished(ws: &Workspace) -> Result<(), OptimizeError> {
    let open: Vec<&str> = ws
        .missions
        .iter()
        .filter(|(_, m)| m.status != MissionStatus::Finished)
        .map(|(id, _)| id.as_str())
        .collect();
    if !open.is_empty() {
        return Err(OptimizeError::Unfinished(open.join(", ")));
    }
    if ws.account.campaigns.is_empty() {
        return Err(OptimizeError::Unfinished("plan".into()));
    }
    Ok(())
}

/// Every `.csv` of the folder, sorted by name. At least one must be a report mads knows.
fn read_reports(dir: &Path) -> Result<Vec<ReportInput>, OptimizeError> {
    let shown = dir.display().to_string();
    let entries =
        std::fs::read_dir(dir).map_err(|e| OptimizeError::Reports(shown.clone(), e.to_string()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("csv")))
        .collect();
    paths.sort();
    let mut files = Vec::new();
    for p in paths {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&p)?;
        files.push((name, read_table(&bytes)));
    }
    if !files.iter().any(|(_, t)| t.is_ok()) {
        let details = if files.is_empty() {
            "no .csv file".to_string()
        } else {
            files
                .iter()
                .filter_map(|(n, t)| t.as_ref().err().map(|e| format!("{n}: {e}")))
                .collect::<Vec<_>>()
                .join("; ")
        };
        return Err(OptimizeError::NoReports {
            dir: shown,
            details,
        });
    }
    Ok(files)
}

/// The app events file and its name.
fn read_events(path: &Path) -> Result<(String, AppEvents), OptimizeError> {
    let shown = path.display().to_string();
    let bytes =
        std::fs::read(path).map_err(|e| OptimizeError::AppEvents(shown.clone(), e.to_string()))?;
    let events = AppEvents::parse(&bytes).map_err(|e| OptimizeError::AppEvents(shown, e))?;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Ok((name, events))
}

/// Copies a folder tree. A missing source copies nothing.
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{BidStrategy, Campaign, Cents, Intent},
        input::{AppStore, ExportStatus},
        perf::DriftKind,
        testutil,
        workspace::MissionState,
    };

    const REPORT: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Custo,Cliques,Impr.\nAtivada,Vinellu - Marca,\"10,00\",8,90\n";

    fn base_run(root: &Path, status: MissionStatus) -> PathBuf {
        let base = root.join("out").join("base");
        std::fs::create_dir_all(&base).unwrap();
        let run = RunDir::open(&base).unwrap();
        std::fs::write(run.root().join("input/business.toml"), "x").unwrap();
        let images = run.platform_dir().join("editor/images/marca");
        std::fs::create_dir_all(&images).unwrap();
        std::fs::write(images.join("g-a.jpg"), b"jpg").unwrap();
        let mut ws = Workspace::new(testutil::input());
        ws.account.campaigns.push(Campaign {
            app_store: None,
            kind: Default::default(),
            asset_groups: Vec::new(),
            name: "Vinellu - Marca".into(),
            slug: "vinellu-marca".into(),
            intent: Intent::Brand,
            daily_budget: Cents(2000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: "r".into(),
            planned_ad_groups: Vec::new(),
            ad_groups: Vec::new(),
            negatives: Vec::new(),
            assets: None,
        });
        ws.missions.insert(
            "plan".into(),
            MissionState {
                status,
                ..Default::default()
            },
        );
        ws.save(&run.workspace_path()).unwrap();
        base
    }

    fn reports(root: &Path, files: &[(&str, &str)]) -> PathBuf {
        let dir = root.join("perf");
        std::fs::create_dir_all(&dir).unwrap();
        for (n, text) in files {
            std::fs::write(dir.join(n), text).unwrap();
        }
        dir
    }

    #[test]
    fn prepares_a_new_run_from_a_finished_one() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT), ("notes.txt", "skip me")]);
        let p = prepare(&base, &perf, &tmp.path().join("out"), &Overrides::default()).unwrap();
        assert_ne!(p.run_dir, base);
        assert!(p.run_dir.join("input/business.toml").exists());
        assert!(
            p.run_dir
                .join("google-ads/editor/images/marca/g-a.jpg")
                .exists()
        );
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert!(ws.missions.is_empty(), "every mission runs again");
        let live = ws.live.expect("live data");
        assert_eq!(live.baseline.campaigns.len(), 1);
        assert_eq!(ws.account, live.baseline);
        assert_eq!(live.performance.campaigns[0].metrics.clicks, 8);
        assert_eq!(p.performance.reports.len(), 1);
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert!(old.live.is_none(), "the base run is not touched");
    }

    #[test]
    fn a_new_daily_budget_replaces_the_account_budget_only_in_the_new_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let kept = prepare(&base, &perf, &tmp.path().join("a"), &Overrides::default()).unwrap();
        let kept = Workspace::load(&kept.run_dir.join("workspace.json")).unwrap();
        let old_daily = kept.input.budget.daily;
        let p = prepare(
            &base,
            &perf,
            &tmp.path().join("b"),
            &Overrides {
                daily_budget: Some(Cents(50000)),
                ..Overrides::default()
            },
        )
        .unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.input.budget.daily, Cents(50000));
        assert_ne!(old_daily, Cents(50000));
        assert_eq!(
            ws.account.campaigns[0].daily_budget,
            Cents(2000),
            "live budget"
        );
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert_eq!(
            old.input.budget.daily, old_daily,
            "the base run is not touched"
        );
    }

    #[test]
    fn a_budget_changed_in_google_ads_is_the_budget_the_campaign_ran_with() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let report = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Orçamento,Custo,Cliques,Impr.\nAtivada,Vinellu - Marca,\"25,50\",\"10,00\",8,90\n";
        let perf = reports(tmp.path(), &[("c.csv", report)]);
        let p = prepare(&base, &perf, &tmp.path().join("b"), &Overrides::default()).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        let live = ws.live.unwrap();
        assert_eq!(live.baseline.campaigns[0].daily_budget, Cents(2550));
        assert_eq!(ws.account.campaigns[0].daily_budget, Cents(2550));
    }

    #[test]
    fn new_apps_replace_the_apps_of_the_new_run_only() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let apps = vec![
            App::installs(AppStore::GooglePlay, "com.vinellu.app"),
            App::installs(AppStore::AppStore, "6751919708"),
        ];
        let overrides = Overrides {
            apps: Some(apps.clone()),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.input.apps, apps);
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert!(old.input.apps.is_empty(), "the base run is not touched");
        let kept = prepare(&base, &perf, &tmp.path().join("c"), &Overrides::default()).unwrap();
        let kept = Workspace::load(&kept.run_dir.join("workspace.json")).unwrap();
        assert!(kept.input.apps.is_empty(), "no flag, the base apps stay");
    }

    #[test]
    fn new_targets_replace_the_targets_of_the_new_run_only() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let overrides = Overrides {
            target_cpa: Some(Cents(4000)),
            target_cpi: Some(Cents(250)),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.input.budget.target_cpa, Some(Cents(4000)));
        assert_eq!(ws.input.budget.target_cpi, Some(Cents(250)));
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert_eq!(
            old.input.budget.target_cpa, None,
            "the base run is not touched"
        );
        let kept = prepare(&base, &perf, &tmp.path().join("c"), &Overrides::default()).unwrap();
        let kept = Workspace::load(&kept.run_dir.join("workspace.json")).unwrap();
        assert_eq!(kept.input.budget.target_cpa, None, "no flag, none appears");
    }

    #[test]
    fn a_new_in_app_target_replaces_the_target_of_the_new_run_only() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let overrides = Overrides {
            target_cpa_in_app: Some(Cents(1250)),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.input.budget.target_cpa_in_app, Some(Cents(1250)));
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert_eq!(old.input.budget.target_cpa_in_app, None);
        let kept = prepare(&base, &perf, &tmp.path().join("c"), &Overrides::default()).unwrap();
        let kept = Workspace::load(&kept.run_dir.join("workspace.json")).unwrap();
        assert_eq!(kept.input.budget.target_cpa_in_app, None, "no flag, none");
    }

    const APP_REPORT: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Custo,Cliques,Impr.,Conversões\nAtivada,Vinellu - Marca,\"10,00\",8,90,\"0,00\"\nAtivada,Vinellu - App,\"300,00\",500,9000,\"2.400,00\"\n";
    const EVENTS: &str = "date,store,installs,in_app_actions\n2026-10-02,google_play,999,99\n2026-10-03,google_play,1000,50\n2026-10-05,google_play,1000,50\n";

    /// A base run whose account has a Google Play App campaign next to the Search one.
    fn base_run_with_app(root: &Path) -> PathBuf {
        let base = base_run(root, MissionStatus::Finished);
        let path = base.join("workspace.json");
        let mut old = Workspace::load(&path).unwrap();
        old.input.apps = vec![App::installs(AppStore::GooglePlay, "com.example.app")];
        let mut app = old.account.campaigns[0].clone();
        (app.name, app.slug) = ("Vinellu - App".into(), "vinellu-app".into());
        app.kind = crate::google::CampaignKind::AppInstalls;
        app.app_store = None;
        old.account.campaigns.push(app);
        old.save(&path).unwrap();
        base
    }

    #[test]
    fn app_events_reach_the_app_campaigns_of_their_store() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run_with_app(tmp.path());
        let perf = reports(tmp.path(), &[("c.csv", APP_REPORT)]);
        let events = tmp.path().join("events.csv");
        std::fs::write(&events, EVENTS).unwrap();
        let overrides = Overrides {
            app_events: Some(events),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        assert_eq!(p.performance.app_events_file.as_deref(), Some("events.csv"));
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        let live = ws.live.unwrap();
        let app = live.performance.campaign("Vinellu - App").unwrap();
        let app = app.app.as_ref().expect("app events");
        assert_eq!((app.app_installs_seen, app.in_app_actions), (2000.0, 100.0));
        assert_eq!(app.cost_per_in_app_action, Some(3.0));
        assert_eq!(app.store_vs_app_install_ratio, Some(1.2));
        let marca = live.performance.campaign("Vinellu - Marca").unwrap();
        assert!(marca.app.is_none());
    }

    #[test]
    fn without_app_events_the_performance_has_no_app_numbers() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run_with_app(tmp.path());
        let perf = reports(tmp.path(), &[("c.csv", APP_REPORT)]);
        let p = prepare(&base, &perf, &tmp.path().join("b"), &Overrides::default()).unwrap();
        assert_eq!(p.performance.app_events_file, None);
        assert!(p.performance.app_events_missing.is_empty());
        assert!(p.performance.campaigns.iter().all(|c| c.app.is_none()));
        let json = std::fs::read_to_string(p.run_dir.join("workspace.json")).unwrap();
        assert!(!json.contains("app_events") && !json.contains("target_cpa_in_app"));
    }

    #[test]
    fn an_unreadable_app_events_file_stops_before_anything_is_written() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run_with_app(tmp.path());
        let perf = reports(tmp.path(), &[("c.csv", APP_REPORT)]);
        let run = |path: PathBuf| {
            let overrides = Overrides {
                app_events: Some(path),
                ..Overrides::default()
            };
            prepare(&base, &perf, &tmp.path().join("new"), &overrides)
                .err()
                .unwrap()
                .to_string()
        };
        let err = run(tmp.path().join("nope.csv"));
        assert!(err.starts_with("cannot read app events"), "{err}");
        let bad = tmp.path().join("bad.csv");
        let text = "date,store,installs,in_app_actions\n2026-10-03,huawei,1,1\n";
        std::fs::write(&bad, text).unwrap();
        let err = run(bad);
        assert!(err.contains("unknown store 'huawei'"), "{err}");
        assert!(!tmp.path().join("new").exists(), "nothing is written");
    }

    #[test]
    fn the_app_flag_keeps_the_goal_of_an_app_that_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run_with_app(tmp.path());
        let path = base.join("workspace.json");
        let mut old = Workspace::load(&path).unwrap();
        old.input.apps[0].optimize_for = crate::input::AppGoal::InAppActions;
        old.input.apps[0].conversion_action = "Sign up".into();
        old.save(&path).unwrap();
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let apps = |play: &str| {
            Some(vec![
                App::installs(AppStore::GooglePlay, play),
                App::installs(AppStore::AppStore, "6751919708"),
            ])
        };
        let run = |name: &str, play: &str| {
            let overrides = Overrides {
                apps: apps(play),
                ..Overrides::default()
            };
            let p = prepare(&base, &perf, &tmp.path().join(name), &overrides).unwrap();
            Workspace::load(&p.run_dir.join("workspace.json"))
                .unwrap()
                .input
                .apps
        };
        let same = run("a", "com.example.app");
        assert_eq!(same[0].optimize_for, crate::input::AppGoal::InAppActions);
        assert_eq!(same[0].conversion_action, "Sign up");
        assert_eq!(same[1].optimize_for, crate::input::AppGoal::Installs);
        let other = run("b", "com.other.app");
        assert_eq!(other[0].optimize_for, crate::input::AppGoal::Installs);
        assert_eq!(other[0].conversion_action, "");
    }

    #[test]
    fn a_new_export_status_replaces_the_status_of_the_new_run_only() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let before = Workspace::load(&base.join("workspace.json"))
            .unwrap()
            .input
            .export
            .status;
        let want = match before {
            ExportStatus::Paused => ExportStatus::Enabled,
            ExportStatus::Enabled => ExportStatus::Paused,
        };
        let overrides = Overrides {
            export_status: Some(want),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.input.export.status, want);
        let old = Workspace::load(&base.join("workspace.json")).unwrap();
        assert_eq!(
            old.input.export.status, before,
            "the base run is not touched"
        );
        let kept = prepare(&base, &perf, &tmp.path().join("c"), &Overrides::default()).unwrap();
        let kept = Workspace::load(&kept.run_dir.join("workspace.json")).unwrap();
        assert_eq!(
            kept.input.export.status, before,
            "no flag, the status stays"
        );
    }

    #[test]
    fn a_live_app_campaign_keeps_the_store_of_the_run_it_came_from() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let path = base.join("workspace.json");
        let mut old = Workspace::load(&path).unwrap();
        old.input.apps = vec![App::installs(AppStore::GooglePlay, "com.vinellu.app")];
        let mut app = old.account.campaigns[0].clone();
        (app.name, app.slug) = ("Vinellu - App".into(), "vinellu-app".into());
        app.kind = crate::google::CampaignKind::AppInstalls;
        app.app_store = None;
        old.account.campaigns.push(app);
        old.save(&path).unwrap();
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let overrides = Overrides {
            apps: Some(vec![
                App::installs(AppStore::GooglePlay, "com.vinellu.app"),
                App::installs(AppStore::AppStore, "6751919708"),
            ]),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        let live = ws.live.unwrap();
        let store_of = |a: &crate::google::Account| {
            a.campaigns.iter().map(|c| c.app_store).collect::<Vec<_>>()
        };
        let want = vec![None, Some(AppStore::GooglePlay)];
        assert_eq!(
            store_of(&live.baseline),
            want,
            "the baseline names its store"
        );
        assert_eq!(store_of(&ws.account), want);
    }

    const OUTSIDE_REPORT: &str = "Relatório de campanha\n3 de outubro de 2026 - 5 de outubro de 2026\nStatus da campanha,Campanha,Orçamento,Tipo de estratégia de lances,Custo,Cliques,Impr.,Conversões\nAtivada,Vinellu - Marca,\"30,00\",Maximizar conversões,\"10,00\",8,90,\"4,00\"\nAtivada,Antiga,\"50,00\",CPC manual,\"90,00\",150,3000,\"0,00\"\nPausada,Antiga pausada,\"50,00\",CPC manual,\"90,00\",150,3000,\"0,00\"\nRemovida,Antiga removida,\"50,00\",CPC manual,\"90,00\",150,3000,\"0,00\"\n";

    #[test]
    fn drift_is_measured_against_what_the_base_run_exported() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", OUTSIDE_REPORT)]);
        let overrides = Overrides {
            daily_budget: Some(Cents(5000)),
            ..Overrides::default()
        };
        let p = prepare(&base, &perf, &tmp.path().join("b"), &overrides).unwrap();
        let drift = &p.performance.drift;
        let budget = drift.campaigns.iter().any(|c| {
            c.name == "Vinellu - Marca"
                && matches!(&c.change, DriftKind::Budget { was, now } if *was == 20.0 && *now == 30.0)
        });
        assert!(budget, "{drift:?}");
        let bids = drift.campaigns.iter().any(
            |c| matches!(&c.change, DriftKind::BidStrategy { was, .. } if was == "Manual CPC"),
        );
        assert!(bids, "{drift:?}");
        // 30 and 50 are enabled and live: 160 percent of the 50 the new run is given.
        assert_eq!(drift.account_budget_pct, Some(160.0));
        let ws = Workspace::load(&p.run_dir.join("workspace.json")).unwrap();
        assert_eq!(ws.account.campaigns[0].daily_budget, Cents(3000), "live");
        assert_eq!(ws.live.unwrap().performance.drift, *drift);
    }

    #[test]
    fn a_report_that_matches_the_base_run_has_no_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let p = prepare(&base, &perf, &tmp.path().join("b"), &Overrides::default()).unwrap();
        assert!(p.performance.drift.is_empty(), "{:?}", p.performance.drift);
        let json = std::fs::read_to_string(p.run_dir.join("workspace.json")).unwrap();
        assert!(!json.contains("\"drift\""), "an empty drift is not written");
    }

    #[test]
    fn pause_outside_run_lists_only_the_campaigns_that_spend_without_converting() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", OUTSIDE_REPORT)]);
        let run = |name: &str, pause_outside_run: bool| {
            let overrides = Overrides {
                pause_outside_run,
                ..Overrides::default()
            };
            let p = prepare(&base, &perf, &tmp.path().join(name), &overrides).unwrap();
            Workspace::load(&p.run_dir.join("workspace.json"))
                .unwrap()
                .live
                .unwrap()
                .performance
        };
        let kept = run("a", false);
        assert_eq!(kept.outside_blind().len(), 1, "still reported");
        assert!(
            kept.pause_outside_run.is_empty(),
            "no flag, nothing to pause"
        );
        let paused = run("b", true);
        assert_eq!(paused.pause_outside_run, ["Antiga"]);
    }

    #[test]
    fn refuses_an_unfinished_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Pending);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let err = prepare(&base, &perf, &tmp.path().join("out"), &Overrides::default())
            .err()
            .unwrap();
        assert!(
            err.to_string().contains("unfinished missions (plan)"),
            "{err}"
        );
    }

    #[test]
    fn refuses_a_folder_without_reports() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("x.csv", "foo,bar\n1,2\n")]);
        let err = prepare(&base, &perf, &tmp.path().join("new"), &Overrides::default())
            .err()
            .unwrap();
        assert!(
            err.to_string().contains("x.csv: no known report header"),
            "{err}"
        );
        assert!(!tmp.path().join("new").exists(), "nothing is written");
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let err = prepare(
            &base,
            &empty,
            &tmp.path().join("new"),
            &Overrides::default(),
        )
        .err()
        .unwrap();
        assert!(err.to_string().contains("no .csv file"), "{err}");
    }

    #[test]
    fn a_missing_base_run_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = prepare(
            &tmp.path().join("nope"),
            tmp.path(),
            tmp.path(),
            &Overrides::default(),
        )
        .err()
        .unwrap();
        assert!(err.to_string().starts_with("cannot read run"), "{err}");
    }
}
