# Contributing to mads

This page shows you how to build mads, run the verification gate and add a provider, a validation rule or a tool.

## Build

You need `rustup`. The repository pins the toolchain in `rust-toolchain.toml` (channel `1.98`, with `rustfmt` and `clippy`), so the first `cargo` call installs it.

```bash
git clone https://github.com/avelino/mads
cd mads
cargo build
target/debug/mads --help
```

## The gate

Run this before you call a change done. It is the same gate every task in the design spec ends on.

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

Clippy runs with `unwrap_used = "warn"` for the workspace, and `-D warnings` turns that into an error. Tests are exempt through `clippy.toml` (`allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`). Integration test files in `tests/` add `#![allow(clippy::unwrap_used)]` at the top.

## Repository layout

```text
crates/
  mads-core/         library, no network IO
    prompts/         system prompts embedded with include_str!
    src/input/       business.toml, catalog.csv, slugify, allowed URLs
    src/google/      domain model, rules, keyword expansion, RSA merge, CSV export
    src/init/        the init mission: tools, draft state, run
    src/tools/       typed tools, JSON schemas, MissionTools (the ToolHost)
    src/agent/       ChatModel and Driver ports, LoopDriver, scripted driver
    src/run.rs       orchestration, limits, resume, export
    src/post.rs      cross negatives, URL check
    src/finalize.rs  validate, URL check, export
    src/report.rs    report.md
    tests/           golden test against data/
  mads-providers/    GenaiChatModel, provider selection, WebClient, SiteClient (init)
    src/cli/         CliDriver and the claude, codex and gemini agents
    src/mcp.rs       the per-mission MCP server for agent CLIs
  mads-cli/          the mads binary, renderers, exit codes
data/                real example output, the golden reference for the export format
docs/                the documentation (GitBook)
docs/superpowers/    design specs
```

The dependency rule is strict. `mads-cli` depends on `mads-providers` and `mads-core`. `mads-providers` depends on `mads-core`. `mads-core` never touches the network. It defines traits (`ChatModel`, `Driver`, `Web`, `SiteFetch`, `ToolHost`) and `mads-providers` implements them. See [Architecture](docs/reference/architecture.md).

## Add an API provider

API providers go through the `genai` crate.

1. Add a row to `API_PROVIDERS` in `crates/mads-providers/src/genai_model.rs`. The row holds the mads name, the genai namespace and the API key variable.
2. Add a `Spec` to `SPECS` in `crates/mads-providers/src/select.rs` with `api(name, Some("ENV_VAR"))`.
3. Add the name to the lists in the tests in `select.rs` (`api_providers_build_with_a_model_and_no_network` and `listing_has_the_documented_providers_and_hides_replay`).
4. Document it in `docs/guides/providers.md`, `docs/reference/environment-variables.md` and the table in `README.md`.

An agent CLI provider is a different job. Implement `CliAgent` in `crates/mads-providers/src/cli/<name>.rs` (command line, config files, stream parser, executable override variable). Then add it to `SPECS`, `build_drivers` and `preflight` in `select.rs`, and document it in `docs/guides/agent-clis.md`. Test the stream parser against recorded fixtures.

## Add a validation rule

Rules live in `crates/mads-core/src/google/rules.rs` (account, campaign, ad group, text) and `crates/mads-core/src/post.rs` (URL check).

1. Pick the next free code. Errors are `E##` and block export. Warnings are `W##` and go to the report.
2. Add the check to the method that owns the path it reports on. Every issue carries a `code`, a `path` and a `message` that names the limit and the actual value.
3. Write table-driven tests with a passing case and a failing case.
4. Update `docs/reference/validation-rules.md`.

Tools reuse these rules. If the rule belongs to an agent-facing tool, make sure the tool filters the issue by path so the agent only sees what its call changed.

## Add a tool

Tools are in `crates/mads-core/src/tools/plan.rs` (plan mission) and `campaign.rs` (campaign missions).

1. Define an arguments struct with `#[serde(deny_unknown_fields)]` and `#[derive(JsonSchema)]`. Optional fields use `#[serde(default)]`, never `Option<T>`.
2. Register it in `specs()` with a name, a description and `schema_for::<Args>()`.
3. Handle it in `call()`. Parse with `parse_args`, validate before you mutate, and save the workspace with `t.persist(ws)` after a successful write. A failed call must not change the workspace.
4. Return `ToolOutput::ok` or `ToolOutput::fail_issues` with a short summary line. The summary shows in progress output.
5. Mention the tool in the matching prompt under `crates/mads-core/prompts/`.
6. Update the toolset tests in `tools/tests.rs`. `every_tool_schema_is_portable_across_providers` fails if your schema uses `$ref`, `oneOf`, `anyOf`, `allOf` or nullable types.
7. Update `docs/reference/mcp-tools.md`.

## Test conventions

- Unit tests sit next to the code in `#[cfg(test)] mod tests`.
- Input tests cover valid and invalid files: unknown key, missing field, bad URL, slug collisions, BOM, CRLF.
- Rule tests are table-driven, one table per code.
- The export has a golden test (`crates/mads-core/tests/golden.rs`). Files 1 to 5 must stay byte-identical to `data/`.
- Tool tests send JSON the way an LLM would. They cover success, every reachable error code and unknown fields.
- Loop and run tests use `ScriptedChatModel`. No test calls a real model.
- HTTP code is tested with `wiremock`.
- CLI end-to-end tests use `assert_cmd` with the hidden `replay` provider (`crates/mads-cli/tests/e2e.rs`). They check run directory contents, exit codes 0, 1, 2 and 3, and valid NDJSON with `--format json`.

## Docs

Docs live in `docs/` and publish through GitBook (`.gitbook.yaml`). Each page starts with an H1 and one sentence that says what the reader can do afterwards. Every command in a page must run against the current binary. Do not edit `docs/superpowers/` unless you are changing a design spec.

## Commits

Keep changes small and focused. Run the gate first.
