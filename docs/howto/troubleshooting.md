# Troubleshooting

This page lists every exit code, the real error messages behind it and how to fix each one.

Run `mads generate` with `-v` for info logs on stderr and `-vv` for debug. `RUST_LOG` overrides both. The run directory of `generate` has `events.ndjson` with every event, which is the first place to look. `mads init` writes no run directory. Its transcript is in `.mads/transcripts/` next to the files it writes, and a failed init prints that path.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. CSVs written. |
| `1` | Runtime failure. A provider or network error, a mission that failed after its retries, the token budget, or a missing run directory. |
| `2` | Invalid input or usage. A bad `business.toml` or `catalog.csv`, an unknown flag, a missing provider or model, an agent CLI that is missing or too old, or `mads init` files that already exist. |
| `3` | The account is invalid after post-processing. Validation errors or URL check failures. No CSV written. |

## Exit code 2

mads stops before it calls any model. The message names what is wrong.

### Provider and model

```text
error: no provider: use --provider or MADS_PROVIDER
```

Pass `--provider <name>` or set `MADS_PROVIDER`.

```text
error: unknown provider 'gpt-9000'; run `mads providers` to see the options
```

The name is not in `mads providers`. Names use dashes, such as `openai-compat`.

```text
error: --model (or MADS_MODEL) is required for anthropic
```

Every API provider needs a model.

```text
error: --base-url (or MADS_BASE_URL) is required for openai-compat
```

Pass the endpoint, for example `--base-url http://localhost:8000/v1`.

An agent CLI that is missing or too old.

```text
error: claude not found (claude): install it or point the matching MADS_*_BIN variable at it
error: claude 2.0.1 is too old: mads needs 2.1.259 or newer
```

Install the CLI, put it in `PATH`, or point `MADS_CLAUDE_BIN` (`MADS_CODEX_BIN`, `MADS_GEMINI_BIN`) at the executable. `claude` needs 2.1.259 or newer. `mads providers` shows `` `claude` not found in PATH `` for a missing default executable. See [Agent CLIs](../guides/agent-clis.md).

### Flags

```text
error: unexpected argument '--nope' found
error: the following required arguments were not provided:
  <BUSINESS>
error: invalid value '0' for '--mission-timeout <MISSION_TIMEOUT>': duration must be greater than 0
```

Run `mads generate --help`. Durations take `90s`, `15m`, `2h` or bare seconds.

### business.toml

```text
error: nope.toml: No such file or directory (os error 2)
```

The path is wrong. It is relative to your current directory.

An unknown key shows the TOML location and the allowed keys.

```text
error: TOML parse error at line 4, column 1
  |
4 | languge = "en"
  | ^^^^^^^
unknown field `languge`, expected one of `name`, `url`, `language`, `locations`, `goal`, `description`, `conversion_tracking`, `brand_terms`, `competitors`, `avoid`, `pages`
```

A missing required key.

```text
error: TOML parse error at line 8, column 1
  |
8 | [budget]
  | ^^^^^^^^
missing field `daily`
```

A bad value names the key.

```text
error: business.locations: exactly 1 location is supported in v1
error: budget.currency: must be 3 uppercase letters
error: business.url: must be an absolute http(s) URL
error: business.description: must be 20 to 4000 chars, got 5
error: budget.daily: must be greater than 0 with at most 2 decimals
```

Every key and limit is in the [business.toml reference](../reference/business-toml.md).

### catalog.csv

```text
error: c.csv: No such file or directory (os error 2)
error: catalog.csv header: required columns: name, url
error: catalog.csv row 2 url: not an absolute http(s) URL: not-a-url
error: catalog.csv row 3 url: duplicate url: https://x.com/a
error: catalog.csv row 2 third_party: expected true or false, got yes
```

Row numbers count the header as row 1. Other messages are `catalog.csv row N name: must be 1 to 120 chars`, `catalog.csv row N notes: at most 500 chars` and `more than 5000 rows`.

### mads init

```text
error: --from-url must be an absolute http(s) URL, got 'vinellu.com'
error: --daily-budget must be greater than 0 with at most 2 decimals
error: --currency must be 3 uppercase letters, such as BRL
error: ./business.toml, ./catalog.csv already exist: use --force to overwrite
```

Fix the flag, or pass `--force` to overwrite. See [Init from a URL](../guides/init-from-url.md).

## Exit code 1

### The provider call failed

The plan mission fails first, so you see it on the `[plan]` line. The run ends with `1 of 1 missions failed`.

A missing key.

```text
12:28:31 [plan] failed: Resolver error for model 'anthropic::claude-sonnet-4-5 (adapter: Anthropic)'.
Cause: ApiKeyEnvNotFound { env_name: "ANTHROPIC_API_KEY" }
```

Export the variable named in the message.

A server that is not reachable.

```text
12:28:34 [plan] failed: Web call failed for model 'ollama::llama3.1 (adapter: Ollama)'.
Cause: Reqwest error: error sending request for url (http://localhost:11434/api/chat)
```

Start the server or fix the URL. See [Run with Ollama](ollama-local.md).

Rate limits (429), timeouts (408) and 5xx responses retry 3 times inside the call, waiting 1 and 2 seconds. Other HTTP errors, such as an invalid key or an unknown model, fail at once. The message carries the provider's text, cut to 600 characters.

After you fix the cause, run `mads generate --resume <run-dir>` with the same flags. See [Resume and export](../guides/resume-and-export.md).

### A mission failed

The mission line shows the reason.

| Reason | Cause | Fix |
|---|---|---|
| `max turns` | The mission hit `--max-turns` without calling `finish`. | Raise `--max-turns`. Check `events.ndjson` for tool calls that returned errors in a loop. Use a stronger model. |
| `no progress` | The model answered three times in a row without calling a tool. | The model does not call tools well. Use another model. |
| `timeout` | The mission ran past `--mission-timeout`. | Raise `--mission-timeout`. Check provider latency. |
| `token budget exceeded` | The run passed `--max-tokens`. | Raise `--max-tokens` or set it to `0`. Resume. |
| a provider message | A fatal provider error. | See the section above. |

mads retries a failed mission with a fresh context (`--mission-retries`, default 1). The report then shows `Incomplete` and the command exits `1`. Resume to retry only the unfinished missions.

Find tool errors in `events.ndjson`.

```bash
grep '"type":"tool_finished"' out/<run-id>/events.ndjson | grep '"ok":false'
```

### An agent CLI mission failed

A CLI mission that ends without calling `finish` fails with the exit status and the tail of the CLI's stderr.

```text
[campaign:vinellu-marca] failed: claude exited with exit status: 1 before calling finish: <last stderr lines>
```

Read that tail first. It usually says what the CLI did not like. If claude is not logged in, run `claude` once and log in, or set `CLAUDE_CODE_OAUTH_TOKEN` (create it with `claude setup-token`) or `ANTHROPIC_API_KEY`. The raw stream is in `transcripts/<mission>.cli.jsonl` with `:` in the mission id written as `-`.

### An init mission failed

The init run prints `failed: <reason>` and exits `1`. No files are written. Tool errors the agent saw are in the output as `< error ...` lines.

| Code | Cause |
|---|---|
| `HOST` | The agent asked for a URL outside the site. Only the start host and its `www.` or apex sibling are allowed. IP-literal and private hosts are refused. Set `MADS_ALLOW_PRIVATE_HOSTS=1` for a local site. |
| `FETCH` | The page or sitemap request failed. The message has the HTTP status or the network error. |
| `LIMIT` | More than 30 pages fetched, or the tool call budget ran out. |
| `E07` | A business page or catalog URL was not fetched or listed in the sitemap during the run. |
| `E13` | The catalog would pass `--catalog-limit`. |

### Resume and export errors

```text
error: cannot resume out/nope: No such file or directory (os error 2)
error: run has unfinished missions (campaign:vinellu-marca); run `mads generate --resume <run-dir>` first
error: run directory not found: out/nope
```

Pass the run directory (the folder with `workspace.json`), not the `out` folder. `export` needs every mission finished. Resume first.

## Exit code 3

The missions finished, but the account has errors, or a URL did not answer. mads writes `report.md` with the errors and no CSV. The exit line looks like this.

```text
12:29:06 [run] failed (exit 3): 104000 in, 18000 out tokens, cost n/a, 3 missions finished
```

Each error prints as `CODE path: message`.

```text
12:28:58 [validate] 7 errors, 24 warnings
12:28:58 [validate] E06 campaigns: campaign budgets sum to 5000 cents, budget.daily is 6000 cents
12:28:58 [validate] E05 campaigns[0].assets.callouts[4]: text contains avoided term 'reviews reais'
```

On GitHub Actions each one is also an annotation.

| Code | Typical cause | Fix |
|---|---|---|
| `E15` | A final or sitelink URL did not answer 2xx after up to 5 redirects. The message is `URL check failed: HTTP 404` or `URL check failed: unreachable`, and the path is the URL. | Fix the page or remove it from the catalog or `business.pages`. Then `mads export <run-dir>`. For a local or private site use `--skip-url-check`. |
| `E05` | Ad text contains a `business.avoid` term. | Remove the term from the list, or generate again. |
| `E06` | Campaign budgets do not sum to `budget.daily`, or a campaign has less than 1.00. | Generate again. |
| `E07` | A URL is outside `business.url`, `business.pages` and the catalog. | Add the page to `business.pages` or the catalog. Generate again. |
| `E12` | Structure error. A duplicate name, a missing planned ad group, an ad group with no keywords or no assets. | Resume, or generate again. |
| other `E` codes | See [Validation rules](../reference/validation-rules.md). | |

Errors that tools catch during a mission rarely reach exit code 3. Agents read those errors and fix their calls. Exit code 3 mostly comes from the URL check, and from a run that is validated again with different input.

To skip the URL check on the first run, pass `--skip-url-check`. The report then says `skipped` under URL check.

### An export with a URL failure

```bash
mads export out/<run-id>
```

Run it again after you fix the page. It does not call a model. A failed export removes the CSV files an earlier export wrote to `google-ads/`, so the folder never holds files that do not match the report.

## The run looks right but Google rejects a row

- Compare headers with a template from your account. See [the bulk upload format](google-ads-bulk-upload-format.md).
- Check the decimal format. If Google reads `1,50` as text, set `export.decimal_comma = false` in `business.toml` and generate again. Or set it in `workspace.json` under `input.export` and run `mads export`.
- Check the location name. mads does not verify `business.locations` against Google's list.
- Upload in numeric order.

## Still stuck

Open an issue with the command, the exit code, the failing line from the output and `report.md`. Remove secrets first.
