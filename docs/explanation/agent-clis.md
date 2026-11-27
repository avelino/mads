# Agent CLIs

Judge what changes, and what never does, when `claude`, `codex` or `gemini` acts as the agent instead of an API provider.

## The idea

An API provider is a model behind an HTTP endpoint. mads loops over it and calls its own tools. An agent CLI is a full coding agent that wants to run its own loop.

mads lets it. But it hands the CLI one thing only: the tools of the current mission. The CLI is the model. The tools stay the same ones an API provider would call, so the same rules apply. A tool validates before it writes, and a failed call changes nothing. See [MCP tools](../reference/mcp-tools.md).

## One server per mission

Each mission gets its own MCP server on `127.0.0.1`, on a random port. It exposes only that mission's tools. Every request needs a bearer token of 64 random hex characters, so another process on the machine cannot call the tools.

The CLI's config (system prompt and MCP settings) goes into a fresh temporary directory in the OS temp folder, written with mode `0600`. When the mission ends, mads stops the server and deletes the directory.

Two consequences follow. The token never lands in `out/`, so an artifact you upload from CI cannot leak it. And two missions running in parallel never share a server, a token or a working directory.

Tool calls are reported by the server, not parsed from the CLI's output. Progress therefore looks the same as with an API provider. The CLI's own text and usage are read from its JSON stream. The raw stream of each mission is kept in `transcripts/<mission>.cli.jsonl`.

## When a mission is done

A mission is done when the agent calls `finish` and the tool accepts it. If the CLI exits first, the mission fails. The failure carries the exit status and the tail of the CLI's stderr, so the reason is usually in the message.

## Why isolation matters

A coding agent loads your configuration by default. That means settings, skills, plugins and other MCP servers. With claude this added about 157k input tokens to a one-word prompt in an unisolated test. With the flags mads sets, the same kind of tiny mission stays near 500.

So mads turns the agent's own tools off and cuts it off from your setup. The CLI sees a private temporary working directory, the mads server as its only MCP server, and no way to ask for permission. There is no switch to undo this. Without it the CLI providers cost far more than the API ones and behave differently on every machine.

For claude there is one more detail. mads turns on claude's bare mode only when `ANTHROPIC_API_KEY` is set and not empty. Without a key, claude uses your login and mads skips your user settings another way, by reading project settings only. mads also turns off claude's auto-updater. It swaps the binary when a mission exits, and the missions that start right after would fail.

The exact flags for each CLI are in [Providers](../reference/providers.md).

## What each CLI can and cannot do

The three CLIs differ in what mads can enforce.

- claude. mads checks the version before the run starts. A missing, unreadable or old CLI stops with exit code `2` before any mission runs.
- codex. It runs read-only and ephemeral, without your user config. The token travels in an environment variable, not on the command line. `codex exec` has no turn limit, so mads uses the tool call budget as the limit.
- gemini. mads redirects its settings and system prompt to the private files. mads writes `--max-turns` into its settings file as the turn limit.

mads enforces a minimum version only for claude. For codex and gemini it still runs `--version` and stops if the CLI is missing or prints no version, but it accepts any version.

## Web search in init

The `init` agent studies a market, so it may search the web. For that mission only, mads opens the CLI's own search and keeps every other built-in tool off. `generate` missions never get web search. `mads init --no-web-search` keeps it off for `init` too.

Searches do not pass through the mads server. They emit no `tool_called` events. Their tokens still count in the usage the CLI reports, so they count against `--max-tokens`.

## Usage and cost

Usage arrives at the end of the CLI's stream, not after each turn. So the token budget checks late. A mission already running when the budget runs out can finish before mads stops the run. When the check trips during a mission, mads kills the CLI process. The mission timeout does the same when it expires. See [Limits and retries](limits-and-retries.md).

claude reports a cost in USD and mads shows it. codex reports tokens only. gemini reports tokens when its result event carries them. Neither of those two reports a cost, so it shows as `n/a`.

## What has been run for real

Only `claude-cli` has an integration test against the real CLI, and it is opt-in. The `codex-cli` and `gemini-cli` stream parsers follow the documented output formats and run against recorded fixtures. If one of them misbehaves, open an issue with its `transcripts/<mission>.cli.jsonl`.

## Next

- [Choose a provider](../how-to/choose-a-provider.md)
- [Providers reference](../reference/providers.md)
- [Run in GitHub Actions](../how-to/run-in-github-actions.md)
