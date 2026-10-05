# Resume and export

This page shows how to finish a run that failed halfway and how to re-validate and re-export a run without calling a model.

## Resume a run

A run is incomplete when a mission fails after its retries. The command exits `1` and the report says so. Finished work is kept. `workspace.json` holds the account and the state of each mission.

Resume with the run directory.

```bash
mads generate --resume out/20261001-122850-b51417 --provider anthropic --model <model-id>
```

What resume does.

- Reads the input from the run directory, never from the current files. Editing `business.toml` after the run changes nothing.
- Skips every mission that is `Finished`.
- Runs the plan mission again only if it did not finish or the plan has no campaigns.
- Runs each unfinished campaign mission with a fresh context. `get_brief` shows the ad groups already built, so the agent does not redo them.
- Starts a new token budget. `--max-tokens` counts from zero.
- Runs the post-processing and export again, then rewrites `report.md` and `run.json`.

The business file is not needed. Pass the provider flags again, because the run directory does not store your key or your flags.

```text
error: cannot resume out/nope: No such file or directory (os error 2)
```

That is the error for a missing directory, with exit code `1`.

To improve a run that is already live from its Google Ads reports, use [`mads optimize`](optimize.md) instead. It creates a new run and leaves this one as it is.

## Export again without a model

`mads export` reruns cross negatives, validation, the URL check and the CSV export on `workspace.json`. No LLM runs.

```bash
mads export out/20261001-122850-b51417
```

Use it when.

- The URL check failed (exit `3`) and you fixed the page. Run it again.
- You want to skip the URL check. Pass `--skip-url-check`.
- You changed `workspace.json` by hand and want fresh files.

```bash
mads export out/20261001-122850-b51417 --skip-url-check
```

`export` refuses a run with unfinished missions.

```text
error: run has unfinished missions (campaign:vinellu-marca); run `mads generate --resume <run-dir>` first
```

Exit code `1`. Resume first, then export.

A run directory that does not exist gives exit code `1`.

```text
error: run directory not found: out/nope
```

### What export reads

`export` reads the input snapshot inside `workspace.json`, not `input/business.toml`. `input/` is a copy for your records. Editing it does not change the next export.

To change an input value such as `export.decimal_comma` or `export.status`, edit the `input.export` object in `workspace.json`, or generate again. Editing `workspace.json` is a manual repair tool. Keep a copy first.

### Limits of export

- `export` takes `--max-ad-groups` (default 50). Pass the same value you used for `generate`. A run generated with a higher limit fails `E13` on export otherwise.
- `events.ndjson` is appended. The export events follow the original events.
- A failed export (exit `3`) deletes the CSV files that an earlier export left in `google-ads/`, so the folder never holds files that do not match the report.
- The run keeps its original provider and model labels in the report. `export` reads them from `run.json`.
- `export` has no image model. It reuses the pictures in `google-ads/editor/images/`, and a missing one fails with `E20`. A failed export keeps the pictures. `generate --resume` draws only the missing ones, within `--max-images`.

## Exit codes after resume or export

The same as `generate`. See [Troubleshooting](../howto/troubleshooting.md).
