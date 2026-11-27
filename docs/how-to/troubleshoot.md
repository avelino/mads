# Troubleshoot a run

Find what went wrong with a `mads` command or a Google Ads import, and fix it from the symptom you see.

This page is organized by symptom. For the full list of messages behind each exit code, see [Exit codes and errors](../reference/exit-codes-and-errors.md).

## Before you start

- You ran a `mads` command and it failed, or it succeeded and Google Ads showed a problem.
- You have the exit code. In a shell, `echo $?` right after the command prints it.
- You know the run directory if the command made one. It is the folder with `workspace.json`. `mads init` makes no run directory. See [Output files](../reference/output-files.md).

## Steps

### 1. Read the exit code

| Exit code | Where to go |
|---|---|
| `0` and Google Ads shows a problem | [The run succeeded but the import has problems](#4-the-run-succeeded-but-the-import-has-problems) |
| `1` | [Exit code 1](#2-exit-code-1-the-run-failed) |
| `2` | [Exit code 2](#3-exit-code-2-mads-stopped-before-calling-a-model) |
| `3` | [Exit code 3](#5-exit-code-3-the-account-has-errors) |

Add `-v` for info logs on stderr and `-vv` for debug. `RUST_LOG` overrides both. The first place to look in a `generate` run is `events.ndjson` in the run directory. It holds every event of the run. `mads init` writes its transcripts to `.mads/transcripts/` in the output folder and prints that path when it fails.

### 2. Exit code 1, the run failed

The run started and something broke while it worked. Find the line that starts with a mission name in brackets, such as `[plan] failed: <reason>`.

| Reason on the line | What to do |
|---|---|
| `ApiKeyEnvNotFound { env_name: "<VAR>" }` | Export the variable it names in the same shell, then resume. |
| `Reqwest error: error sending request for url (...)` | The server is not reachable. Start it or fix the URL. For a local model see [Run with Ollama](run-with-ollama.md). |
| `max turns` | Raise `--max-turns`, or use a stronger model. Look for tool calls that fail in a loop (see below). |
| `no progress` | The model answered three times without calling a tool. Use a model that supports tool calls. |
| `timeout` | Raise `--mission-timeout`, and check provider latency. |
| `token budget exceeded` | Raise `--max-tokens`, or set it to `0`. |
| `<cli> exited with <status> before calling finish: ...` | Read the stderr tail on that line. See the agent CLI step below. |

Rate limits (429), timeouts (408), 5xx responses and network failures are retried inside the call. A call has 3 attempts, with a wait of 1 second and then 2. Other HTTP errors, such as an invalid key or an unknown model, fail at once. The message carries the provider's text. How the limits and the two retry layers work is in [Limits and retries](../explanation/limits-and-retries.md).

To find tool calls that failed in a loop, search the event log.

```bash
grep '"type":"tool_finished"' out/<run-id>/events.ndjson | grep '"ok":false'
```

If a mission was run by an agent CLI and ended without calling `finish`, the raw stream is in `transcripts/<mission>.cli.jsonl`, with `:` in the mission id written as `-`. When `claude` is not logged in, run `claude` once and log in. In CI, `claude` reads `CLAUDE_CODE_OAUTH_TOKEN` (made with `claude setup-token`) or `ANTHROPIC_API_KEY`. See [Agent CLIs](../explanation/agent-clis.md).

After you fix the cause, continue the same run with the same flags. Only unfinished missions run again. See [Resume or re-export a run](resume-a-run.md).

```bash
mads generate --resume out/<run-id> --provider <provider> --model <model-id>
```

Other exit `1` messages name what could not be read or written. They are in the [reference](../reference/exit-codes-and-errors.md#exit-code-1).

- `cannot resume <dir>` and `run directory not found`. Pass the run directory, the folder with `workspace.json`, not the `out` folder.
- `run has unfinished missions`. `export` and `optimize` need every mission finished. Resume first.
- `no Google Ads report in <dir>`. The message lists each CSV with the reason mads skipped it. Export the report again from Google Ads as CSV. A header renamed by hand is not recognized. An ads report is refused, so export the search keywords and search terms reports instead. See [Google Ads reports](../reference/google-ads-reports.md).

`warning: the reports cover different dates, export them for the same dates` is not an error. The run goes on, and the cost hidden from search terms is left out.

An `init` mission that fails prints `failed: <reason>` and writes no files. The tool codes (`HOST`, `FETCH`, `LIMIT`, `E07`, `E13`) are in the [reference](../reference/exit-codes-and-errors.md#init-tool-codes). For a local site, set `MADS_ALLOW_PRIVATE_HOSTS=1`.

### 3. Exit code 2, mads stopped before calling a model

mads checked your input and refused it. Nothing was spent. The message names the problem. Look it up in the reference by the start of the message.

- Provider and model messages, such as `no provider`, `--model ... is required` and `not found`. See [Providers](../reference/providers.md#errors).
- Image provider messages. Export the key in the same shell, or pick another provider with `--image-provider`. `mads providers` shows which image providers are ready.
- `business.toml` messages. A bad value names its key, such as `budget.currency: must be 3 uppercase letters`. An unknown key shows the TOML location and the keys mads allows. Every key and limit is in the [business.toml reference](../reference/business-toml.md).
- `catalog.csv` messages. Row numbers count the header as row 1. See the [catalog.csv reference](../reference/catalog-csv.md).
- `already exist: use --force to overwrite`. `mads init` does not overwrite your files. Pass `--force` only if you want that.
- Flag errors from the argument parser, such as an unknown flag or `duration must be greater than 0`. Run the command with `--help`. Durations take `90s`, `15m`, `2h` or bare seconds. See the [CLI reference](../reference/cli.md).

The full tables are in [Exit code 2](../reference/exit-codes-and-errors.md#exit-code-2). Fix the input and run the command again.

### 4. The run succeeded but the import has problems

The exit code was `0`. The files are valid for mads, and Google Ads or Editor shows a problem. Find your symptom.

#### No pictures were generated

The report has no `Images` section, and every row of its Summary table says `Search`. The `Images` section appears only when the account has a campaign that is not Search. See the [report.md reference](../reference/report-md.md).

A Search-only account still has `google-ads/editor/account.csv`. It has no `editor/images/` folder.

1. Look for `No image campaign:` in the campaign rationales under Budget and bids. The plan agent must write it when image campaigns are possible and it chose none. The reason follows.
2. If there is no such line, images were not possible. Check `mads providers` for an image model, then the logo and `[app]` in `business.toml`. The steps are in [Add image and app campaigns](add-image-campaigns.md).
3. To require a format, list it in `[campaigns] formats` in `business.toml`, then generate again. Generating calls a model and spends your tokens.

#### Google refuses keywords with a policy error

Editor marks the refused keywords or ads after you post, with a message such as `Alcohol sale`. That is Google's restricted content policy, not a file format problem.

Google's help says an item flagged before posting shows an exclamation mark, and selecting it shows the policy violation. A **Request exemption** button in the edit panel applies to items flagged before posting. An item that posted and was later disapproved cannot get an exemption. The source is [Fix issues with your ads and keywords](https://support.google.com/google-ads/editor/answer/9059819).

Check that `restricted` in `business.toml` lists the category, then follow [Handle restricted categories](handle-restricted-categories.md).

#### An App or Demand Gen ad group has no ads

The campaign and ad group exist in Google Ads, but the group has no ads. The ad row was in `account.csv`, but it failed to post. An App ad needs at least one picture. A Demand Gen image ad needs its pictures and the logo. Editor takes no images from a CSV, so they are missing until you attach them.

1. In Editor, look for the ad marked with an exclamation mark and read its message. Google's help says a flagged item shows this mark, and selecting it shows the violation.
2. Attach the files listed for that ad group in the `Images` table of `report.md`. Demand Gen also needs `editor/images/logo.png`.
3. Post again.

The attach steps are in [Import into Google Ads Editor](import-into-google-ads-editor.md). Attach the pictures before you post next time.

#### Google rejects a row of a bulk CSV

This applies to the five files of `google-ads/bulk/`. `editor/account.csv` always writes money with a dot.

- Check the decimal format. The bulk files use a decimal comma when `business.language` is `pt`, `es`, `fr`, `de` or `it`. If your account reads the comma as text, set `export.decimal_comma = false` in `business.toml` and generate again. Or set it in `workspace.json` under `input.export` and run `mads export <run-dir>`. That second path calls no model.
- Compare the headers with a template from your account. See [Bulk upload format](../reference/bulk-upload-format.md).
- Check `business.locations`. mads does not verify it against Google's list. A city or a state needs the canonical Google name.
- Upload the files in numeric order.

### 5. Exit code 3, the account has errors

The missions finished, but validation found errors or a URL did not answer. mads writes `report.md` with the errors and no CSV. Each error prints as `[validate] <CODE> <path>: <message>`. On GitHub Actions it is also an annotation.

| Code | Typical cause | Fix |
|---|---|---|
| `E15` | A final or sitelink URL did not answer 2xx after up to 5 redirects. | Fix the page, or remove it from the catalog or `business.pages`. Then run `mads export <run-dir>`. For a local or private site, pass `--skip-url-check`. |
| `E05` | Ad text contains a `business.avoid` term. | Remove the term from the list, or generate again. |
| `E06` | Campaign budgets do not sum to `budget.daily`, or a campaign has less than 1.00. | Generate again. |
| `E07` | A URL is outside `business.url`, `business.pages` and the catalog. | Add the page to `business.pages` or the catalog, then generate again. |
| `E12` | A structure error, such as a duplicate name or an ad group with no keywords. | Resume, or generate again. |
| `E20` | A picture is missing. The message is `image '<id>' has no file`. | Resume with a higher `--max-images`. Pictures already made are reused. |

Every other code is in [Validation rules](../reference/validation-rules.md). Exit code 3 mostly comes from the URL check, because agents read validation errors during a mission and fix their calls.

After you fix a URL, export again. It calls no model.

```bash
mads export out/<run-id>
```

A run or export that exits `1` or `3` removes the CSV files an earlier export left in `google-ads/`, `google-ads/bulk/` and `google-ads/editor/`, and the `google-ads/automation/` folder. Pictures in `editor/images/` stay.

## Check it worked

- The command exits `0`.
- `report.md` has the line ``> Ready to import. Review the files in `google-ads/` before uploading.`` under its title.
- For an import problem, the refused rows are accepted in Editor, or you removed them on purpose.

If you are still stuck, open an issue with the command, the exit code, the failing line from the output and `report.md`. Remove secrets first.

## Next

- [Exit codes and errors](../reference/exit-codes-and-errors.md)
- [Resume or re-export a run](resume-a-run.md)
- [Limits and retries](../explanation/limits-and-retries.md)
