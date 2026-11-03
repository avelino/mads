use std::sync::Arc;

use anyhow::Context;
use mads_core::{agent::RunControl, events::EventSink, run::export_run};
use mads_providers::WebClient;

use crate::{
    cli::{ExportArgs, Layout},
    render::{Format, pump},
};

pub async fn run(args: ExportArgs, format: Format) -> anyhow::Result<i32> {
    let web = Arc::new(WebClient::new().context("could not start the HTTP client")?);
    // An export appends to the run's events.ndjson: continue the sequence a
    // studio job (or an earlier CLI run) left there, or the reader-side
    // `Last-Event-ID` dedup drops every new event.
    let base = mads_core::events::last_seq_in_log(&args.run_dir.join("events.ndjson"));
    let (events, rx) = EventSink::channel_from(base);
    let printer = tokio::spawn(pump(rx, format));
    let result = export_run(
        &args.run_dir,
        web.as_ref(),
        args.skip_url_check,
        args.max_ad_groups,
        args.layout.map(Layout::core),
        &events,
        RunControl::default(),
    )
    .await;
    drop(events);
    printer.await.context("event printer stopped")?;
    Ok(result?.exit_code)
}
