# Agent CLIs

This page shows how to run mads with a coding agent you already have installed (`claude`, `codex` or `gemini`) instead of an API key.

## How it works

For every mission, mads does this.

1. Starts a local MCP server on `127.0.0.1` with a random port. One server per mission. It exposes only that mission's tools, the same ones API providers call. See [MCP tools](../reference/mcp-tools.md).
2. Creates a bearer token of 64 hex characters. Every request to the server needs it.
3. Writes the CLI's config files (system prompt and MCP settings) with mode `0600` into a fresh temporary directory in the OS temp folder.
4. Runs the CLI in that directory with the mads server as its only MCP server and its built-in tools turned off.
5. Reads the CLI's JSON stream. Text becomes `agent_text` events and usage becomes `usage` events. Tool call events come from the MCP server, so progress looks the same as with an API provider.
6. When the mission ends, stops the server and deletes the temporary directory.

The config files live outside the run directory. Tokens never end up in an artifact you upload from CI. The raw stream of each mission is saved in `transcripts/<mission>.cli.jsonl`.

A mission finishes when the agent calls `finish` and the tool accepts it. If the CLI exits before that, the mission fails and the reason has the exit status and the tail of the CLI's stderr.

## Setup

| Provider | CLI | Override the executable | Login |
|---|---|---|---|
| `claude-cli` | `claude` | `MADS_CLAUDE_BIN` | `claude` login, `ANTHROPIC_API_KEY`, or `CLAUDE_CODE_OAUTH_TOKEN` |
| `codex-cli` | `codex` | `MADS_CODEX_BIN` | `codex` login |
| `gemini-cli` | `gemini` | `MADS_GEMINI_BIN` | `gemini` login |

`mads providers` shows whether the default executable name is in `PATH`. It does not read the `MADS_*_BIN` overrides, but `generate` and `init` do.

```bash
mads providers
```

```text
claude-cli       cli   ready
codex-cli        cli   `codex` not found in PATH
gemini-cli       cli   `gemini` not found in PATH
```

mads does not manage logins. Log in with the CLI itself first. `--model` is optional. Without it the CLI picks its own default.

```bash
mads generate business.toml --provider claude-cli
mads generate business.toml --provider claude-cli --model <model-id>
```

### claude-cli

mads needs `claude` 2.1.259 or newer. It checks the version before the run starts. A missing or old CLI exits with code `2`.

mads runs claude with these flags.

| Flag | Why |
|---|---|
| `-p <prompt>` | Print mode. |
| `--system-prompt-file` | The mission prompt. |
| `--mcp-config` and `--strict-mcp-config` | The mads server is the only MCP server. |
| `--setting-sources project` | Skip your user settings. |
| `--disable-slash-commands` | Skip your skills and commands. |
| `--tools ""` | No built-in tools. |
| `--allowedTools "mcp__mads__*"` | Allow only the mads tools. |
| `--permission-prompts none` | Never ask. |
| `--output-format stream-json --verbose` | The stream mads reads. |
| `--max-turns <n>` | From `--max-turns`. |
| `--no-session-persistence` | Leave no session behind. |
| `--bare` | Only when `ANTHROPIC_API_KEY` is set. |
| `--model <id>` | Only when you pass `--model`. |

mads also sets `ENABLE_TOOL_SEARCH=false`, so claude loads the mads tools up front.

Bare mode skips your hooks, plugins and `CLAUDE.md`, but it ignores a subscription login. That is why mads uses it only when an API key is set. Without a key, claude runs with your subscription login and `--setting-sources project`. The working directory is an empty temporary folder, so nothing project-specific loads.

### codex-cli

mads runs `codex exec` with `--json --ephemeral --skip-git-repo-check --ignore-user-config --sandbox read-only` and `-c` overrides that point `mcp_servers.mads` at the local server, require it, and turn off the shell tool, multi-agent and web search. The token travels in the environment variable `MADS_MCP_TOKEN`, not on the command line. `codex exec` has no turn limit flag, so the tool call budget (`--max-turns` times 4) acts as the limit. mads does not check the codex version.

### gemini-cli

mads runs `gemini` with `--output-format stream-json --approval-mode yolo --allowed-mcp-server-names mads`. It redirects the CLI's settings and system prompt to the private files through `GEMINI_CLI_SYSTEM_SETTINGS_PATH` and `GEMINI_SYSTEM_MD`. The settings turn core tools off and set `maxSessionTurns` from `--max-turns`. mads does not check the gemini version.

## What was tested

`claude-cli` ran against the real CLI, for `generate` missions and for `init`. The `codex-cli` and `gemini-cli` stream parsers follow the documented output formats and are tested against recorded fixtures. They were not run against the real CLIs. If one of them misbehaves, open an issue with `transcripts/<mission>.cli.jsonl`.

## Why isolation matters for cost

A coding agent loads your configuration into its context by default. That includes settings, skills, plugins and other MCP servers. With claude that can add 150k input tokens to a mission before it does any work. With the flags above, one tiny mission measured about 500 input tokens. The isolation is what makes the CLI providers affordable, so mads does not offer a switch to turn it off.

## Cost and usage

Usage arrives at the end of the CLI's stream, not after every turn. `claude-cli` reports cost in USD, and mads shows it.

```text
12:56:53 [init] tokens 68765 in, 3842 out, $0.1099
```

`codex-cli` reports tokens. `gemini-cli` reports tokens when its result event carries them. Neither reports a cost, so it shows `n/a`.

The token budget (`--max-tokens`) checks after usage arrives. For CLI providers that can lag by one mission. A mission that is running when the budget runs out can finish before mads stops the run. When the check trips during a mission, mads stops the CLI process.

The mission timeout (`--mission-timeout`) stops the CLI process when it expires.

## In CI

Use an API key or a token. A browser login does not exist on a runner.

For claude, either set `ANTHROPIC_API_KEY`, which also turns on bare mode, or create a token on your machine with `claude setup-token` and store it as `CLAUDE_CODE_OAUTH_TOKEN`. The workflow needs claude installed. See [GitHub Actions](github-actions.md).

When you want fewer moving parts, use `--provider anthropic` with `ANTHROPIC_API_KEY`. An API provider needs no CLI and no version check.

## Errors

```text
error: claude not found (claude): install it or point the matching MADS_*_BIN variable at it
error: claude 2.0.1 is too old: mads needs 2.1.259 or newer
```

Both exit with code `2`. Other failures show as a failed mission, such as `claude exited with exit status: 1 before calling finish: <stderr tail>`. See [Troubleshooting](../howto/troubleshooting.md).
