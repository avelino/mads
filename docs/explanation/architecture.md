# Architecture

Find where a behavior lives in the three crates of mads, and embed the library in your own program.

## Three crates, one direction

```text
mads-cli  ───►  mads-providers  ───►  mads-core
(binary)        (network, models)     (rules, state, no network)
```

`mads-cli` depends on both other crates. `mads-providers` depends on `mads-core`. `mads-core` depends on neither.

The split exists for one reason. Google's limits and the account state must be testable without a network and without a model. So `mads-core` never opens a socket. It defines traits for everything that needs the outside world, and the other two crates implement them. Tests plug in fakes and run the full flow offline.

`mads-core` does touch the disk. It writes `workspace.json`, creates run directories and writes the CSVs and `report.md`. The rule is "no network", not "no IO".

## What each crate owns

**mads-core** holds everything that decides or enforces.

- `input` parses and validates `business.toml` and the catalog.
- `google` is the account model, the validation rules, keyword expansion, RSA merging and the CSV exports.
- `workspace` is the account plus the state of every mission.
- `tools` are the typed tools the agents call. `mission` builds their prompts. `init` is the same pair for `mads init`.
- `agent` has the loop that drives a model through a mission.
- `run`, `post` and `finalize` orchestrate a run, add cross negatives, check URLs and export.
- `images` draws and validates pictures for image campaigns.
- `perf` and `optimize` read Google Ads report CSVs and prepare a new run from a finished one.
- `events` and `report` give you the event stream and `report.md`.
- `money` (integer cents), `usage` (token counts) and `web` (the URL-check trait) are small shared types.

**mads-providers** holds everything that talks to something.

- `GenaiChatModel` reaches every API provider.
- `CliDriver` runs the claude, codex and gemini CLIs.
- The per-mission MCP server lets those CLIs call the tools.
- `WebClient` checks URLs and downloads reference photos. `SiteClient` reads a website for `init`.
- Image models for Gemini and OpenAI, plus provider selection.

**mads-cli** is the `mads` binary. It parses arguments, renders progress, appends every event to `events.ndjson` and turns the result into an exit code. It decides nothing about Google Ads.

## Ports

Each trait in `mads-core` is a seam where the real world plugs in.

| Port | What it hides | Real implementation | Offline stand-in |
|---|---|---|---|
| `ChatModel` | One LLM turn | `GenaiChatModel` | `ScriptedChatModel` |
| `Driver` | A whole mission against a `ToolHost` | `CliDriver`, or `LoopDriver` over a `ChatModel` | `ScriptedDriver` |
| `ToolHost` | The tools one mission may call | `MissionTools` | none needed |
| `Web` | URL status and photo downloads | `WebClient` | fakes in tests |
| `SiteFetch` | Reading a site for `init` | `SiteClient` | fakes in tests |
| `ImageModel` | Drawing one picture | Gemini and OpenAI models | `SolidImageModel` |

The `Driver` port is what lets an API model and a coding-agent CLI do the same job. `LoopDriver` calls the model, runs the tool calls it asks for and repeats. `CliDriver` hands the whole mission to a CLI and exposes the same tools over MCP. Both end at the same `ToolHost`, so the rules apply the same way. See [Agent CLIs](agent-clis.md) for the CLI side.

`ChatError` tells the loop whether a failure is worth retrying. A rate limit, a timeout or a 5xx is `Transient`. Anything else is `Fatal`. The retry numbers are in [Limits and retries](limits-and-retries.md).

Two hidden providers wire the offline stand-ins in. `replay` plays a recorded script through `ScriptedDriver`. `solid` is an image provider that draws flat color pictures. They exist for tests and for running the flow without spending tokens.

## Tools are the only way in

An agent never writes the account directly. It calls a tool, and the tool validates before it writes.

- Arguments deserialize into a typed struct that rejects unknown fields. A bad call returns `ARGS`.
- A tool outside the mission's set returns `UNKNOWN_TOOL`. A campaign mission can only read and change its own campaign.
- Past `max_turns * 4` calls in one mission, every call returns `LIMIT`.
- A failed call changes nothing. A successful write saves the workspace.
- Tool schemas stay portable across providers, with no `$ref`, `anyOf`, `oneOf` or nullable types. A unit test walks every schema.

This is why a confused model cannot corrupt the export. The worst it can do is get `ok:false` with a list of issues and try again. The tool list is in [MCP tools](../reference/mcp-tools.md).

## State and events

`Workspace` holds the input, the account and the state of each mission. It sits behind a shared async mutex, so campaign missions running in parallel write one after another. It saves to a temporary file and renames it, so a crash never leaves half a file. `export` and `--resume` read `workspace.json`, not `input/`. The file shape is in [Output files](../reference/output-files.md).

`EventSink` is a cloneable handle on an unbounded channel. It stamps each event with a sequence number and a UTC timestamp. If nothing listens, sending is a no-op and the run goes on. The CLI listens, writes `events.ndjson` and drives the renderer. A library user can listen with any code they like.

## Where a run goes through these pieces

A run is missions. `run::generate` runs the plan mission, then one campaign mission per campaign, then the deterministic steps in `post` and `finalize`. `mads optimize` calls `optimize::prepare` to build a new run directory from a finished run and its reports, then calls the same `generate`. `mads init` runs a single mission with its own tools and creates no run directory. The user-level story is in [How a run works](how-a-run-works.md) and [How optimize decides](how-optimize-decides.md).

## Embedding mads-core

You can run the pipeline from your own Rust program. The pieces are small because the CLI is thin.

- `mads_core::input::load_input` reads the business file.
- `mads_providers::build_drivers` takes a `ProviderSelection` and returns the `Drivers` (plan, campaign and optional image model).
- `mads_core::run::generate` takes the input, the drivers, a `Web`, a `RunConfig` and an `EventSink`, and returns a `RunResult` with the run directory, exit code and totals.
- `mads_core::run::export_run` exports a finished run again without a model.

To use your own model, implement `ChatModel` and wrap it in `LoopDriver`. To use your own transport for URL checks, implement `Web`. The library loads no keys and prints nothing. Both are the CLI's job.

## Next

- [How a run works](how-a-run-works.md)
- [MCP tools](../reference/mcp-tools.md)
- [Agent CLIs](agent-clis.md)
