# CLI reference

This page lists every `mads` command and flag with its default, taken from `--help` of the current build.

```text
mads [OPTIONS] <COMMAND>
```

| Command | Purpose |
|---|---|
| `init` | Read a website with an agent and draft `business.toml` and `catalog.csv` for review. |
| `generate` | Generate campaigns and the Google Ads bulk upload CSVs from `business.toml`. |
| `export` | Validate and export a finished run again, without calling any model. |
| `providers` | List the providers and whether each one is ready. |
| `help` | Print help for the program or a subcommand. |

## Global options

They work before or after the subcommand.

| Flag | Env | Default | Description |
|---|---|---|---|
| `--format <FORMAT>` | `MADS_FORMAT` | `auto` | One of `auto`, `pretty`, `plain`, `json`, `github`. |
| `-v`, `--verbose` | | warn | `-v` logs at info, `-vv` at debug, to stderr. `RUST_LOG` overrides. |
| `-h`, `--help` | | | Print help. |
| `-V`, `--version` | | | Print the version. |

### Formats

| Format | Output |
|---|---|
| `auto` | `github` when `GITHUB_ACTIONS=true`, else `pretty` when stdout is a terminal, else `plain`. |
| `pretty` | One spinner per running mission with its last tool summary. Validation, URL check and step lines print above the spinners. |
| `plain` | `HH:MM:SS [mission] message` lines. Agent text is cut to 200 characters on one line. Validation prints at most 20 issues per list. |
| `json` | One event per line on stdout and nothing else. See [Output files](output-files.md#eventsndjson). |
| `github` | `plain`, plus `::error` and `::warning` annotations for validation issues, plus `report.md` appended to `$GITHUB_STEP_SUMMARY`. |

Every format also appends each event to `events.ndjson` in the run directory.

## mads init

```text
mads init [OPTIONS] --from-url <FROM_URL> --daily-budget <DAILY_BUDGET> --currency <CURRENCY>
```

An agent reads the website and writes `business.toml` and `catalog.csv`. See [Init from a URL](../guides/init-from-url.md).

| Flag | Default | Description |
|---|---|---|
| `--from-url <FROM_URL>` | | Website to read. The agent only requests pages on this host. Required. |
| `--daily-budget <DAILY_BUDGET>` | | Daily budget in the account currency. Required. Greater than 0, at most 2 decimals. |
| `--currency <CURRENCY>` | | ISO 4217 code, 3 uppercase letters. Required. |
| `--out-dir <OUT_DIR>` | `.` | Where `business.toml` and `catalog.csv` are written. |
| `--catalog-limit <CATALOG_LIMIT>` | `50` | Catalog items the agent may add. |
| `--force` | off | Overwrite existing files. |

It also takes the agent flags of `generate` (`--provider`, `--model`, `--plan-model`, `--base-url`, `--max-turns`, `--mission-timeout`, `--max-tokens`, `--mission-retries`) with the same defaults. When `--plan-model` is set, init uses it instead of `--model`.

init writes only the two files. It creates no run directory. It refuses to overwrite existing files without `--force` and exits `2`. Exit codes are `0` (files written), `1` (the mission failed) and `2` (bad flags, input or existing files). Set `MADS_ALLOW_PRIVATE_HOSTS=1` to let it read loopback and private hosts, for local development only.

## mads generate

```text
mads generate [OPTIONS] [BUSINESS]
```

Builds the account and writes the run directory.

| Argument or flag | Env | Default | Description |
|---|---|---|---|
| `[BUSINESS]` | | | Path to `business.toml`. Required unless `--resume` is given. |
| `--out <OUT>` | | `out` | Directory where run directories are created. |
| `--resume <RESUME>` | | | Run directory to resume. Only missions that are not finished run again. |
| `--parallel <PARALLEL>` | | `4` | Campaign missions that run at the same time. |
| `--skip-url-check` | | off | Do not request the final and sitelink URLs. |
| `--max-ad-groups <MAX_AD_GROUPS>` | | `50` | Ad groups allowed in the whole account. |

Agent flags:

| Flag | Env | Default | Description |
|---|---|---|---|
| `--provider <PROVIDER>` | `MADS_PROVIDER` | none | `anthropic`, `openai`, `gemini`, `openrouter`, `groq`, `deepseek`, `xai`, `ollama`, `openai-compat`, `claude-cli`, `codex-cli` or `gemini-cli`. Required. |
| `--model <MODEL>` | `MADS_MODEL` | none | Model id. Required for API providers, optional for CLI providers. |
| `--plan-model <PLAN_MODEL>` | `MADS_PLAN_MODEL` | `--model` | Model for the plan mission only. |
| `--base-url <BASE_URL>` | `MADS_BASE_URL` | none | Endpoint for `openai-compat`. Required there. |
| `--max-turns <MAX_TURNS>` | | `40` | Turns allowed per mission. |
| `--mission-timeout <MISSION_TIMEOUT>` | | `15m` | Time allowed per mission. `90s`, `15m`, `2h` or bare seconds. Zero is rejected. |
| `--max-tokens <MAX_TOKENS>` | | `4000000` | Input plus output tokens allowed in the whole run. `0` disables the limit. |
| `--mission-retries <MISSION_RETRIES>` | | `1` | Extra attempts for a mission that fails. |

`MADS_API_KEY` is read by `openai-compat`. `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN` and `MADS_GEMINI_BIN` override the CLI executables. See [Environment variables](environment-variables.md).

Examples that run as written once you set a key.

```bash
mads generate business.toml --provider anthropic --model <model-id>
mads generate business.toml --provider ollama --model <model-name> --format plain
mads generate business.toml --provider claude-cli
mads generate --resume out/<run-id> --provider anthropic --model <model-id>
```

Exit codes: `0`, `1`, `2`, `3`. See below.

## mads export

```text
mads export [OPTIONS] <RUN_DIR>
```

Reruns cross negatives, validation, the URL check and the CSV export on `workspace.json`. No model call.

| Argument or flag | Default | Description |
|---|---|---|
| `<RUN_DIR>` | | Run directory with a finished run. Required. |
| `--skip-url-check` | off | Skip the URL check. |
| `--max-ad-groups <MAX_AD_GROUPS>` | `50` | Ad groups allowed in the whole account. |

`export` fails with exit code `1` when the directory does not exist or a mission is unfinished. A failed export (exit `3`) removes the CSV files an earlier export left in `google-ads/`.

## mads providers

```bash
mads providers
```

```text
PROVIDER         KIND  STATUS
anthropic        api   missing ANTHROPIC_API_KEY
openai           api   missing OPENAI_API_KEY
gemini           api   missing GEMINI_API_KEY
openrouter       api   missing OPENROUTER_API_KEY
groq             api   missing GROQ_API_KEY
deepseek         api   missing DEEPSEEK_API_KEY
xai              api   missing XAI_API_KEY
ollama           api   ready
openai-compat    api   ready
claude-cli       cli   ready
codex-cli        cli   `codex` not found in PATH
gemini-cli       cli   `gemini` not found in PATH
```

Statuses are `ready`, `missing <ENV_VAR>` for an API provider without a key, and a "not found in PATH" message for an agent CLI. The command always exits `0`. It checks the default executable names and ignores the `MADS_*_BIN` overrides.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. CSVs written. |
| `1` | Runtime failure. Provider or network error, mission failed after retries, token budget exceeded, missing run directory. |
| `2` | Invalid input or usage. Bad `business.toml` or `catalog.csv`, unknown flag, missing provider or model, agent CLI missing or too old, `init` files that already exist. |
| `3` | Account invalid after post-processing. Validation errors or URL check failures. No CSV written. |

Details and fixes are in [Troubleshooting](../howto/troubleshooting.md).

## Logs

Logs from `tracing` always go to stderr. The default level is `warn`. Progress and events go to stdout.
