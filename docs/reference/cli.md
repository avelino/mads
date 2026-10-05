# CLI reference

This page lists every `mads` command and flag with its default, taken from `--help` of the current build.

```text
mads [OPTIONS] <COMMAND>
```

| Command | Purpose |
|---|---|
| `init` | Read a website with an agent and draft `business.toml` and `catalog.csv` for review. |
| `generate` | Generate campaigns and the Google Ads bulk upload CSVs from `business.toml`. |
| `optimize` | Optimize a finished run that is live in Google Ads, from the reports exported there. |
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

An agent studies the business and writes `business.toml`, `catalog.csv` and `research.md`. See [Init from a URL](../guides/init-from-url.md).

| Flag | Default | Description |
|---|---|---|
| `--from-url <FROM_URL>` | | Website to read. The agent only requests pages on this host. Required. |
| `--daily-budget <DAILY_BUDGET>` | | Daily budget in the account currency. Required. Greater than 0, at most 2 decimals. |
| `--currency <CURRENCY>` | | ISO 4217 code, 3 uppercase letters. Required. |
| `--out-dir <OUT_DIR>` | `.` | Where `business.toml`, `catalog.csv` and `research.md` are written. |
| `--catalog-limit <CATALOG_LIMIT>` | `50` | Catalog items the agent may add. |
| `--force` | off | Overwrite existing files. |
| `--no-web-search` | off | Keep the agent off the web. Agent CLIs search the web by default. API providers never do. |
| `--focus` | off | Advertise only the offer of `--from-url` (a route, a product line, a location), not the whole business. init writes a `[focus]` table with `terms` and keeps the catalog inside it. |

It also takes the agent flags of `generate` (`--provider`, `--model`, `--plan-model`, `--base-url`, `--max-turns`, `--mission-timeout`, `--max-tokens`, `--mission-retries`) with the same defaults. When `--plan-model` is set, init uses it instead of `--model`.

init writes `business.toml`, `research.md`, `catalog.csv` when it added items, `brand/logo.png` when it found a logo on the site, and `DESIGN.md` when it found brand colors or the agent described the design. It keeps the agent's transcript in `<out-dir>/.mads/transcripts/`. It creates no run directory. It refuses to overwrite existing files without `--force` and exits `2`. Exit codes are `0` (files written), `1` (the mission failed) and `2` (bad flags, input or existing files). Set `MADS_ALLOW_PRIVATE_HOSTS=1` to let it read loopback and private hosts, for local development only.

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
| `--image-provider <IMAGE_PROVIDER>` | `MADS_IMAGE_PROVIDER` | `auto` | Image model for Performance Max and Demand Gen: `auto`, `gemini`, `openai` or `none`. `auto` picks gemini when `GEMINI_API_KEY` is set, then openai when `OPENAI_API_KEY` is set, else none. An unknown name exits `2`. |
| `--image-model <IMAGE_MODEL>` | `MADS_IMAGE_MODEL` | | Image model name. Defaults `gemini-2.5-flash-image` and `gpt-image-1`. |
| `--max-images <MAX_IMAGES>` | | `40` | New pictures allowed in one run. See [Image campaigns](../guides/image-campaigns.md). |

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

## mads optimize

```text
mads optimize [OPTIONS] --reports <REPORTS> <RUN_DIR>
```

Starts a new run from a finished one and the CSV reports of its live account, and runs every mission again with those numbers. The base run is not changed. See [Optimize a live account](../guides/optimize.md).

| Argument or flag | Default | Description |
|---|---|---|
| `<RUN_DIR>` | | Finished run directory whose account is live in Google Ads. Required. |
| `--reports <REPORTS>` | | Folder with the CSV reports exported from Google Ads. Required. |

It takes every flag of `mads generate` except `[BUSINESS]` and `--resume`: `--out`, `--parallel`, `--skip-url-check`, `--max-ad-groups`, the image flags and the agent flags, with the same defaults.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --provider anthropic --model <model-id>
```

`optimize` fails with exit code `1`, before creating a run directory, when the run does not exist, has unfinished missions, or the folder holds no report mads knows.

## mads export

```text
mads export [OPTIONS] <RUN_DIR>
```

Reruns cross negatives, validation, the URL check and the CSV export on `workspace.json`. No model call. It has no image model: it reuses the pictures already in `google-ads/editor/images/`, and a missing one fails with `E20`.

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

IMAGE PROVIDER         STATUS
gemini                 missing GEMINI_API_KEY
openai                 missing OPENAI_API_KEY
none                   ready
```

The second table lists the image providers for `--image-provider`.

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
