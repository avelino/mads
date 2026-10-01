use std::sync::Arc;

use anyhow::Context;
use mads_core::{
    events::EventSink,
    init::{InitConfig, InitError, run_init},
    input::normalize_url,
    money::Cents,
};
use mads_providers::{SiteClient, build_drivers, preflight};

use crate::{
    cli::{InitArgs, usage},
    render::{Format, pump},
};

/// Set to any value to let `init` read loopback and private hosts. For local development and tests.
const ALLOW_PRIVATE_ENV: &str = "MADS_ALLOW_PRIVATE_HOSTS";

pub async fn run(args: InitArgs, format: Format) -> anyhow::Result<i32> {
    if normalize_url(&args.from_url).is_none() {
        return Err(usage(format!(
            "--from-url must be an absolute http(s) URL, got '{}'",
            args.from_url
        )));
    }
    let daily = Cents::from_f64(args.daily_budget)
        .ok_or_else(|| usage("--daily-budget must be greater than 0 with at most 2 decimals"))?;
    let currency = &args.currency;
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(usage("--currency must be 3 uppercase letters, such as BRL"));
    }

    let selection = args.agent.selection();
    let drivers = build_drivers(&selection).map_err(|e| usage(e.to_string()))?;
    preflight(&selection)
        .await
        .map_err(|e| usage(e.to_string()))?;
    let allow_private = std::env::var_os(ALLOW_PRIVATE_ENV).is_some_and(|v| !v.is_empty());
    let site = Arc::new(SiteClient::new(&args.from_url, allow_private).map_err(usage)?);

    let mut cfg = InitConfig::new(
        args.out_dir.clone(),
        args.from_url.clone(),
        daily,
        currency.clone(),
    );
    cfg.catalog_limit = args.catalog_limit;
    cfg.force = args.force;
    cfg.max_turns = args.agent.max_turns;
    cfg.mission_timeout = args.agent.mission_timeout;
    cfg.max_tokens = args.agent.max_tokens;
    cfg.mission_retries = args.agent.mission_retries;
    cfg.provider = selection.provider.clone();
    cfg.model = selection.model.clone();

    let (events, rx) = EventSink::channel();
    let printer = tokio::spawn(pump(rx, format));
    let result = run_init(cfg, drivers.plan, site, events).await;
    printer.await.context("event printer stopped")?;
    match result {
        Ok(r) => {
            if r.exit_code == 0 && format != Format::Json {
                eprintln!(
                    "Review {} and the catalog, then run: mads generate {} --provider {}",
                    r.business.display(),
                    r.business.display(),
                    selection.provider
                );
            }
            Ok(r.exit_code)
        }
        Err(e @ InitError::Exists(_)) => Err(usage(e.to_string())),
        Err(e) => Err(e.into()),
    }
}
