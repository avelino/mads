# Output files

This page describes every file in a run directory, the events in `events.ndjson`, `workspace.json` and `run.json`.

## Run directory

`mads generate` creates `<out>/<YYYYMMDD-HHMMSS>-<6 hex chars>/`. The timestamp is UTC.

```text
out/20261001-122850-b51417/
  run.json                       status, provider, model, exit code, totals
  workspace.json                 input, account and mission state
  events.ndjson                  every event
  report.md                      the report
  input/
    business.toml                copy of the file you passed
    catalog.csv                  copy of the catalog, under its original file name
    logo.png                     copy of [brand] logo, when set
  google-ads/                    one folder per ad platform
    1-campaign.csv               files 1 to 5: Search campaigns only
    2-ad-groups.csv
    3-keywords.csv
    4-negative-keywords.csv
    5-responsive-search-ads.csv
    editor/                      the whole account, for Google Ads Editor
      account.csv                UTF-16, tab separated, as Editor exports
      images/
        logo.png
        <campaign>/<asset-group>-<image-id>.jpg
    drive/                       only with --layout drive-folders
      B - Estrutural/
        B1-campanhas.csv
        B2-status-campanha.csv
        B3-grupos.csv
        B4-keywords.csv
        B5-anuncios.csv
        B6-utm.csv
        B7-negativas.csv
        B8-extensao-app.csv      omitted when google_ads.app_id is empty
        LEIA-ME.md               UTF-8, Portuguese paste order
  transcripts/                   one file per mission
```

- `google-ads/` has no CSV when the run exits `1` or `3`. That includes `drive/`. Pictures in `editor/images/` stay: they cost money and the next `--resume` or `export` reuses them.
- Files 1 to 5 hold Search campaigns only, for the web bulk upload, and are not written when the account has none. `editor/account.csv` holds every campaign. Import one or the other, never both. `editor/images/` exists only when the account has an image or App campaign, and its files are attached by hand in Editor. See [Image campaigns](../guides/image-campaigns.md#import).
- `--layout drive-folders` writes `drive/B - Estrutural/` and does not write files 1 to 5 or `editor/account.csv`. Re-exporting a bulk run with this layout removes an empty `google-ads/editor/` directory. The directory stays when `editor/images/` still holds pictures. The CSVs are UTF-16 LE with a BOM and tab separators, the same encoding as `editor/account.csv`. `LEIA-ME.md` is UTF-8. A later export with the bulk layout removes the drive folder. See [Drive folder layout](../howto/google-ads-editor-drive-folders.md).
- `workspace.json` points `input.logo` at the copy in `input/`, so the run does not depend on where the logo was. Each ad platform gets its own folder, so other platforms will not mix with these files.
- `input/` is a record. `export` and `--resume` read `workspace.json`, not `input/`.
- `transcripts/` has one file per mission: `<mission>.jsonl` for API providers (every message) and `<mission>.cli.jsonl` for agent CLIs (the raw stream). A `:` in a mission id becomes `-`, so `campaign:vinellu-catalogo` is `campaign-vinellu-catalogo.jsonl`.
- `events.ndjson`, `run.json` and `report.md` are written again by `--resume` and `export`. `events.ndjson` is appended to, the others are replaced.
- A copy of the catalog keeps the file name from `catalog.file`. If your catalog is `data/wines.csv`, the copy is `input/wines.csv`.

The `out` directory is in `.gitignore` of this repository.

## events.ndjson

One JSON object per line. `--format json` prints the same lines on stdout.

```json
{"seq":2,"ts":"2026-10-01T12:28:00.195817Z","event":{"type":"mission_started","mission":"plan","attempt":1}}
```

| Field | Meaning |
|---|---|
| `seq` | Sequence number, from 1, assigned in order of emission |
| `ts` | RFC 3339 timestamp, UTC |
| `event` | The event. Its `type` field names the kind. |

### Event types

| `type` | Fields | When |
|---|---|---|
| `run_started` | `run_id`, `run_dir`, `provider`, `model` | First event. `model` is null when unset. |
| `mission_started` | `mission`, `attempt` | A mission attempt begins. |
| `agent_text` | `mission`, `text` | The model sent text with a response. |
| `tool_called` | `mission`, `tool`, `summary` | A tool call starts. |
| `tool_finished` | `mission`, `tool`, `ok`, `summary` | A tool call ends. `ok` is false when the tool refused the call. |
| `usage` | `mission`, `input_tokens`, `output_tokens`, `cost_usd` | After every model response. `cost_usd` is null unless the provider reports it. |
| `mission_finished` | `mission`, `ok`, `reason` | A mission attempt ends. `reason` is null on success. |
| `step` | `name`, `detail` | Post-processing steps: `cross-negatives`, `images`, `validate`, `url-check`, `export`. `images` reports what will be generated and every failed picture. |
| `validation` | `errors`, `warnings` | The issues. Each issue has `code`, `severity`, `path` and `message`. |
| `url_checked` | `url`, `status`, `ok` | One per URL. `status` is null when the request failed. |
| `artifact_written` | `path` | A CSV, a picture or `report.md` was written. |
| `run_finished` | `ok`, `exit_code`, `totals` | Last event. |

Mission ids are `plan` and `campaign:<slug>`, where the slug is `slugify(campaign name)`.

`totals` has `input_tokens`, `output_tokens`, `cost_usd`, `missions` and `failed_missions`.

### Reading it

Count failed tool calls.

```bash
grep -c '"type":"tool_finished".*"ok":false' out/<run-id>/events.ndjson
```

List the URL check.

```bash
grep '"type":"url_checked"' out/<run-id>/events.ndjson
```

## workspace.json

The state of the run. It is saved after every successful mutating tool call (write to `workspace.json.tmp`, then rename), so a crash never leaves half a file.

```json
{
  "version": 1,
  "input": { "business": {}, "budget": {}, "export": {}, "catalog": [] },
  "account": { "brand_kit": {}, "campaigns": [] },
  "missions": {
    "plan": { "status": "finished", "attempts": 1, "usage": { "input_tokens": 26000, "output_tokens": 4500, "cost_usd": null } }
  }
}
```

| Field | Content |
|---|---|
| `version` | Schema version, `1`. Another value fails to load. |
| `input` | The parsed business, budget, export settings and catalog. `--resume` and `export` use this. |
| `account` | The brand kit and the campaigns with their planned ad groups, ad groups, negatives and assets. |
| `missions` | One entry per mission id. `status` is `pending`, `running`, `finished` or `{"failed": {"reason": "..."}}`. |
| `live` | Only in a `mads optimize` run. `baseline` is the account of the run it started from, `performance` the digest of the reports. Absent otherwise. See [Optimize a live account](../guides/optimize.md). |

Money in `account` is in cents (`5000` for 50.00). Enums are snake_case strings.

## run.json

A short summary written at the end.

```json
{
  "version": 1,
  "run_id": "20261001-122850-b51417",
  "provider": "anthropic",
  "model": "<model-id>",
  "status": "success",
  "exit_code": 0,
  "totals": {
    "input_tokens": 104000,
    "output_tokens": 18000,
    "cost_usd": null,
    "missions": 3,
    "failed_missions": 0
  }
}
```

`status` is `success` (exit `0`), `invalid` (exit `3`) or `incomplete` (exit `1`).

## report.md

Sections in order: header, status line, Summary, Budget and bids, Validation, URL check, Usage, How to import, After launch. A run of `mads optimize` adds Performance data after Summary and Changes after Budget and bids, and its How to import keeps only Google Ads Editor. See [Review and import](../guides/review-and-import.md).
