# Architecture

This page explains how the crates fit together, how a mission runs and how to embed `mads-core` in your own program.

## Crates

```text
business.toml, catalog.csv
        |
        v
  +---------------------- mads-core ----------------------+
  | input      parse and validate business.toml, catalog   |
  | init       init mission, tools and draft state         |
  | google     domain model, rules, expansion, export      |
  | workspace  account state and mission state             |
  | tools      typed tools, JSON schemas, MissionTools     |
  | mission    mission prompts and ids                     |
  | agent      ChatModel, Driver, LoopDriver               |
  | run        orchestration, limits, resume, export       |
  | post       cross negatives, URL check                  |
  | finalize   validate, URL check, export                 |
  | events     Event, EventSink                            |
  | report     report.md                                   |
  +--------^------------------------------^----------------+
           |                              |
    mads-providers                    mads-cli
    GenaiChatModel, CliDriver,        the mads binary,
    MCP server, SiteClient,           renderers, exit codes
    WebClient
```

| Crate | Owns |
|---|---|
| `mads-core` | Everything that is not IO. Input parsing, the Google model and rules, keyword expansion, RSA merge, CSV export, workspace, tools, missions, the agent loop, orchestration, events and the report. |
| `mads-providers` | `GenaiChatModel` (all API providers through `genai`), `CliDriver` with the claude, codex and gemini agents, the per-mission MCP server, provider listing and selection, `WebClient` and `SiteClient`. |
| `mads-cli` | The `mads` binary. Argument parsing, renderers, `events.ndjson`, exit codes. |

The dependency rule is strict. `mads-cli` depends on `mads-providers` and `mads-core`. `mads-providers` depends on `mads-core`. `mads-core` performs no network IO. It defines traits and the other crates implement them.

## Ports

| Trait | Purpose | Implemented by |
|---|---|---|
| `ChatModel` | One LLM turn. System prompt, messages and tool specs in. Text, tool calls and usage out. | `GenaiChatModel` (providers), `ScriptedChatModel` (core, tests) |
| `Driver` | Runs one whole mission against a `ToolHost`. | `LoopDriver<M: ChatModel>` and `ScriptedDriver` (core), `CliDriver<A: CliAgent>` (providers) |
| `Web` | Checks a URL's HTTP status and downloads reference photos. | `WebClient` (providers), fakes in tests |
| `SiteFetch` | Reads one website for `init`: same-site check, page fetch, sitemap, logo download. | `SiteClient` (providers), fakes in tests |
| `ImageModel` | Draws one picture from a prompt, a ratio and an optional reference photo. | `GeminiImageModel` and `OpenAiImageModel` (providers), `SolidImageModel` (core, tests and the hidden `solid` provider) |
| `ToolHost` | Lists tool specs and executes tool calls for one mission. | `MissionTools` (core) |

`CliDriver` runs a coding agent for a mission. A `CliAgent` implementation (`Claude`, `Codex`, `Gemini`) says how to build the command line and config files, and how to read the CLI's JSON stream. The driver starts a `McpEndpoint` for the mission (HTTP on `127.0.0.1`, random port, bearer token), writes the config files with mode `0600` into a temporary directory, runs the CLI and deletes the directory when the mission ends. See [Agent CLIs](../guides/agent-clis.md).

`ChatError` is `Transient` (429, 408, 5xx, network) or `Fatal`.

`ScriptedDriver` plays a recorded JSON script, `{"missions": {"plan": [...], "campaign:<slug>": [...]}}`, through the same loop. The hidden `replay` provider uses it. Tests use it so no test calls a real model. It is not a user feature.

## Mission flow

`mads init` runs `init::run_init`. It starts one `init` mission with `InitTools`, retries it like any mission, and writes `business.toml`, `catalog.csv` and `research.md` only when the mission finishes. The mission asks for web search only when the driver says it can give it (`Driver::web_search`), and the agent is told which case it is in. It creates no run directory. The transcript goes to `<out-dir>/.mads/transcripts/`.

`run::generate` does this.

1. Create or open the run directory. Load `workspace.json` if it exists, else start a new workspace from the input.
2. Emit `run_started`.
3. Run the plan mission, unless it is `Finished` and the plan has campaigns. If the plan fails, the run fails, because campaign missions depend on it.
4. Run one campaign mission per campaign that is not `Finished`, at most `--parallel` at a time. A failed campaign does not stop the others.
5. If every mission finished, add cross negatives and run `finalize`. Otherwise exit code `1`.
6. `finalize` validates the account, checks URLs (unless skipped) and exports the CSVs. Errors give exit code `3`.
7. Write the CSVs into `google-ads/`, `report.md` and `run.json`. Emit `run_finished`. On any exit code other than `0`, stale CSVs in `google-ads/` are removed first. Every model message or CLI stream line is appended to `transcripts/`.

Each mission attempt does this.

1. Build `MissionTools` for the mission kind. A campaign mission can only read and change its own campaign.
2. Run the driver under `--mission-timeout`.
3. Add the usage to the mission state and save the workspace.
4. On `Finished`, mark it so. On failure, try again with a fresh context while attempts are below `1 + --mission-retries`.

`LoopDriver` loop.

```text
messages = [User(mission.user)]
for turn in 1..=max_turns:
    response = complete(...)            retry transient errors, 3 attempts, waits 1 s and 2 s
    emit usage, add to the run budget, stop when the token budget is exceeded
    emit agent_text when the model sent text
    if no tool calls:
        if tools.finished(): done
        nudge (at most 2 times), else fail "no progress"
    else:
        run the tool calls in order, emit tool_called and tool_finished
        if tools.finished(): done
fail "max turns"
```

Token budget is checked after each response, across all missions.

## Tool contract

```rust
pub struct ToolSpec { pub name: String, pub description: String, pub input_schema: serde_json::Value }

#[async_trait]
pub trait ToolHost: Send + Sync {
    fn specs(&self) -> Vec<ToolSpec>;
    async fn call(&self, name: &str, args: serde_json::Value) -> ToolOutput;
    fn finished(&self) -> bool;
}
```

Rules every tool follows.

- Arguments deserialize into a typed struct with `#[serde(deny_unknown_fields)]` and `#[derive(JsonSchema)]`.
- Schemas are portable across providers. They use `object`, `string` (with optional `enum`), `number`, `integer`, `boolean` and `array`. They never use `$ref`, `$defs`, `oneOf`, `anyOf`, `allOf` or nullable types. A unit test walks every schema.
- Output is JSON. Success is `{"ok": true, "result": {...}, "warnings": [...]}`. Failure is `{"ok": false, "errors": [{"code", "severity", "path", "message"}], "warnings": [...]}`.
- Bad arguments give code `ARGS`. A tool that is not in the mission's toolset gives `UNKNOWN_TOOL`. Past the call budget (`max_turns * 4`) every call gives `LIMIT`.
- A failed call never changes the workspace.
- A successful write saves `workspace.json`.

The tools, including the init tools, are listed in [MCP tools](mcp-tools.md).

## Workspace

`Workspace` holds the input, the account and the state of each mission. It lives behind `Arc<tokio::sync::Mutex<Workspace>>` and saves with write-then-rename. The shape of the file is in [Output files](output-files.md#workspacejson).

## Events

`EventSink` wraps an unbounded channel sender. It stamps each `Event` with a sequence number and an RFC 3339 timestamp and never fails the run when the receiver is gone. The CLI consumes the channel, appends every event to `events.ndjson` and feeds the renderer. See [Output files](output-files.md#eventsndjson).

## Embed mads-core

You can run the whole pipeline from your own Rust program. This example compiles against the workspace crates.

```toml
[dependencies]
mads-core = { git = "https://github.com/avelino/mads" }
mads-providers = { git = "https://github.com/avelino/mads" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1"
anyhow = "1"
```

```rust
use std::{path::Path, sync::Arc};

use mads_core::{
    events::EventSink,
    input::load_input,
    run::{RunConfig, generate},
};
use mads_providers::{ProviderSelection, WebClient, build_drivers};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let input = load_input(Path::new("business.toml"))?;

    let drivers = build_drivers(&ProviderSelection {
        provider: "anthropic".into(),
        model: Some("<model-id>".into()),
        ..Default::default()
    })?;

    let (events, mut rx) = EventSink::channel();
    let printer = tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
        }
    });

    let mut cfg = RunConfig::new("out".into());
    cfg.provider = "anthropic".into();
    cfg.model = Some("<model-id>".into());

    let web = Arc::new(WebClient::new()?);
    let result = generate(input, drivers, web, cfg, events).await?;
    printer.await?;

    println!("{} {}", result.run_dir.display(), result.exit_code);
    Ok(())
}
```

Notes.

- `generate` takes the `EventSink` by value. The receiver ends when the run is done, so awaiting the printer task returns.
- `RunResult` has `run_dir`, `exit_code` (`0`, `1` or `3`) and `totals`.
- `mads_core::run::export_run` re-exports a finished run. `mads_core::run::Drivers` holds the plan driver and the campaign driver.
- To use your own model, implement `ChatModel` and wrap it with `LoopDriver::new(Arc::new(model))`. To use your own transport for URL checks, implement `Web`.
- The library does not load keys or print anything. That is the CLI's job.
