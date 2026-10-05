//! `mads optimize`: a new run that starts from a finished one and the reports of its live account.
//! The missions run again with the reports in their tools, through the same `generate`.

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    images::EDITOR_DIR,
    perf::{Live, Performance, ReportInput, digest, read_table},
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
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct Prepared {
    pub run_dir: PathBuf,
    pub performance: Performance,
}

/// Creates the optimization run under `out` and returns its directory. Nothing is written when
/// the base run or the reports are not usable.
pub fn prepare(base: &Path, reports: &Path, out: &Path) -> Result<Prepared, OptimizeError> {
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

    let mut ws = Workspace::new(old.input.clone());
    ws.input.logo = old.input.logo.as_deref().map(|logo| {
        Path::new(logo).strip_prefix(base).map_or_else(
            |_| logo.to_string(),
            |rel| run.root().join(rel).display().to_string(),
        )
    });
    ws.account = old.account.clone();
    ws.live = Some(Live {
        baseline: old.account,
        performance: performance.clone(),
    });
    ws.save(&run.workspace_path())?;
    Ok(Prepared {
        run_dir: run.root().to_path_buf(),
        performance,
    })
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
        let p = prepare(&base, &perf, &tmp.path().join("out")).unwrap();
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
    fn refuses_an_unfinished_run() {
        let tmp = tempfile::tempdir().unwrap();
        let base = base_run(tmp.path(), MissionStatus::Pending);
        let perf = reports(tmp.path(), &[("c.csv", REPORT)]);
        let err = prepare(&base, &perf, &tmp.path().join("out"))
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
        let err = prepare(&base, &perf, &tmp.path().join("new"))
            .err()
            .unwrap();
        assert!(
            err.to_string().contains("x.csv: no known report header"),
            "{err}"
        );
        assert!(!tmp.path().join("new").exists(), "nothing is written");
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let err = prepare(&base, &empty, &tmp.path().join("new"))
            .err()
            .unwrap();
        assert!(err.to_string().contains("no .csv file"), "{err}");
    }

    #[test]
    fn a_missing_base_run_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = prepare(&tmp.path().join("nope"), tmp.path(), tmp.path())
            .err()
            .unwrap();
        assert!(err.to_string().starts_with("cannot read run"), "{err}");
    }
}
