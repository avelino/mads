# CLI

Look up any `mads` command, flag, default or environment variable without reading a guide.

The tables match `mads --help` and `mads <command> --help` of the current build.

```text
mads [OPTIONS] <COMMAND>
```

| Command | What it does | Exit codes |
|---|---|---|
| [`init`](#mads-init) | Reads a website with an agent and drafts `business.toml`, `catalog.csv` and `research.md` for review. | `0`, `1`, `2` |
| [`generate`](#mads-generate) | Builds the account from `business.toml` and writes a run directory. | `0`, `1`, `2`, `3` |
| [`optimize`](#mads-optimize) | Starts a new run from a finished one and the Google Ads reports of its live account. | `0`, `1`, `2`, `3` |
| [`export`](#mads-export) | Validates and exports a finished run again, without calling any model. | `0`, `1`, `3` |
| [`providers`](#mads-providers) | Lists the providers and whether each one is ready. | `0` |
| `help` | Prints help for the program or a subcommand. | |

A flag that clap rejects (unknown flag, missing required flag, bad value) exits `2` on every command. The meaning of each code is in [Exit codes and errors](exit-codes-and-errors.md).

## Global options

They work before or after the subcommand.

| Flag | Env | Default | Description |
|---|---|---|---|
| `--format <FORMAT>` | `MADS_FORMAT` | `auto` | One of `auto`, `pretty`, `plain`, `json`, `github`. |
| `-v`, `--verbose` | | `warn` | `-v` logs at `info`, `-vv` at `debug`, to stderr. `RUST_LOG` overrides it. |
| `-h`, `--help` | | | Prints help. |
| `-V`, `--version` | | | Prints the version. |

### Output formats

| Format | Output |
|---|---|
| `auto` | `github` when `GITHUB_ACTIONS` is `true`, else `pretty` when stdout is a terminal, else `plain`. |
| `pretty` | One spinner per running mission with its last tool summary. Step, validation and URL lines print above the spinners. Agent text and token usage are not shown. |
| `plain` | One `HH:MM:SS [mission] message` line per event. Agent text is cut to 200 characters on one line. Validation prints at most 20 issues per list, then `and N more errors` or `and N more warnings`. |
| `json` | One event per line on stdout and nothing else. The line format is in [Output files](output-files.md). |
| `github` | `plain`, plus a `::error` or `::warning` annotation per validation issue, plus `report.md` appended to the file named by `GITHUB_STEP_SUMMARY` when the run finishes. |

Every format also appends each event to `events.ndjson` in the run directory. Progress and events go to stdout. Logs from `tracing` go to stderr.

## mads init

```text
mads init [OPTIONS] --from-url <FROM_URL> --daily-budget <DAILY_BUDGET> --currency <CURRENCY>
```

An agent studies the business and writes the draft files. To use it, see [Draft the files from your site](../how-to/draft-from-your-site.md).

| Flag | Default | Description |
|---|---|---|
| `--from-url <FROM_URL>` | | Website to read. Required. Must be an absolute `http` or `https` URL. The agent only requests pages on this host. |
| `--daily-budget <DAILY_BUDGET>` | | Daily budget in the account currency. Required. Greater than 0, at most 2 decimals. |
| `--currency <CURRENCY>` | | ISO 4217 code, 3 uppercase letters. Required. |
| `--out-dir <OUT_DIR>` | `.` | Where the files are written. |
| `--catalog-limit <CATALOG_LIMIT>` | `50` | Catalog items the agent may add. |
| `--force` | off | Overwrite existing files. Without it, existing files exit `2`. |
| `--no-web-search` | off | Keep the agent off the web. Agent CLIs search the web by default. API providers never do. |
| `--focus` | off | Advertise only the offer of `--from-url` (a route, a product line), not the whole business. |
| `--strategy <STRATEGY>` | | `visibility`, `profitability`, `need`, or a path to a strategy `.md` file. A value that contains `/` or ends in `.md` is read as a file and its name is the file stem. Any other value exits `2`. |
| `--product <URL>` | | Product page to push, on the same site as `--from-url`. Repeat for more. Replaces the products of the strategy. Needs `--strategy`. |
| `--top <TOP>` | | How many products the agent picks when no `--product` is given. Replaces the top of the strategy. Needs `--strategy`. |

It also takes the [agent flags](#agent-flags). When `--plan-model` is set, init uses it in place of `--model`. It has no run directory flags and no image flags.

Files init writes in `--out-dir`:

| File | When |
|---|---|
| `business.toml` | Always. |
| `research.md` | When the agent wrote research notes. |
| `catalog.csv` | When the agent added items. |
| `brand/logo.png` | When it found a logo on the site. |
| `DESIGN.md` | When it found brand colors or the agent described the design. |
| `STRATEGY.md` | With `--strategy`. |
| `.mads/transcripts/` | Always. Holds the agent transcript. |

`MADS_ALLOW_PRIVATE_HOSTS` set to any non-empty value lets init read loopback and private hosts. It exists for local development.

## mads generate

```text
mads generate [OPTIONS] [BUSINESS]
```

Builds the account and writes a run directory under `--out`. To use it, see [Review a run](../how-to/review-a-run.md).

| Argument or flag | Env | Default | Description |
|---|---|---|---|
| `[BUSINESS]` | | | Path to `business.toml`. Required unless `--resume` is given. |
| `--resume <RESUME>` | | | Run directory to resume. Only the missions that are not finished run again. The input comes from the run's `workspace.json`, not from a `business.toml`. |

Run flags, shared with `optimize`:

| Flag | Env | Default | Description |
|---|---|---|---|
| `--out <OUT>` | | `out` | Directory where run directories are created. |
| `--parallel <PARALLEL>` | | `4` | Campaign missions that run at the same time. |
| `--skip-url-check` | | off | Do not request the final and sitelink URLs. |
| `--max-ad-groups <MAX_AD_GROUPS>` | | `50` | Ad groups allowed in the whole account. |
| `--image-provider <IMAGE_PROVIDER>` | `MADS_IMAGE_PROVIDER` | `auto` | Image model for Performance Max and Demand Gen. One of `auto`, `gemini`, `openai`, `none`. A hidden `solid` value exists for tests. Any other name exits `2`. |
| `--image-model <IMAGE_MODEL>` | `MADS_IMAGE_MODEL` | provider default | Image model name. The defaults are `gemini-2.5-flash-image` for `gemini` and `gpt-image-2` for `openai`. |
| `--max-images <MAX_IMAGES>` | | `40` | New pictures allowed in one run. |
| `--layout <LAYOUT>` | | see below | File set to write. `bulk` or `drive-folders`. |

`--image-provider auto` picks `gemini` when `GEMINI_API_KEY` is set, then `openai` when `OPENAI_API_KEY` is set, else no image model. A named provider without its key exits `2`. A run whose `campaigns.formats` asks for pictures and has no image model exits `2` before the run directory exists.

`--layout` values:

| Value | Files |
|---|---|
| `bulk` | Files 1 to 5 in `google-ads/bulk/` plus `google-ads/editor/account.csv`. See [Bulk upload format](bulk-upload-format.md) and [Google Ads Editor file](editor-file.md). |
| `drive-folders` | The ads automation package in `google-ads/automation/`. See [Ads automation package](automation-package.md). |

Without `--layout`, `generate` writes `bulk`, and `--resume` writes the layout saved in the run's `run.json`.

### Agent flags

`init`, `generate` and `optimize` take these. Provider names and models are in [Providers](providers.md). The limits are in [Run limits](limits.md).

| Flag | Env | Default | Description |
|---|---|---|---|
| `--provider <PROVIDER>` | `MADS_PROVIDER` | none | `anthropic`, `openai`, `gemini`, `openrouter`, `groq`, `deepseek`, `xai`, `ollama`, `openai-compat`, `claude-cli`, `codex-cli` or `gemini-cli`. Required. Without it the command exits `2`. |
| `--model <MODEL>` | `MADS_MODEL` | none | Model id. Required for API providers. Optional for the agent CLIs. |
| `--plan-model <PLAN_MODEL>` | `MADS_PLAN_MODEL` | the value of `--model` | Model for the plan mission only. |
| `--base-url <BASE_URL>` | `MADS_BASE_URL` | none | Endpoint for `openai-compat`. Required there. |
| `--max-turns <MAX_TURNS>` | | `40` | Turns allowed per mission. |
| `--mission-timeout <MISSION_TIMEOUT>` | | `15m` | Time allowed per mission. `90s`, `15m`, `2h` or bare seconds. Zero is rejected. |
| `--max-tokens <MAX_TOKENS>` | | `4000000` | Input plus output tokens allowed in the whole run. `0` removes the limit. |
| `--mission-retries <MISSION_RETRIES>` | | `1` | Extra attempts for a mission that fails. |

## mads optimize

```text
mads optimize [OPTIONS] --reports <REPORTS> <RUN_DIR>
```

Starts a new run from a finished one and runs every mission again. The base run is not changed. To use it, see [Optimize a live account](../how-to/optimize-a-live-account.md). To see how it decides, see [How optimize decides](../explanation/how-optimize-decides.md).

| Argument or flag | Default | Description |
|---|---|---|
| `<RUN_DIR>` | | Finished run directory whose account is live in Google Ads. Required. |
| `--reports <REPORTS>` | | Folder with the CSV reports exported from Google Ads. Required. The report kinds and columns are in [Google Ads reports](google-ads-reports.md). |
| `--daily-budget <DAILY_BUDGET>` | the base run's | New daily budget of the whole account, in its currency. Greater than 0, at most 2 decimals. |
| `--app <STORE=ID>` | the base run's | App to promote, such as `google_play=com.example.app` or `app_store=6751919708`. Repeat it for one app per store. It replaces the apps of the base run. An unknown store, an id that does not fit its store, or two apps of one store exit `2`. An app that stays, with the same store and id, keeps its `optimize_for` and `conversion_action`. |
| `--status <paused\|enabled>` | the base run's | Status of the campaigns that are not in the account yet. It replaces `export.status` of the base run. A campaign already in the account that the plan keeps is exported enabled. |
| `--target-cpa <TARGET_CPA>` | the base run's, else none | The most you pay for one conversion, in the account currency. Greater than 0, at most 2 decimals, else exit `2`. |
| `--target-cpi <TARGET_CPI>` | the base run's, else none | The most you pay for one app install. App campaigns are judged by it. Same validation as `--target-cpa`. |
| `--target-cps <TARGET_CPS>` | the base run's, else none | The most you pay for one in-app action, such as a sign-up. With `--app-events`, App campaigns are judged by it in place of `--target-cpi`. Same validation as `--target-cpa`. |
| `--app-events <CSV>` | none | CSV from your product analytics with the columns `date`, `store` (`google_play` or `app_store`), `installs` and `in_app_actions`. |
| `--pause-outside-run` | off | Writes a `Paused` row for each enabled campaign outside the run that has no conversion while the account converts, and has 100 clicks or more or over 10 percent of the account cost. A removed campaign is never touched. Without it, campaigns outside the run are left as they are. |

It also takes the [run flags](#mads-generate) and the [agent flags](#agent-flags). It has no `[BUSINESS]` and no `--resume`.

`--layout` on `optimize` defaults to the layout saved in the base run's `run.json`, and to `bulk` when the base run has none. An explicit `--layout` wins.

`optimize` exits `1` before it creates a run directory when:

- the run directory does not exist or has no readable `workspace.json`
- the run has unfinished missions
- the reports folder holds no report mads knows
- the `--app-events` file cannot be read

The base run is read before the flags are checked, so a bad `--daily-budget` or `--target-*` with an unreadable base run exits `1`, not `2`. After the run directory exists, `optimize` prints a summary of the reports to stderr, just before the missions start.

## mads export

```text
mads export [OPTIONS] <RUN_DIR>
```

Reruns cross negatives, validation, the URL check and the file export on the run's `workspace.json`. It calls no model. To use it, see [Resume or re-export a run](../how-to/resume-a-run.md).

| Argument or flag | Default | Description |
|---|---|---|
| `<RUN_DIR>` | | Run directory with a finished run. Required. |
| `--skip-url-check` | off | Do not request the final and sitelink URLs. |
| `--max-ad-groups <MAX_AD_GROUPS>` | `50` | Ad groups allowed in the whole account. |
| `--layout <LAYOUT>` | the layout saved in `run.json`, else `bulk` | Same values as for `generate`. |

`export` has no image model. It reuses the pictures in `google-ads/editor/images/`, and a missing one fails with `E20`.

`export` exits `1` when the directory does not exist or a mission is unfinished.

Every export first removes the CSV files of the earlier export from `google-ads/`, `google-ads/bulk/` and `google-ads/editor/`, and the `google-ads/automation/` folder, then writes the new set. A failed export (exit `3`) leaves none of them. Pictures in `editor/images/` stay.

## mads providers

```text
mads providers [OPTIONS]
```

Prints the provider table, a blank line, and the image provider table. It exits `0` in every case.

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

This is the output on a machine with no keys set and `claude` on the `PATH`. Yours differs.

| Status | Meaning |
|---|---|
| `ready` | An API provider with its key set, a provider that needs no key, or an agent CLI that was found. |
| `missing <ENV_VAR>` | The provider needs that variable and it is unset or empty. |
| `` `<name>` not found in PATH `` | The agent CLI executable is not found. |

For an agent CLI, the check looks at the `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN` or `MADS_GEMINI_BIN` override first, then at `PATH`.

## Environment variables

| Variable | Read by | Description |
|---|---|---|
| `MADS_FORMAT` | all commands | Same as `--format`. |
| `RUST_LOG` | all commands | Log filter. Overrides `-v`. |
| `GITHUB_ACTIONS` | all commands | `auto` format picks `github` when it is `true`. |
| `GITHUB_STEP_SUMMARY` | `github` format | File that gets `report.md` appended. |
| `MADS_PROVIDER`, `MADS_MODEL`, `MADS_PLAN_MODEL`, `MADS_BASE_URL` | `init`, `generate`, `optimize` | Same as the agent flags. |
| `MADS_API_KEY` | `openai-compat` | Key sent to the endpoint. Optional. |
| `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN`, `MADS_GEMINI_BIN` | agent CLI providers, `providers` | Path to the executable. |
| `MADS_IMAGE_PROVIDER`, `MADS_IMAGE_MODEL` | `generate`, `optimize` | Same as the image flags. |
| `MADS_ALLOW_PRIVATE_HOSTS` | `init` | Allows loopback and private hosts. |

The full list, including provider keys, is in [Environment variables](environment-variables.md).

## Next

- [Exit codes and errors](exit-codes-and-errors.md)
- [Providers](providers.md)
- [Environment variables](environment-variables.md)
