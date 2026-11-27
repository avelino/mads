# Resume or re-export a run

Finish a run that stopped halfway with `mads generate --resume`, or rebuild its files with `mads export` without calling a model.

Pick one.

- A mission failed and the report says `Incomplete`. Resume the run.
- The run finished but the export stopped, or you changed a page or `workspace.json`. Export again.

## Before you start

- A run directory under `out/` from `mads generate`. The layout is in [Output files](../reference/output-files.md).
- For resume only: the provider flags you used for the run. The run directory stores no key and no flags. See [Choose a provider](choose-a-provider.md).
- The run directory must have a readable `workspace.json`. Both commands read it, never `input/`.

Resume calls a model for each unfinished mission and spends your tokens. Export calls none.

## Resume a failed run

1. Open `report.md` in the run directory. The status line reads `Incomplete: some missions failed.` The report lists the failed missions and why. The failure reasons are in [Limits and retries](../explanation/limits-and-retries.md).

2. Resume with the run directory and the same provider flags.

   ```bash
   mads generate --resume out/20261001-122850-b51417 --provider anthropic --model <model-id>
   ```

   You do not pass `business.toml`. Resume reads the input saved in `workspace.json`, so editing `business.toml` after the run changes nothing.

3. Wait for the run to end. Resume does this:

   - skips every mission that is `Finished`
   - runs the plan mission again only if it did not finish or the plan has no campaigns
   - runs each unfinished campaign mission again, and the agent sees the ad groups already built in `get_brief`
   - starts a new `--max-tokens` budget from zero
   - runs the cross negatives, validation, URL check and export again, then rewrites `report.md` and `run.json`

   Without `--layout`, resume writes the layout saved in `run.json`. Pictures already in `google-ads/editor/images/` are reused and only the missing ones are drawn, within `--max-images`.

If this fails with `cannot resume`, see [Troubleshoot](troubleshoot.md).

## Export again without a model

`mads export` reruns cross negatives, validation, the URL check and the file export on `workspace.json`. Use it when:

- the URL check failed (exit `3`) and you fixed the page
- you want to skip the URL check
- you edited `workspace.json` by hand and want fresh files

1. Run it with the run directory.

   ```bash
   mads export out/20261001-122850-b51417
   ```

2. Add flags only when you need them.

   ```bash
   mads export out/20261001-122850-b51417 --skip-url-check
   mads export out/20261001-122850-b51417 --layout drive-folders
   ```

   Pass `--max-ad-groups` with the value you used for `generate`. The default is `50`. A run built with a higher limit fails `E13` otherwise.

3. To change an input value such as `export.decimal_comma` or `export.status`, edit the `input.export` object in `workspace.json` first. Keep a copy of the file. Editing `input/business.toml` does nothing, because `input/` is a record.

Export refuses a run with an unfinished mission. Resume it first.

Example run (replay provider, no model was called). This is `mads export` on a finished run, with `--skip-url-check`.

```text
...
10:24:49 [run] url-check: skipped
10:24:49 [run] export: writing files
10:24:49 [out] generate/20261127-102117-a7fb01/google-ads/bulk/1-campaign.csv
10:24:49 [out] generate/20261127-102117-a7fb01/google-ads/editor/account.csv
10:24:49 [out] generate/20261127-102117-a7fb01/report.md
10:24:49 [run] done (exit 0): 135000 in, 21800 out tokens, cost n/a, 4 missions finished
```

The start of the output (the validation lines) is left out, and the listing is cut to three of the seven files. The token numbers come from the original run. Export read them from the saved state and called no model.

What a failed export leaves behind, and what `--layout` removes, is in [Output files](../reference/output-files.md). The picture rule is in [Add image campaigns](add-image-campaigns.md).

## Check it worked

- The command exits `0`.
- The status line in `report.md` reads `Ready to import. Review the files in google-ads/ before uploading.`
- After a resume, exit `1` with an `Incomplete` status means a mission failed again. Resume again.
- Exit `3` means the account is invalid or a URL failed. No CSV is written. See [Exit codes and errors](../reference/exit-codes-and-errors.md).

`events.ndjson` is appended, so the new events follow the original ones.

## Next

- [Review a run](review-a-run.md)
- [Troubleshoot](troubleshoot.md)
- [Exit codes and errors](../reference/exit-codes-and-errors.md)
