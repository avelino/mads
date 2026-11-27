# Output files

Look up every file a run directory holds, what writes it, and the fields of `events.ndjson`, `workspace.json` and `run.json`.

## Run directory

`mads generate` and `mads optimize` each create `<out>/<YYYYMMDD-HHMMSS>-<6 hex chars>/`. The timestamp is UTC and `--out` defaults to `out`. `--resume` and `mads export` reuse an existing directory. `mads init` makes no run directory.

```text
out/20261001-122850-b51417/
  run.json
  workspace.json
  events.ndjson
  report.md
  input/
    business.toml
    <catalog file name>
    <logo file name>
  google-ads/
    bulk/
      1-campaign.csv
      2-ad-groups.csv
      3-keywords.csv
      4-negative-keywords.csv
      5-responsive-search-ads.csv
    editor/
      account.csv
      images/
        logo.<ext>
        <campaign slug>/<asset group>-<image id>.jpg
    automation/
      <Business> Ads Automation/
        0. Feed.xlsx
        README.xlsx
        daily-sync.js
        Daily/
        Structure/
        Images/
  transcripts/
```

| Path | Written by | Content |
|---|---|---|
| `run.json` | every run, `--resume`, `export` | Status, provider, model, layout, exit code, totals. Replaced each time. |
| `workspace.json` | every tool call that changes state | The whole state of the run. |
| `events.ndjson` | every command that starts a run | One JSON line per event. Appended to, never replaced. |
| `report.md` | every run, `--resume`, `export` | The run report. Replaced each time. Sections are in [report.md](report-md.md). |
| `input/business.toml` | `generate` | Copy of the file you passed. |
| `input/<catalog file name>` | `generate` | Copy of the catalog, under the file name from `catalog.file`. `data/wines.csv` becomes `input/wines.csv`. |
| `input/<logo file name>` | `generate` | Copy of `[brand] logo`, under its own file name. `workspace.json` points `input.logo` at this copy. |
| `google-ads/bulk/` | export, bulk layout | Files 1 to 5. Search campaigns only. Absent when the account has no Search campaign. See [Bulk upload format](bulk-upload-format.md). |
| `google-ads/editor/account.csv` | export, bulk layout | Every campaign of the account. See [Google Ads Editor file](editor-file.md). |
| `google-ads/editor/images/` | image step | The pictures and the logo. Present when an image or App campaign has pictures. |
| `google-ads/automation/` | export, `--layout drive-folders` | The ads automation package. See [Ads automation package](automation-package.md). |
| `transcripts/` | every mission | One file per mission. |

## What each command writes

| Command | Run directory | `input/` |
|---|---|---|
| `generate` | New. | Written from your files. |
| `generate --resume <dir>` | Existing. | Left as it is. |
| `export <dir>` | Existing. | Left as it is. |
| `optimize <dir>` | New. | Copied from the base run. |

`optimize` also copies `google-ads/editor/images/` from the base run into the new one and writes a `workspace.json` that has the `live` field. It never changes the base run.

`export` and `--resume` read `workspace.json`, not `input/`. `input/` is a record.

## Exit codes and the files

| Exit code | `run.json` status | `google-ads/` CSVs and workbooks |
|---|---|---|
| `0` | `success` | Written. |
| `3` | `invalid` | None. |
| `1` | `incomplete` | None. |

Before every export, mads removes the CSVs of the earlier export. The files it removes are `*.csv` directly in `google-ads/`, `bulk/` (the folder too), `editor/` (the folder stays when `editor/images/` has files), and the whole `automation/` folder. Pictures in `editor/images/` stay, so the next `--resume` or `export` reuses them.

## Layouts

`--layout` picks the file set the export writes.

| Layout | Writes | Does not write |
|---|---|---|
| `bulk` (default) | `bulk/` and `editor/account.csv` | `automation/` |
| `drive-folders` | `automation/<Business> Ads Automation/` | `bulk/`, `editor/account.csv` |

- A later export with the other layout removes what the earlier one wrote, as the cleanup above says.
- `<Business>` is `business.name` without `/ \ : * ? " < > |` and control characters. Runs of spaces collapse to one. An empty name gives `Ads Automation`.
- `Images/` inside the package holds a copy of each picture under `<campaign slug>/<file>` and the logo as `logo.<ext>`. The pictures stay in `editor/images/` too.
- The package files are `0. Feed.xlsx`, `README.xlsx`, `daily-sync.js`, `Daily/D1` to `D7` and `Structure/S1` to `S7`. `Structure/S8. App Extension.xlsx` exists only when `google_ads.app_id` is set.

## transcripts/

| File | Written for | Content |
|---|---|---|
| `<mission>.jsonl` | API providers | Every message of the mission. |
| `<mission>.cli.jsonl` | Agent CLIs | The raw stream of the CLI. |

Mission ids are `plan` and `campaign:<slug>`. A `:` in the id becomes `-`, so `campaign:vinellu-catalogo` is `campaign-vinellu-catalogo.jsonl`.

## events.ndjson

One JSON object per line. `--format json` prints the same lines on stdout.

```json
{"seq":2,"ts":"2026-10-01T12:28:00.195817Z","event":{"type":"mission_started","mission":"plan","attempt":1}}
```

| Field | Content |
|---|---|
| `seq` | Sequence number, from 1 in each command, in order of emission. |
| `ts` | RFC 3339 timestamp, UTC. |
| `event` | The event. Its `type` field names the kind. |

### Event types

| `type` | Fields | When |
|---|---|---|
| `run_started` | `run_id`, `run_dir`, `provider`, `model` | First event. `model` is null when unset. |
| `mission_started` | `mission`, `attempt` | A mission attempt begins. |
| `agent_text` | `mission`, `text` | The model sent text with a response. |
| `tool_called` | `mission`, `tool`, `summary` | A tool call starts. |
| `tool_finished` | `mission`, `tool`, `ok`, `summary` | A tool call ends. `ok` is false when the tool refused the call. |
| `usage` | `mission`, `input_tokens`, `output_tokens`, `cost_usd` | After a model response. `cost_usd` is null unless the provider reports it. |
| `mission_finished` | `mission`, `ok`, `reason` | A mission attempt ends. `reason` is null on success. |
| `step` | `name`, `detail` | A post-processing step. |
| `validation` | `errors`, `warnings` | The issues. Each has `code`, `severity`, `path` and `message`. Emitted again when the URL check adds errors. |
| `url_checked` | `url`, `status`, `ok` | One per URL. `status` is null when the request failed. |
| `artifact_written` | `path` | A CSV, workbook, script, picture or `report.md` was written. |
| `run_finished` | `ok`, `exit_code`, `totals` | Last event. |

`step` names are `cross-negatives`, `images`, `validate`, `url-check` and `export`. `images` reports what will be generated and every failed picture. `url-check` has the detail `skipped` with `--skip-url-check`.

`totals` has `input_tokens`, `output_tokens`, `cost_usd`, `missions` and `failed_missions`.

`--resume` and `export` append to the same `events.ndjson`, so one file can hold several `run_started` and `run_finished` pairs.

## workspace.json

The state of the run. Every write goes to `workspace.json.tmp` first and is then renamed, so a crash never leaves half a file.

```json
{
  "version": 1,
  "input": {},
  "account": { "brand_kit": {}, "campaigns": [] },
  "missions": {
    "plan": { "status": "finished", "attempts": 1, "usage": { "input_tokens": 26000, "output_tokens": 4500, "cost_usd": null } }
  }
}
```

| Field | Content |
|---|---|
| `version` | Schema version, `1`. Any other value fails to load. |
| `input` | The parsed input: `business`, `budget`, `export`, `catalog`, `research`, `logo`, `formats`, `design`, `focus`, `strategy`, `apps` and `google_ads`. Unset ones are empty, null or `[]`. `--resume` and `export` read this. |
| `account` | The brand kit and the campaigns, with their planned ad groups, ad groups, asset groups, negatives and assets. |
| `missions` | One entry per mission id. `status` is `pending`, `running`, `finished` or `{"failed": {"reason": "..."}}`. Also `attempts` and `usage`. |
| `live` | Only in a `mads optimize` run. `baseline` is the account of the run it started from. `performance` is the digest of the reports. Absent otherwise. |

Money in `account` is in cents (`5000` for 50.00). Enums are snake_case strings.

## run.json

```json
{
  "version": 1,
  "run_id": "20261001-122850-b51417",
  "provider": "anthropic",
  "model": "<model-id>",
  "layout": "bulk",
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

| Field | Content |
|---|---|
| `version` | `1`. |
| `run_id` | The name of the run directory. |
| `provider`, `model` | As used by the run. `export` takes them from the earlier `run.json`. `model` is null when unset. |
| `layout` | `bulk` or `drive-folders`. `export` and `--resume` read it when `--layout` is not given. A `run.json` without it reads as `bulk`. |
| `status` | `success` (exit `0`), `invalid` (exit `3`) or `incomplete` (exit `1`). |
| `exit_code` | `0`, `1` or `3`. See [Exit codes and errors](exit-codes-and-errors.md). |
| `totals` | The same object as in `run_finished`. |

## Next

- [report.md](report-md.md)
- [Google Ads Editor file](editor-file.md)
- [Resume or re-export a run](../how-to/resume-a-run.md)
