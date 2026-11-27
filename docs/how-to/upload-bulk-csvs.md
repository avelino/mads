# Upload the bulk CSVs

Upload the five CSV files of `google-ads/bulk/` through the Google Ads web bulk upload, for a Search-only account.

> **Pick the path before you upload.** A Search-only first run can use the bulk upload. Every `mads optimize` run goes through Google Ads Editor, and the bulk files are never the right input for a live account. If you run mads in production, start on Editor with [Import into Google Ads Editor](import-into-google-ads-editor.md) and skip this page.

Never import both sets. `google-ads/editor/account.csv` holds the same Search campaigns as the bulk files, so using both creates every Search campaign twice.

## Before you start

- A finished run with the default layout and a status of `Ready to import`. See [Review a run](review-a-run.md).
- `google-ads/bulk/` in the run directory holds files 1 to 5. With `--layout drive-folders` mads writes no `bulk/` folder, see [Use the automation package](use-the-automation-package.md).
- Search campaigns only. Performance Max, Demand Gen and App campaigns are not in these files. They exist only in the Editor file, see [Add image campaigns](add-image-campaigns.md).
- A Google Ads account where you may upload files. If you have not prepared it yet, see [Prepare Google Ads](prepare-google-ads.md).
- A decision on number format. The files use comma decimals when `business.language` is `pt`, `es`, `fr`, `de` or `it`. If your account uses another format, set `export.decimal_comma` in `business.toml`, then run `mads generate` again, or edit `input.export` in `workspace.json` and run `mads export <run-dir>`. See [Bulk upload format](../reference/bulk-upload-format.md#file-rules).

Example run (replay provider, no model was called, URL check skipped with `--skip-url-check`). The run wrote these files.

```text
10:21:18 [out] .../google-ads/bulk/1-campaign.csv
10:21:18 [out] .../google-ads/bulk/2-ad-groups.csv
10:21:18 [out] .../google-ads/bulk/3-keywords.csv
10:21:18 [out] .../google-ads/bulk/4-negative-keywords.csv
10:21:18 [out] .../google-ads/bulk/5-responsive-search-ads.csv
10:21:18 [out] .../google-ads/editor/account.csv
10:21:18 [out] .../report.md
```

Only the five `bulk/` files go through this page.

## Steps

1. Read the report. Open `report.md` in the run directory and follow [Review a run](review-a-run.md). Fix errors before you upload anything.

2. Check the files by hand. Look at what the validator cannot judge.
   - Does every headline come from facts you gave? mads blocks `avoid` terms and `!`. It does not fact-check.
   - Read the ads in your language.
   - Look at every `W01` and decide per trademark name.
   - Do the negatives block searches you want?
   - Does the final URL of each ad group (last column of file 5) fit its keywords?

   You can edit the CSVs by hand. If you change many things, change the input and generate again.

3. Open the bulk upload page in Google Ads. The menu path is in the How to import section of `report.md`, which mads writes. For the labels Google shows today, use [Google Ads Help](https://support.google.com/google-ads).

   Example run (replay provider, no model was called):

   ```markdown
   ### Google Ads on the web, Search campaigns only

   1. Open Google Ads, then Tools, Bulk actions, Uploads.
   2. Upload files 1 to 5 of `google-ads/bulk/` in numeric order.
   3. Preview the changes, fix anything Google flags, then apply.
   ```

4. Upload the files in numeric order, one at a time.

   | Order | File |
   |---|---|
   | 1 | `bulk/1-campaign.csv` |
   | 2 | `bulk/2-ad-groups.csv` |
   | 3 | `bulk/3-keywords.csv` |
   | 4 | `bulk/4-negative-keywords.csv` |
   | 5 | `bulk/5-responsive-search-ads.csv` |

   Rows point to their parents by name, never by ID. An ad group can only land in a campaign that already exists, so the campaign file goes first.

   Example run (replay provider, no model was called). The first rows of files 1 and 2.

   ```csv
   Row Type,Action,Campaign status,Campaign,Campaign type,Networks,Budget,Budget type,Bid strategy type,Language,Location,Final URL suffix,EU political ads
   Campaign,Add,Paused,Vinellu - Marca,Search,Google search,8,Daily,Manual CPC,pt,Brazil,utm_source=google&utm_medium=cpc&utm_campaign=vinellu-marca&utm_content={adgroupid}&utm_term={keyword},No
   ```

   ```csv
   Row Type,Action,Ad group status,Campaign,Ad group,Ad group type,Default max. CPC
   Ad group,Add,Enabled,Vinellu - Marca,marca,Standard,"0,80"
   Ad group,Add,Enabled,Vinellu - Rotulos,rotulo-alamos,Standard,"1,50"
   ```

5. For each file, read the preview. Google lists what it will add and flags rows it cannot accept. Fix what it flags, then apply.

6. If Google refuses a keyword or an ad, the cause is often a restricted category. The `Restricted categories` section of `report.md` has the exception text to paste. See [Handle restricted categories](handle-restricted-categories.md).

## Check it worked

- All five files applied with no flagged rows.
- The campaigns exist in Google Ads, and each one is `Paused`. `export.status` defaults to `Paused`, so nothing serves yet. The ad groups, keywords and ads are `Enabled` inside the paused campaign.
- The campaign, ad group and keyword counts match the Summary table of `report.md`.

Sitelinks, callouts and structured snippets are validated but not in the files. Add them in the Google Ads interface. See [Limitations](../reference/limitations.md).

## If this fails

- Google flags the budget or bid cells. Check the decimal setting in Before you start.
- Rows fail because the parent is missing. You skipped a file or uploaded out of order. Upload the missing earlier file first.
- The report status is not `Ready to import`. No CSV was written by that run. See [Troubleshoot](troubleshoot.md) and [Resume a run](resume-a-run.md).

## Next

- [Go live](go-live.md)
- [Import into Google Ads Editor](import-into-google-ads-editor.md)
- [Bulk upload format](../reference/bulk-upload-format.md)
