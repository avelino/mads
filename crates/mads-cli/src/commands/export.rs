use std::sync::Arc;

use anyhow::Context;
use mads_core::{events::EventSink, run::export_run};
use mads_providers::WebClient;

use crate::{
    cli::ExportArgs,
    render::{Format, pump},
};

pub async fn run(args: ExportArgs, format: Format) -> anyhow::Result<i32> {
    let web = Arc::new(WebClient::new().context("could not start the HTTP client")?);
    let (events, rx) = EventSink::channel();
    let printer = tokio::spawn(pump(rx, format));
    let result = export_run(
        &args.run_dir,
        web.as_ref(),
        args.skip_url_check,
        args.max_ad_groups,
        &events,
    )
    .await;
    drop(events);
    printer.await.context("event printer stopped")?;
    Ok(result?.exit_code)
}
