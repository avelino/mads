# Environment variables

This page lists every environment variable mads reads.

## mads settings

Each one is the environment form of a flag. A flag wins over its variable.

| Variable | Flag | Description |
|---|---|---|
| `MADS_PROVIDER` | `--provider` | Provider name. |
| `MADS_MODEL` | `--model` | Model id. |
| `MADS_PLAN_MODEL` | `--plan-model` | Model for the plan mission. |
| `MADS_BASE_URL` | `--base-url` | Endpoint for `openai-compat`. |
| `MADS_FORMAT` | `--format` | `auto`, `pretty`, `plain`, `json` or `github`. |
| `MADS_API_KEY` | none | Key for `openai-compat`. Optional. |
| `MADS_CLAUDE_BIN` | none | Path of the `claude` executable. Default `claude`. |
| `MADS_CODEX_BIN` | none | Path of the `codex` executable. Default `codex`. |
| `MADS_GEMINI_BIN` | none | Path of the `gemini` executable. Default `gemini`. |
| `MADS_ALLOW_PRIVATE_HOSTS` | none | Any non-empty value lets `mads init` read loopback and private hosts. Local development only. |

mads has no variable for the turn, timeout, token or retry limits. Use the flags.

## Provider keys

mads reads the key from the provider's standard variable. An empty value counts as missing in `mads providers`.

| Provider | Variable |
|---|---|
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai` | `OPENAI_API_KEY` |
| `gemini` | `GEMINI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `groq` | `GROQ_API_KEY` |
| `deepseek` | `DEEPSEEK_API_KEY` |
| `xai` | `XAI_API_KEY` |
| `ollama` | none |
| `openai-compat` | `MADS_API_KEY` (optional) |

## Agent CLIs

| Variable | Description |
|---|---|
| `CLAUDE_CODE_OAUTH_TOKEN` | Login token for `claude` in CI. Create it with `claude setup-token`. mads does not read it. The CLI does. |
| `ANTHROPIC_API_KEY` | Also read by `claude`. When set, mads runs claude in bare mode. |

mads sets a few variables for the CLI process itself. `ENABLE_TOOL_SEARCH=false` for claude, `MADS_MCP_TOKEN` for codex, and `GEMINI_CLI_SYSTEM_SETTINGS_PATH` and `GEMINI_SYSTEM_MD` for gemini. You do not set them. See [Agent CLIs](../guides/agent-clis.md).

## Logging

| Variable | Description |
|---|---|
| `RUST_LOG` | A `tracing` filter such as `debug` or `mads_core=debug`. Overrides `-v` and `-vv`. |

## GitHub Actions

| Variable | Description |
|---|---|
| `GITHUB_ACTIONS` | When `true`, `--format auto` picks `github`. |
| `GITHUB_STEP_SUMMARY` | With the `github` format, mads appends `report.md` to this file when the run finishes. |
