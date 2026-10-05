# Review and import

This page shows how to read `report.md`, check the CSVs and upload them to Google Ads without surprises.

## Read report.md first

The report is in the run directory next to `google-ads/`. It always has these sections, in this order.

### Header and status

The first line is the business name. The second line is the run id, the date, the provider and the model. A status line follows, one of three.

| Status line | Exit code | What it means |
|---|---|---|
| `Ready to import. Review the files in google-ads/ before uploading.` | 0 | Files written. |
| `Not exported: the account has errors. Fix them and run mads export <run-dir>, or generate again.` | 3 | Validation or URL check failed. No CSV written by this run. |
| `Incomplete: some missions failed. Run mads generate --resume <run-dir> to retry only those.` | 1 | At least one mission failed. |

### Summary

One row per campaign with intent, daily budget, bidding, ad group count and keyword count. Check that the split matches what you want and that the totals match your `budget.daily`.

The daily budget prints with two decimals (`10,00 BRL`), while the CSV prints `10`.

### Budget and bids

The agent's rationale for each campaign, and the CPC of each ad group with its reason. The section says the numbers are estimates. mads has no auction data. Compare each CPC with the Keyword Planner and your own history.

### Validation

Errors and warnings grouped by code with a count and up to 5 examples each. More show as `and N more`. Each example has the path into the account, for example `campaigns[1].ad_groups[0].rsa.headlines[0]`. The first index is the campaign in plan order. Cross-negative notes appear under Notes.

```markdown
**W04 (1)**

- `campaigns[0].ad_groups[0].rsa`: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
```

Warnings never block the export. Read them anyway. See [Validation rules](../reference/validation-rules.md) for every code.

### URL check

One row per distinct final URL and sitelink URL with the HTTP status after redirects. With `--skip-url-check` the section says `skipped`. A skipped check means mads did not verify that your pages load.

### Usage

Attempts, input tokens, output tokens, cost and result for each mission, then totals. A failed mission shows its reason, such as `failed: max turns`. Cost is `n/a` for API providers.

### How to import

A short list of the upload steps. The next section expands it.

### After launch

What to check once the campaigns serve, and what to export for the next run.

- After 2 days, look at the [keyword status](https://support.google.com/google-ads/answer/2453978?hl=en) in Google Ads. `Below first page bid` means the ad is not reaching the first page of results, usually because of Ad Rank. `Rarely shown` means a low Quality Score. Add the [First page bid estimate](https://support.google.com/google-ads/answer/105665?hl=en) column and raise the CPC to it, or pause the keyword.
- When Search campaigns run without `conversion_tracking`, the section says so. Google Ads then shows clicks and cost but not which searches bring customers.
- After 14 days, export search terms, keywords with Quality Score and bid estimates, campaigns by day and asset performance as CSV into one folder, and run [`mads optimize`](optimize.md). Keep the campaign and ad group names: they tie the reports to the run.

## Review the CSVs

Open the files in `google-ads/`. Check the things the validator cannot judge.

- **Claims.** Does every headline come from facts you gave? mads blocks `avoid` terms and `!`. It does not fact-check.
- **Tone and language.** Read the ads in your language.
- **Trademarks.** Look at every `W01`. Decide per name.
- **Negatives.** Do any block searches you want? mads checks only against your own keywords.
- **Landing pages.** Each ad group's final URL (last column of file 5) fits its keywords.

You can edit the CSVs by hand before uploading. If you change many things, change the input and generate again.

## Upload in Google Ads

1. Open Google Ads and go to Tools, Bulk actions, Uploads.
2. Upload the files in numeric order, one at a time. Rows reference their parents by name, so the campaign file goes first.
3. For each file, look at the preview. Google lists what it will add and flags rows it cannot accept.
4. Fix anything flagged, then apply.

The order is `1-campaign.csv`, `2-ad-groups.csv`, `3-keywords.csv`, `4-negative-keywords.csv`, `5-responsive-search-ads.csv`.

The files use CRLF line endings, UTF-8 without a BOM, and comma decimals for `pt`, `es`, `fr`, `de` and `it`. If your Google Ads account uses a different number format, set `export.decimal_comma` in `business.toml` and generate again, or edit `input.export` in `workspace.json` and run `mads export`. See [the bulk upload format](../howto/google-ads-bulk-upload-format.md).

## Paused by default

`export.status` defaults to `Paused`. Campaigns arrive paused, and nothing serves until you enable them. The ad groups, keywords and ads are `Enabled` inside the paused campaign.

Keep `Paused` for your first runs. Open the campaigns, check settings and enable one at a time.

The last import step in the report follows `export.status`. With `Enabled` it says "Campaigns arrive enabled. They start serving as soon as Google approves the ads."

## Not in the files yet

Sitelinks, callouts and structured snippets are validated but not exported. Add them in the Google Ads interface until the asset templates are verified.

When `business.toml` lists a restricted category, Google may refuse some keywords or ads at upload, such as `Alcohol sale: Your creative promotes the online sale of alcohol.` The `Restricted categories` section of `report.md` has the exception text to paste. See [Restricted categories](restricted-categories.md#when-google-refuses-a-keyword-or-an-ad).

Performance Max, Demand Gen and App campaigns are not in files 1 to 5. `google-ads/editor/account.csv` holds the whole account, Search included, for Google Ads Editor, and you attach the pictures there by hand. Import that file or files 1 to 5, never both. See [Image campaigns](image-campaigns.md#import).
