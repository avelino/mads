//! `mads optimize`: a new run that starts from a finished one and the reports of its live account.
//! The missions run again with the reports in their tools, through the same `generate`.

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    google::{Account, CampaignKind, Cents},
    images::EDITOR_DIR,
    input::{App, Input},
    perf::{Live, Performance, ReportInput, digest, read_table},
    run::{PLATFORM_DIR, RunDir, saved_layout, write_atomic},
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
    let performance = digest(&files, &old.account);

    let run = RunDir::create(out)?;
    copy_dir(&base.join("input"), &run.root().join("input"))?;
    let images = Path::new(PLATFORM_DIR).join(EDITOR_DIR).join("images");
    copy_dir(&base.join(&images), &run.root().join(&images))?;

    let mut account = old.account.clone();
    name_app_stores(&mut account, &old.input);
    let mut ws = Workspace::new(old.input.clone());
    if let Some(daily) = overrides.daily_budget {
        ws.input.budget.daily = daily;
    }
    if let Some(apps) = &overrides.apps {
        ws.input.apps = apps.clone();
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
    let run_id = run.id();
    let meta = serde_json::json!({
        "version": 1,
        "run_id": run_id,
        "provider": "",
        "model": null,
        "layout": saved_layout(base).unwrap_or_default(),
        "status": "prepared",
        "exit_code": 0,
        "totals": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cost_usd": null,
            "missions": 0,
            "failed_missions": 0
        },
        "origin": "optimize",
        "parent_run": base.file_name().and_then(|n| n.to_str()),
        "revision": 0,
        "edits": 0
    });
    write_atomic(
        &run.run_json_path(),
        &serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?,
    )?;
    Ok(Prepared {
        run_dir: run.root().to_path_buf(),
        performance,
    })
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
        input::AppStore,
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
    fn new_apps_replace_the_apps_of_the_new_run_only() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let apps = vec![
            App {
                store: AppStore::GooglePlay,
                id: "com.vinellu.app".into(),
            },
            App {
                store: AppStore::AppStore,
                id: "6751919708".into(),
            },
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
    fn a_live_app_campaign_keeps_the_store_of_the_run_it_came_from() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Finished);
        let path = base.join("workspace.json");
        let mut old = Workspace::load(&path).unwrap();
        old.input.apps = vec![App {
            store: AppStore::GooglePlay,
            id: "com.vinellu.app".into(),
        }];
        let mut app = old.account.campaigns[0].clone();
        (app.name, app.slug) = ("Vinellu - App".into(), "vinellu-app".into());
        app.kind = crate::google::CampaignKind::AppInstalls;
        app.app_store = None;
        old.account.campaigns.push(app);
        old.save(&path).unwrap();
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let overrides = Overrides {
            apps: Some(vec![
                App {
                    store: AppStore::GooglePlay,
                    id: "com.vinellu.app".into(),
                },
                App {
                    store: AppStore::AppStore,
                    id: "6751919708".into(),
                },
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
