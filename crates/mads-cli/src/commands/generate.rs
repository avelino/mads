use std::{path::Path, sync::Arc};

use anyhow::Context;
use mads_core::{
    events::EventSink,
    input::{Input, load_input, parse_input_toml},
    run::{RunConfig, RunDir, generate},
    workspace::Workspace,
};
use mads_providers::{WebClient, build_drivers, preflight};

use crate::{
    cli::{CliError, GenerateArgs, usage},
    render::{Format, pump},
};

/// Copies the inputs into the run directory so the run can be audited later.
fn snapshot(run: &RunDir, business: &Path) -> anyhow::Result<()> {
    let dest = run.root().join("input");
    std::fs::copy(business, dest.join("business.toml"))
        .context("could not copy business.toml into the run")?;
    let text = std::fs::read_to_string(business)?;
    if let Ok(file) = parse_input_toml(&text)
        && let Some(rel) = file.catalog_file
    {
        let src = business
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&rel);
        let name = Path::new(&rel).file_name().unwrap_or_default();
        std::fs::copy(&src, dest.join(name)).context("could not copy the catalog into the run")?;
    }
    Ok(())
}

/// Reads what the run needs without touching the disk: a resumed run keeps the input it started with.
fn load_input_for(args: &GenerateArgs) -> anyhow::Result<Input> {
    if let Some(dir) = &args.resume {
        let ws = Workspace::load(&dir.join("workspace.json")).map_err(|e| CliError {
            code: 1,
            message: format!("cannot resume {}: {e}", dir.display()),
        })?;
        return Ok(ws.input);
    }
    let path = args
        .business
        .as_deref()
        .ok_or_else(|| usage("business.toml is required"))?;
    load_input(path).map_err(|e| usage(e.to_string()))
}

/// The run directory to write to. A fresh run gets a snapshot of its inputs.
fn prepare_run_dir(args: &GenerateArgs) -> anyhow::Result<std::path::PathBuf> {
    if let Some(dir) = &args.resume {
        return Ok(dir.clone());
    }
    let run = RunDir::create(&args.out).context("could not create the run directory")?;
    if let Some(business) = args.business.as_deref() {
        snapshot(&run, business)?;
    }
    Ok(run.root().to_path_buf())
}

pub async fn run(args: GenerateArgs, format: Format) -> anyhow::Result<i32> {
    // Everything that can be wrong with the inputs fails here, before a run directory exists.
    let input = load_input_for(&args)?;
    let selection = args.agent.selection();
    let drivers = build_drivers(&selection).map_err(|e| usage(e.to_string()))?;
    preflight(&selection)
        .await
        .map_err(|e| usage(e.to_string()))?;
    let run_dir = prepare_run_dir(&args)?;

    let mut cfg = RunConfig::new(args.out.clone());
    cfg.run_dir = Some(run_dir);
    cfg.parallel = args.parallel;
    cfg.max_turns = args.agent.max_turns;
    cfg.mission_timeout = args.agent.mission_timeout;
    cfg.max_tokens = args.agent.max_tokens;
    cfg.mission_retries = args.agent.mission_retries;
    cfg.max_ad_groups = args.max_ad_groups;
    cfg.skip_url_check = args.skip_url_check;
    cfg.provider = selection.provider.clone();
    cfg.model = selection.model.clone();

    let web = Arc::new(WebClient::new().context("could not start the HTTP client")?);
    let (events, rx) = EventSink::channel();
    let printer = tokio::spawn(pump(rx, format));
    let result = generate(input, drivers, web, cfg, events).await;
    printer.await.context("event printer stopped")?;
    Ok(result?.exit_code)
}
