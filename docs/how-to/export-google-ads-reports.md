# Export Google Ads reports

Export the reports `mads optimize` reads from a live Google Ads account, so the next run is built on real numbers.

Without the keywords and search terms reports, mads cannot cut a wasted keyword or add a negative from a real query. Export all the reports below, for the same dates, before you run `mads optimize`.

## Before you start

- A finished run, imported into Google Ads and running. See [Import into Google Ads Editor](import-into-google-ads-editor.md) and [Go live](go-live.md).
- Data to read. A campaign with fewer than 14 days or fewer than 100 clicks, and fewer than 30 conversions, is thin: only its structure changes. A campaign that is stalled is never thin. See [How optimize decides](../explanation/how-optimize-decides.md).
- Campaign and ad group names as the run exported them. A report row joins the run by campaign name. A renamed campaign is listed as `not in this run` and its rows are not used.
- Access to the account in the Google Ads web interface.

## Steps

1. Pick one date range and use it in every report. For example, the whole life of the account. Reports with different dates are read, but mads prints a warning and does not compute the cost that campaign rows have and search terms do not list.

2. Open the table for the first report in the list below. Set the date range, then add the columns that report needs. To add columns, follow Google's help page [Create, save and schedule reports from your statistics tables](https://support.google.com/google-ads/answer/2404176), which links to the steps for adding columns and segments.

3. Download the table. Google's help page says to click the download icon above the table, then **Download**, then pick a format. Pick a CSV format, such as Excel CSV. The file name does not matter. mads tells a file apart by its columns, in English or Portuguese. It reads UTF-8 and UTF-16 text, comma or tab separated. The title lines, the `Total:` rows, `--` for empty and numbers such as `1.020,50` or `1,020.50` are handled, so leave them in.

4. Repeat for each report. Put every file in one folder.

| Report | What the table is | Columns mads needs | Worth adding |
|---|---|---|---|
| Search terms | What people typed before they saw your ad | Search term, Campaign, Ad group, Impr., Clicks, Cost, Conversions | Match type, Added/Excluded |
| Search keywords | One row per keyword | Keyword, Match type, Campaign, Ad group, Impr., Clicks, Cost, Conversions | Keyword status, Max. CPC, Quality Score, Status reasons, first page bid, top of page bid |
| Campaigns | One row per campaign | Campaign, Impr., Clicks, Cost, Conversions | Campaign status, Budget, Bid strategy type, Status reasons, lost impression share (budget), lost impression share (rank), Conv. value |
| Campaigns by day | The Campaigns table segmented by day | Day, Campaign, Impr., Clicks, Cost, Conversions | The same as Campaigns |
| Conversions by action | The Campaigns table segmented by conversion action | Campaign, Conversion action, Conversions | Conversion category, Conversion source, All conv. |
| Networks | The Campaigns table segmented by network | Campaign, Network, Impr., Clicks, Cost, Conversions | The same as Campaigns |
| Ad groups | One row per ad group, with no keyword, day or ad columns | Campaign, Ad group, Impr., Clicks, Cost, Conversions | Ad group status, Conv. value |
| Assets | One row per headline, description or image | Asset, Campaign | Ad group, Asset type, Performance label, Impr. |

One column matters more than it looks. Search keywords must have **Match type**. A keywords export without it is skipped. For every column name, language and what mads does with it, see [Google Ads reports](../reference/google-ads-reports.md).

At least one report must be in the folder. The conversions by action and networks reports are optional. Campaigns by day can stand in for Campaigns.

A file with the right header but no metric columns is read, and its numbers come out as zero. Add the metrics.

Do not export the Ads report, the one with a row per ad. mads skips it and reads no ad-level numbers.

## Check it worked

Run `mads optimize` with `--reports <folder>`. It prints what it found before the missions start. Read these lines first. Press Ctrl+C if any of them is wrong.

Example run (replay provider, no model was called). It used `--skip-url-check` and `--pause-outside-run`, which do not change these lines:

```text
reports: 8 read (2026-10-13 to 2026-11-02, 21 days)
not in this run: Vinellu - Search antiga
too little data, structure only: Vinellu - Rotulos, Vinellu - App de vinhos
```

- `reports: N read` and the dates must be the range you meant. N is the number of files read.
- `skipped: <file>: <reason>` names a file mads refused and says what it saw. Fix or remove that file.
- `not in this run` lists campaigns the run does not have. Their rows are not used.
- `missing reports, keywords kept as they ran: <kinds>` lists what the folder lacks. Without the Keywords report, an ad group rebuilt without some of the keywords that ran gets them back, with warning `W10`.

Real runs call a model and spend your tokens. The model is called only after these lines. For the full list of lines and what each means, see [Google Ads reports](../reference/google-ads-reports.md#what-mads-optimize-prints-about-the-reports).

## Next

- [Optimize a live account](optimize-a-live-account.md) to run `mads optimize` on the folder.
- [Google Ads reports](../reference/google-ads-reports.md) for every column and rule.
- [Troubleshoot](troubleshoot.md) when a file is skipped.
