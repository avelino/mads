# Providers

Look up every provider mads accepts, the variable each one reads, what each agent CLI is run with, and the errors provider selection returns.

For when to pick which, see [Choose a provider](../how-to/choose-a-provider.md).

## Provider list

`mads providers` prints these twelve rows, in this order. The command always exits `0`.

| Provider | Kind | Key variable | Notes |
|---|---|---|---|
| `anthropic` | api | `ANTHROPIC_API_KEY` | |
| `openai` | api | `OPENAI_API_KEY` | |
| `gemini` | api | `GEMINI_API_KEY` | |
| `openrouter` | api | `OPENROUTER_API_KEY` | mads reads this variable itself. |
| `groq` | api | `GROQ_API_KEY` | |
| `deepseek` | api | `DEEPSEEK_API_KEY` | |
| `xai` | api | `XAI_API_KEY` | |
| `ollama` | api | none | No key needed. |
| `openai-compat` | api | `MADS_API_KEY`, optional | Needs `--base-url`. |
| `claude-cli` | cli | none | Runs `claude`. |
| `codex-cli` | cli | none | Runs `codex`. |
| `gemini-cli` | cli | none | Runs `gemini`. |

A provider named `replay` exists for tests. It is hidden from the list.

### Status values

| Status | Meaning |
|---|---|
| `ready` | The key variable is set and not empty, or the provider needs none, or the CLI executable was found. |
| `missing <VAR>` | An API provider whose key variable is unset or empty. |
| `` `<bin>` not found in PATH `` | An agent CLI whose executable was not found. |
| `not available in this build` | The provider is listed but not built in. No provider has this status today. |

`ollama` and `openai-compat` are `ready` without a check that a server is listening. For an agent CLI, `mads providers` looks for the executable the same way `generate` does. It uses the `MADS_<NAME>_BIN` variable first, then `PATH`. The variable can hold a path or a name found in `PATH`.

A second table lists the image providers.

| Image provider | Key variable |
|---|---|
| `gemini` | `GEMINI_API_KEY` |
| `openai` | `OPENAI_API_KEY` |
| `none` | none |

## Flags and variables

| Flag | Variable | Applies to |
|---|---|---|
| `--provider` | `MADS_PROVIDER` | All. Required. |
| `--model` | `MADS_MODEL` | Required for API providers. Optional for agent CLIs. |
| `--plan-model` | `MADS_PLAN_MODEL` | The plan mission only. Defaults to `--model`. |
| `--base-url` | `MADS_BASE_URL` | `openai-compat`. Required there. Ignored by the others. |
| none | `MADS_API_KEY` | `openai-compat`. Sent as the key. Without it mads sends the placeholder `none`. |
| none | `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN`, `MADS_GEMINI_BIN` | Path of the executable. |

A flag overrides its variable. The full list of variables is in [Environment variables](environment-variables.md). The agent flags are in [CLI](cli.md).

## What an API provider call does

| Item | Value |
|---|---|
| Output tokens requested per response | 8192 |
| Tools | Every mission sends its tool definitions and expects tool calls back. |
| Nudge text | `Continue using the tools. Call finish when the work is done.` |
| Nudges before failure | 2 in a row. A third response in a row with no tool call fails the mission with `no progress`. A tool call resets the count. |
| Retried errors | HTTP 408, 429, 5xx and network failures. |
| Attempts | 3 per model call. |
| Wait between attempts | 1 second, then 2 seconds. |
| Other errors | Final for that call. The mission fails with the error text, cut at 600 characters. |
| Usage | Input and output tokens. No cost. The report shows cost as `n/a`. |

`--max-turns` caps the turns of a mission and a mission that runs out fails with `max turns`. Mission retries and the token budget are in [Limits](limits.md).

## Agent CLIs

Every agent CLI mission gets its own local MCP server and a private temporary directory. The reasons are in [Agent CLIs](../explanation/agent-clis.md).

| Provider | Executable | Override | Minimum version |
|---|---|---|---|
| `claude-cli` | `claude` | `MADS_CLAUDE_BIN` | 2.1.259 |
| `codex-cli` | `codex` | `MADS_CODEX_BIN` | not checked |
| `gemini-cli` | `gemini` | `MADS_GEMINI_BIN` | not checked |

Before a run starts, `generate`, `optimize` and `init` run `<executable> --version` and compare it with the minimum.

### claude-cli flags

| Flag | Value |
|---|---|
| `-p` | The mission prompt. |
| `--system-prompt-file` | `system.md` in the temporary directory. |
| `--mcp-config` | `mcp.json` in the temporary directory. |
| `--strict-mcp-config` | Always. |
| `--setting-sources` | `project` |
| `--disable-slash-commands` | Always. |
| `--tools` | Empty. `WebSearch,WebFetch` in `init` with web search on. |
| `--allowedTools` | `mcp__mads__*`. `mcp__mads__* WebSearch WebFetch` in `init` with web search on. |
| `--permission-prompts` | `none` |
| `--output-format` | `stream-json` |
| `--verbose` | Always. |
| `--max-turns` | From `--max-turns`. |
| `--no-session-persistence` | Always. |
| `--bare` | Only when `ANTHROPIC_API_KEY` is set and not empty. |
| `--model` | Only when you pass `--model`. |

Environment set for the process.

| Variable | Value |
|---|---|
| `ENABLE_TOOL_SEARCH` | `false` |
| `DISABLE_AUTOUPDATER` | `1` |

### codex-cli flags

The command is `codex exec` with these options.

| Option | Value |
|---|---|
| `--json`, `--ephemeral`, `--skip-git-repo-check`, `--ignore-user-config` | Always. |
| `--sandbox` | `read-only` |
| `-c model_instructions_file` | `system.md` in the temporary directory. |
| `-c mcp_servers.mads.url` | The local MCP server. |
| `-c mcp_servers.mads.bearer_token_env_var` | `MADS_MCP_TOKEN` |
| `-c mcp_servers.mads.required` | `true` |
| `-c features.shell_tool` | `false` |
| `-c features.multi_agent` | `false` |
| `-c web_search` | `disabled`. `live` in `init` with web search on. |
| `--model` | Only when you pass `--model`. |

The bearer token is set in the environment variable `MADS_MCP_TOKEN`. `codex exec` has no turn flag. For every provider, a mission gets `--max-turns` times 4 tool calls, and a call past that returns `LIMIT`.

### gemini-cli flags

| Item | Value |
|---|---|
| `-p` | The mission prompt. |
| `--output-format` | `stream-json` |
| `--approval-mode` | `yolo` |
| `--allowed-mcp-server-names` | `mads` |
| `--model` | Only when you pass `--model`. |
| `GEMINI_CLI_SYSTEM_SETTINGS_PATH` | `settings.json` in the temporary directory. |
| `GEMINI_SYSTEM_MD` | `system.md` in the temporary directory. |
| Settings `tools.core` | Empty. `google_web_search` and `web_fetch` in `init` with web search on. |
| Settings `model.maxSessionTurns` | From `--max-turns`. |

### Web search

Only the `init` mission opens web search. `--no-web-search` on `mads init` turns it off. `generate` missions never search.

| CLI | What changes in `init` |
|---|---|
| `claude-cli` | `--tools` and `--allowedTools` add `WebSearch` and `WebFetch`. |
| `codex-cli` | `web_search` is `live`. |
| `gemini-cli` | `tools.core` lists `google_web_search` and `web_fetch`. |

### Usage reported

| CLI | Tokens | Cost |
|---|---|---|
| `claude-cli` | Input includes cache creation and cache reads. | USD, from the CLI. |
| `codex-cli` | Per turn. | `n/a` |
| `gemini-cli` | From the result event, when it carries them. | `n/a` |

The raw stream of each mission is saved as `transcripts/<mission>.cli.jsonl` in the run directory.

## Errors

Selection errors exit with code `2`, before any mission starts.

| Message | Cause |
|---|---|
| `no provider: use --provider or MADS_PROVIDER` | No provider given. |
| ``unknown provider '<name>'; run `mads providers` to see the options`` | The name is not in the list. |
| `provider '<name>' is not available in this build yet` | The name is listed but not built in. |
| `--model (or MADS_MODEL) is required for <provider>` | An API provider with no model. |
| `--base-url (or MADS_BASE_URL) is required for openai-compat` | `openai-compat` with no endpoint. |
| `<name> not found (<program>): install it or point the matching MADS_*_BIN variable at it` | The agent CLI executable is missing. |
| `<name> <found> is too old: mads needs <min> or newer` | The agent CLI is older than the minimum. |
| `cannot read the <name> version from: <output>` | The `--version` output has no `x.y.z`. |

A missing API key is not a selection error. The plan mission fails with `ApiKeyEnvNotFound { env_name: "<VAR>" }` and the run exits with code `1`. Exit codes are in [Exit codes and errors](exit-codes-and-errors.md).

## Next

- [Choose a provider](../how-to/choose-a-provider.md)
- [Agent CLIs](../explanation/agent-clis.md)
- [Limits](limits.md)
