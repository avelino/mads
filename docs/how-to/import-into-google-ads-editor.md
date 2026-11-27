# Import into Google Ads Editor

Load a finished run into Google Ads Editor, attach its pictures and post it to your account with every campaign paused.

## Before you start

- A finished run whose `report.md` status line says `Ready to import`. See [Review a run](review-a-run.md).
- Google Ads Editor installed and signed in to the account. See [Prepare your Google Ads account](prepare-google-ads.md).
- `export.status` in `business.toml` left at `Paused`, its default. New campaigns then arrive paused and nothing serves until you enable it.

## Steps

1. **Pick the file.** Import `out/<run>/google-ads/editor/account.csv` and nothing else. It holds every campaign type, Search included. The five CSVs in `google-ads/bulk/` are for the web bulk upload only. Importing them into Editor as well adds each Search campaign twice. `report.md` opens with a notice that says the same. For the web upload, see [Upload the bulk CSVs](upload-bulk-csvs.md).

2. **On an optimize run, read the `Changes` section of `report.md`.** It lists what the import pauses, adds or leaves alone. See [Editor file](../reference/editor-file.md#rows-added-by-an-optimize-run) for every row it adds, and [Safe on a live account](../explanation/safe-on-a-live-account.md) for the reasoning. Two rules surprise people.
   - A campaign the plan keeps comes in `Enabled`, even when it is paused in the account. The plan drops what should stay off.
   - An enabled ad group that an older import left in a kept campaign gets a `Paused` row too, next to the groups the new run dropped.

3. **Get the recent changes.** In the toolbar, click Get recent changes, so Editor starts from what is live. Google describes the options in [Review recent account changes](https://support.google.com/google-ads/editor/answer/30582?hl=en).

4. **Import the file.** Follow Google's [Import a CSV file](https://support.google.com/google-ads/editor/answer/30564?hl=en).
   1. Select Account, then Import. The item is grayed out while other proposed changes are pending, so keep or reject them first.
   2. Choose From file ... and pick `google-ads/editor/account.csv`.
   3. Check the column headers, then select Import.
   4. Review the imported changes. Errors and warnings show up there.
   5. Compare the counts of campaigns, ad groups, keywords and ads with the Summary table in `report.md`.
   6. Select Review imported changes, then Keep proposed changes.

5. **Attach the pictures.** Skip this step when the run has no App, Demand Gen or Performance Max campaign. Then `editor/images/` does not exist. Editor neither exports nor imports the link between an ad and its images, so you attach them.
   1. Select Account, then Import, then Import image assets from files. Click Select folder, pick `google-ads/editor/images`, click Open, then Import. Google documents this in [How to import and export image assets in Editor](https://support.google.com/google-ads/editor/answer/13372857?hl=en).
   2. The pictures sit in one subfolder per campaign, which the account does not have. If Editor asks what to do with them, import them to the root folder. Skipping them leaves the pictures out.
   3. Open the Images table in `report.md`. It lists every ad and asset group with its files.
   4. For each row, open that ad or asset group in Editor and attach those files. The report lists the logo too when the run has one.

   Do this before you post. An App or Demand Gen ad without its pictures fails to post, and its ad group goes up empty with no active ads. Editor adds each file under its own name, which carries the ad group, so no picture overwrites another. See [Add image and app campaigns](add-image-campaigns.md) for how the pictures are made.

6. **Post the changes.** Fix every error Editor lists first. If an ad fails to post, filter Ads by errors in Editor, attach what is missing and post again. See [Troubleshoot](troubleshoot.md).

If Google refuses a keyword or an ad in a restricted category, `report.md` has an exception request to paste. See [Handle restricted categories](handle-restricted-categories.md).

Rows for Search, App and Demand Gen went through a real import in Editor 2.13.3 with no error. Performance Max rows follow Google's documented headers and have not gone through a real import. Read the preview with care on those. The full list is in [Editor file](../reference/editor-file.md#verification-level).

## Check it worked

Before you post, the counts in the import preview match the Summary table of the run. This is the Summary of a run with three Search campaigns.

Example run (replay provider, no model was called, URL check skipped with `--skip-url-check`).

```markdown
> Ready to import. Review the files in `google-ads/` before uploading.

## Summary

| Campaign | Format | Intent | Daily budget | Bidding | Groups | Keywords |
|---|---|---|---|---|---|---|
| Vinellu - Marca | Search | brand | 8,00 BRL | Manual CPC | 1 | 9 |
| Vinellu - Rotulos | Search | catalog | 30,00 BRL | Manual CPC | 6 | 71 |
| Vinellu - App de vinhos | Search | generic | 12,00 BRL | Manual CPC | 3 | 24 |
```

After you post, open the account in Google Ads. Every new campaign is paused. A campaign you kept from an earlier run shows as enabled, as step 2 says. In an optimize run, each item the new plan dropped is paused, and nothing was deleted.

An optimize run leaves some things to you.

- A responsive search ad with new texts comes in next to the old one. The old one stays enabled. Pause it by hand after you check the preview.
- A negative keyword the new run dropped stays in the account. Remove it by hand.
- A bid strategy you changed in Google Ads stays as it is. The strategy cell of the file is empty for that campaign.

## Without Google Ads Editor

`--layout drive-folders` writes an ads automation package instead of `account.csv`. See [Use the automation package](use-the-automation-package.md).

## Next

- [Go live](go-live.md)
- [Optimize a live account](optimize-a-live-account.md)
- [Troubleshoot](troubleshoot.md)
