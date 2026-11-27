# Exit codes and errors

Look up what a `mads` exit code means and which error messages sit behind it.

Errors print on stderr as `error: <message>`. Progress lines such as `[plan] failed: <reason>` print on stdout. Run with `-v` for info logs and `-vv` for debug. `RUST_LOG` overrides both. For when to use each fix, see [Troubleshoot a run](../how-to/troubleshoot.md).

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. Files written. |
| `1` | Runtime failure. A provider or network error, a mission that failed after its retries, the token cap, a run directory or report folder that cannot be read, or a file that cannot be written. |
| `2` | Invalid input or usage. Raised before any model call. |
| `3` | The account is invalid after post-processing. Validation errors or URL check failures. No CSV written. |

Which codes each command can return.

| Command | Codes | Notes |
|---|---|---|
| `mads generate` | `0` `1` `2` `3` | `--resume` with an unreadable `workspace.json` is `1`. |
| `mads export` | `0` `1` `3` | Every read failure is `1`. Bad flags are `2`, raised by the argument parser. |
| `mads optimize` | `0` `1` `2` `3` | Provider and flag errors are `2`. Run, report and app events errors are `1`. |
| `mads init` | `0` `1` `2` | `1` when the mission does not finish. No files are written then. |
| `mads providers` | `0` | Always `0`, whatever the statuses say. |

An unknown flag, a missing required argument and an invalid flag value are exit `2`. The argument parser prints the message.

## Exit code 2

### Providers and models

| Message | Cause |
|---|---|
| `no provider: use --provider or MADS_PROVIDER` | Neither the flag nor the variable is set. |
| `unknown provider '<name>'; run `mads providers` to see the options` | The name is not in `mads providers`. |
| `--model (or MADS_MODEL) is required for <provider>` | An API provider needs a model. |
| `--base-url (or MADS_BASE_URL) is required for openai-compat` | `openai-compat` needs an endpoint. |
| `<cli> not found (<program>): install it or point the matching MADS_*_BIN variable at it` | The agent CLI is not in `PATH` and no `MADS_*_BIN` points at it. |
| `<cli> <version> is too old: mads needs <min> or newer` | The agent CLI is older than the minimum. |
| `cannot read the <cli> version from: <output>` | The CLI ran but its version output did not parse. |

The providers and their statuses are in [Providers](providers.md). The `MADS_*_BIN` variables are in [Environment variables](environment-variables.md).

### Image provider

| Message | Cause |
|---|---|
| `unknown image provider '<name>': use auto, gemini, openai or none` | The `--image-provider` value is not known. |
| `--image-provider <name> needs <ENV_VAR>` | `gemini` needs `GEMINI_API_KEY`. `openai` needs `OPENAI_API_KEY`. |
| `campaigns.formats asks for <format> and there is no image model: set OPENAI_API_KEY or GEMINI_API_KEY, or pick one with --image-provider` | An image format is listed and no image model resolved. |

### business.toml and catalog.csv

A bad value prints `<key>: <message>`. A syntax error or an unknown key prints the TOML parser's message with the line and column. A file that cannot be opened prints `<path>: <os error>`.

| Message | Cause |
|---|---|
| `business.locations: must have 1 to 10 locations` | Zero or more than 10 locations. |
| `business.url: must be an absolute http(s) URL` | The URL has no scheme or host. |
| `business.description: must be 20 to 4000 chars, got <n>` | The text is out of range. Other text keys print the same shape with their own range. |
| `budget.daily: must be greater than 0 with at most 2 decimals` | `budget.max_cpc` prints the same message under its own key. |
| `budget.currency: must be 3 uppercase letters` | The code is not three letters A to Z. |
| `brand.logo: <path>: <reason>` | The reason is `<w>x<h> is not square`, `<w>x<h> is below the minimum 144x144` or `<n> bytes, limit is 153600`. |
| `campaigns.formats: <format> needs [brand] logo` | An image format is listed without a logo. |
| `campaigns.formats: Performance Max needs business.conversion_tracking = true` | Performance Max without conversion tracking. |
| `catalog.csv header: required columns: name, url` | The header lacks `name` or `url`. |
| `catalog.csv row <n> name: must be 1 to 120 chars` | Row numbers count the header as row 1. |
| `catalog.csv row <n> url: not an absolute http(s) URL: <url>` | A bad URL. The `image` column prints the same message under `image`. |
| `catalog.csv row <n> url: duplicate url: <url>` | Two rows share a URL. |
| `catalog.csv row <n> third_party: expected true or false, got <value>` | A bad flag value. |
| `catalog.csv row <n> notes: at most 500 chars` | The notes are too long. |
| `catalog.csv row 5002 : more than 5000 rows` | The first row past the limit. |

Every key and limit is in [business.toml reference](business-toml.md) and [catalog.csv reference](catalog-csv.md).

### Flags checked by the command

| Message | Command |
|---|---|
| `business.toml is required` | `generate` without a path and without `--resume`. |
| `--from-url must be an absolute http(s) URL, got '<value>'` | `init` |
| `--daily-budget must be greater than 0 with at most 2 decimals` | `init` |
| `--currency must be 3 uppercase letters, such as BRL` | `init` |
| `<files> already exist: use --force to overwrite` | `init` |
| `--daily-budget must be greater than 0 with at most 2 decimals` | `optimize`. `--target-cpa`, `--target-cpi` and `--target-cps` print the same message. |
| `invalid value '<v>' for '--mission-timeout <MISSION_TIMEOUT>': ...` | Any command with the agent flags. Durations take `90s`, `15m`, `2h` or bare seconds. |

An invalid `--app` value is exit `2` and prints the `app.*` message from [business.toml reference](business-toml.md).

## Exit code 1

### Reading a run

| Message | Command |
|---|---|
| `cannot resume <dir>: <reason>` | `generate --resume`. The folder has no readable `workspace.json`. |
| `run directory not found: <path>` | `export` |
| `run has unfinished missions (<ids>); run `mads generate --resume <run-dir>` first` | `export`. A run with no missions fails the same way, with an empty id list. |
| `cannot read run <dir>: <reason>` | `optimize` |
| `run has unfinished missions (<ids>); finish it with `mads generate --resume <run-dir>` first` | `optimize` |
| `cannot read reports folder <dir>: <reason>` | `optimize` |
| `no Google Ads report in <dir>: export search terms, keywords or campaigns as CSV (<details>)` | `optimize`. The details list each CSV with the reason it was skipped, or `no .csv file`. |
| `cannot read app events <file>: <reason>` | `optimize --app-events` |

No run directory is created for any of the `optimize` errors. The report types mads recognizes and the reasons for skipping a file are in [Google Ads reports](google-ads-reports.md).

An ads report is refused with a reason that ends `an ads report has one row per ad, and mads reads no ad-level numbers. Export the search keywords and search terms reports instead`. A file with an unknown header gets `no known report header (...)` followed by the first columns it found.

`warning: the reports cover different dates, export them for the same dates` is not an error. The run goes on.

### Writing files

| Message | Cause |
|---|---|
| `could not create the run directory: <os error>` | The `--out` folder is not writable. |
| `could not copy business.toml into the run: <os error>` | The snapshot of the inputs failed. |
| `could not copy the catalog into the run: <os error>` | Same, for the catalog. |
| `could not copy the logo into the run: <os error>` | Same, for the logo. |
| `could not start the HTTP client: <reason>` | The HTTP client failed to build. |

### Mission failures

A failed mission prints `[<mission>] failed: <reason>` and the run ends with a line such as `[run] failed (exit 1): ... 1 of 3 missions failed`. The report status is `Incomplete`.

| Reason | Cause |
|---|---|
| `max turns` | The mission used all of `--max-turns` without calling `finish`. |
| `no progress` | The model answered three times in a row without calling a tool. |
| `timeout` | The mission ran past `--mission-timeout`. |
| `token budget exceeded` | The run passed `--max-tokens`. `0` disables the cap. |
| `<cli> exited with <status> before calling finish: <stderr tail>` | An agent CLI ended without calling `finish`. |
| `cannot wait for <cli>: <error>` | mads could not collect the agent CLI's exit status. |
| the provider's message | A fatal provider error. mads cuts it to 600 characters. |

A failed mission runs again with a fresh context, `--mission-retries` times. The defaults of the flags are in [Limits](limits.md). Why the loop stops and how retries work is in [Limits and retries](../explanation/limits-and-retries.md).

Inside one model call, HTTP 408, 429, any 5xx and network failures are retried. A call has 3 attempts, with a wait of 1 second and then 2 seconds. Any other status fails at once.

### Init tool codes

An init mission that does not finish exits `1` and writes no files. Tool errors the agent saw print as `< error ...` lines.

| Code | Cause |
|---|---|
| `HOST` | The URL is not on the business site. IP-literal and private hosts are refused. `MADS_ALLOW_PRIVATE_HOSTS` set to any non-empty value lifts the block on loopback and private addresses. |
| `FETCH` | The page or sitemap request failed. The message carries the HTTP status or the network error. |
| `LIMIT` | `at most <n> pages can be fetched`, or `tool call budget for this mission is exhausted`. |
| `E07` | `URL was not fetched or listed in the sitemap: <url>`. |
| `E13` | A catalog would pass `--catalog-limit`, or a strategy has the wrong number of products. |

## Exit code 3

The missions finished and the account has errors, or a URL did not answer. mads writes `report.md` and no CSV. Each error prints as `[validate] <CODE> <path>: <message>`. The terminal shows the first 20 of each kind, then `and <n> more errors`.

```text
[validate] 7 errors, 24 warnings
[run] failed (exit 3): <n> in, <n> out tokens, cost n/a, 3 missions finished
```

On GitHub Actions each issue also prints as `::error title=<CODE>::<path>: <message>`.

| Code | Message and path |
|---|---|
| `E15` | `URL check failed: HTTP <status>` or `URL check failed: unreachable`. The path is the URL. |
| `E20` | `image '<id>' has no file`. |
| other `E` codes | See [Validation rules](validation-rules.md). |

`--skip-url-check` skips the URL check, and the report says `skipped` under URL check.

A run or export that reaches the end step and exits `1` or `3` removes the CSV files an earlier export left in `google-ads/`, `google-ads/bulk/` and `google-ads/editor/`, and the `google-ads/automation/` folder. Pictures in `editor/images/` stay.

## Next

- [Troubleshoot a run](../how-to/troubleshoot.md)
- [Validation rules](validation-rules.md)
- [Providers](providers.md)
