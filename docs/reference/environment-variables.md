# Environment variables

Look up every environment variable mads reads, what it does and what it defaults to.

## Settings

Each one is the environment form of a flag. A flag wins over its variable.

| Variable | Flag | Description |
|---|---|---|
| `MADS_FORMAT` | `--format` | `auto`, `pretty`, `plain`, `json` or `github`. Default `auto`. |
| `MADS_PROVIDER` | `--provider` | Provider name. |
| `MADS_MODEL` | `--model` | Model id. |
| `MADS_PLAN_MODEL` | `--plan-model` | Model for the plan mission only. |
| `MADS_BASE_URL` | `--base-url` | Endpoint for `openai-compat`. Required there. |
| `MADS_IMAGE_PROVIDER` | `--image-provider` | `auto`, `gemini`, `openai` or `none`. Default `auto`. |
| `MADS_IMAGE_MODEL` | `--image-model` | Image model name. |

mads has no variable for the turn, timeout, token or retry limits. Use the flags in [CLI](cli.md).

## Variables without a flag

| Variable | Description |
|---|---|
| `MADS_API_KEY` | Key for `openai-compat`. Optional. An empty value counts as unset. |
| `MADS_CLAUDE_BIN` | Path of the `claude` executable. Default `claude`. |
| `MADS_CODEX_BIN` | Path of the `codex` executable. Default `codex`. |
| `MADS_GEMINI_BIN` | Path of the `gemini` executable. Default `gemini`. |
| `MADS_GEMINI_BASE_URL` | Endpoint of the Gemini image API. Default `https://generativelanguage.googleapis.com`. |
| `MADS_OPENAI_BASE_URL` | Endpoint of the OpenAI image API. Default `https://api.openai.com`. |
| `MADS_ALLOW_PRIVATE_HOSTS` | Any non-empty value lets `mads init` read loopback and private hosts. For local development. |

An empty value is ignored for the three `MADS_*_BIN` variables and for both image base URLs. `mads providers` also honors the `MADS_*_BIN` overrides when it looks for an agent CLI.

## Provider keys

An empty value counts as missing in `mads providers`.

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
| `claude-cli`, `codex-cli`, `gemini-cli` | none, the CLI does its own login |

The image providers read the same keys. `gemini` reads `GEMINI_API_KEY` and `openai` reads `OPENAI_API_KEY`. With `--image-provider auto`, mads picks `gemini` when `GEMINI_API_KEY` is set, then `openai` when `OPENAI_API_KEY` is set, and none when neither is. See [Providers](providers.md).

## Agent CLIs

| Variable | Description |
|---|---|
| `ANTHROPIC_API_KEY` | When set and not empty, mads starts `claude` with `--bare`. |
| `CLAUDE_CODE_OAUTH_TOKEN` | Login token for `claude` in CI. mads does not read it. The CLI does. |

mads sets these on the agent CLI process it starts. You do not set them.

| Variable | Set for | Value |
|---|---|---|
| `ENABLE_TOOL_SEARCH` | `claude` | `false` |
| `DISABLE_AUTOUPDATER` | `claude` | `1` |
| `MADS_MCP_TOKEN` | `codex` | Bearer token of the per-mission MCP server. |
| `GEMINI_CLI_SYSTEM_SETTINGS_PATH` | `gemini` | Settings file mads writes for the mission. |
| `GEMINI_SYSTEM_MD` | `gemini` | System prompt file mads writes for the mission. |

See [Agent CLIs](../explanation/agent-clis.md) for why.

## Logging

| Variable | Description |
|---|---|
| `RUST_LOG` | A `tracing` filter such as `debug` or `mads_core=debug`. When set and valid, it wins over `-v` and `-vv`. Without it, the level is `warn`, `info` for `-v` and `debug` for `-vv`. |

## GitHub Actions

| Variable | Description |
|---|---|
| `GITHUB_ACTIONS` | When exactly `true`, `--format auto` picks `github`. |
| `GITHUB_STEP_SUMMARY` | With the `github` format, mads appends `report.md` to this file when the run finishes. |

See [Run in GitHub Actions](../how-to/run-in-github-actions.md).

## Next

- [CLI](cli.md)
- [Providers](providers.md)
- [Choose a provider](../how-to/choose-a-provider.md)
