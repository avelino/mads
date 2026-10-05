# mads

Rust CLI that turns a business description into Google Ads bulk-upload CSVs, with LLM agents deciding and deterministic code enforcing Google's limits. Crates: `mads-core` (library, no network), `mads-providers` (LLM and HTTP adapters), `mads-cli` (the `mads` binary). Design spec: `docs/superpowers/specs/2026-10-01-mads-v1-design.md`. Section 16 lists where the code differs from it: the code wins.

## Commands

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace   # the gate
cargo build -p mads-cli && target/debug/mads --help
MADS_IT=1 cargo test -p mads-providers --test it_claude -- --ignored --nocapture   # real claude, spends tokens
```

Run the whole flow offline with the hidden `replay` provider: see `crates/mads-cli/tests/e2e.rs` for scripts. Report "done" only with the gate at exit 0.

## Architecture

- `mads-core` never touches the network. It defines the traits (`ChatModel`, `Driver`, `Web`, `SiteFetch`, `ToolHost`) and `mads-providers` implements them. `mads-cli` depends on both.
- A run is missions: `plan`, then one `campaign:<slug>` per campaign in parallel, then deterministic post-processing (cross negatives, validate, URL check, export). `mads init` is a separate single mission. `mads optimize` prepares a new run from a finished one plus Google Ads report CSVs (`src/perf`, `workspace.live`) and runs the same missions.
- Agents work only through typed tools (`crates/mads-core/src/tools`, `src/init/tools.rs`). API providers loop over `ChatModel`. Agent CLIs reach the same tools through a per-mission MCP server (`mads-providers/src/mcp.rs`).
- Output per ad platform: `out/<run>/google-ads/` (five CSVs), plus `report.md`, `workspace.json`, `events.ndjson`, `transcripts/`.

## Invariants

- `data/` is the golden reference. `crates/mads-core/tests/golden.rs` must keep exporting files 1 to 5 byte for byte.
- A tool validates before it writes and never panics. A failed call changes nothing and returns `ok:false` with issues.
- Tool schemas stay portable: no `$ref`, `anyOf`, `oneOf` or nullable types (a test enforces it).
- No `unwrap` outside tests. Tests come first and must fail for the right reason before the code exists.
- Money is `Cents` (integer). Keyword text and limits compare through `normalize` and `char_len`.

## Looks like a bug, is a decision

- Only `manual_cpc` bidding is accepted. Maximize clicks and conversions return `UNSUPPORTED`.
- Campaign negatives are expanded into every ad group row of `4-negative-keywords.csv`.
- Sitelinks, callouts and snippets are validated but not exported. M11 waits for real Google templates in `data/templates/`.

## Gotchas

- Real runs cost the user's tokens. Never run `claude`, `codex` or `gemini` for real without saying so first. An unisolated `claude -p` loaded 157k tokens (about US$0.63) for one word. The driver's isolation flags keep it near 500 tokens: do not remove them.
- Claude Code needs `ttlMs` and `cacheScope` in the MCP `tools/list` result, and `ENABLE_TOOL_SEARCH=false`. Without them the server shows `connected` and the model sees no tool. `claude --debug-file f` shows why.
- `cargo fmt` reorders imports (edition 2024). Do not patch formatted code with `str.replace`. In Python, `open(p, "w").write(open(p).read())` truncates the file.
- `export` and `--resume` read `workspace.json`, not `input/`.
- Do not commit or push. The user does it.

## Changing things

- New provider, validation rule or tool: follow "Add ..." in `CONTRIBUTING.md` and update the matching pages under `docs/`.
- Prompts live in `crates/mads-core/prompts/*.md`. `mission.rs` tests assert the playbook rules: keep them in sync.
- Docs and README: see `.claude/rules/docs.md`.
- CI is one workflow, `.github/workflows/ci.yml` (path filter, gate once, image after the gate). Do not add a second workflow that compiles the workspace again. The image is built by the `Dockerfile`; keep `docs/guides/docker.md` in sync.
