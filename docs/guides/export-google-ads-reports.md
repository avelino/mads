# Export Google Ads reports for optimize

This page shows which reports to export from Google Ads, with which columns, so `mads optimize` has the numbers it needs to rebuild a live account.

`mads optimize` is only as good as the reports in its folder. Without the keywords and search terms reports it cannot cut a wasted keyword or add a negative from a real query. Export all of the reports below, for the same dates, before you run it. For what mads does with them, see [Optimize a live account](optimize.md).

## Before you export

- **Pick one date range.** Use the same range in every report, for example the whole life of the account. Reports of different dates are read, but mads warns and drops the cost it cannot match between campaigns and search terms.
- **Wait for data.** A campaign with less than 14 days or 100 clicks is treated as thin, and only its structure changes.
- **Do not rename campaigns or ad groups.** The name links a report row to the run.
- **Export as CSV.** The file names do not matter. mads tells the reports apart by their columns, in English or Portuguese. The "Excel" download (UTF-16, tab separated) is read too.
- **Leave the totals in.** The title lines, the `Total:` rows, `--` for empty and numbers such as `1.020,50` or `1,020.50` are handled.

The menu names above are the English ones. In a Portuguese account they are *Insights e relatórios*, *Campanhas*, *Públicos-alvo, palavras-chave e conteúdo* (then *Palavras-chave*) and *Recursos*.

In every report the steps are the same:

1. Set the date range at the top right.
2. Open **Columns**, then **Modify columns**, and add the columns listed for that report.
3. Click **Download**, then **.csv**.

A report with the right header but without the metric columns is read, and its numbers come out as zero. Add the metrics.

## What to export

| Report | Where in Google Ads | Needed | Worth adding |
|---|---|---|---|
| [Search terms](#search-terms) | Insights and reports, Search terms | Search term, Campaign, Ad group, Impr., Clicks, Cost, Conversions | Match type, Added/Excluded |
| [Search keywords](#search-keywords) | Audiences, keywords and content, Keywords | Keyword, Match type, Campaign, Ad group, Impr., Clicks, Cost, Conversions | Max. CPC, Quality Score, Status reasons, First page bid, Top of page bid |
| [Campaigns](#campaigns) | Campaigns | Campaign, Impr., Clicks, Cost, Conversions | Campaign status, Budget, Status reasons, Search lost IS (budget), Search lost IS (rank), Conv. value |
| [Campaigns by day](#campaigns-by-day) | Campaigns, segmented by Day | Day, Campaign, Impr., Clicks, Cost, Conversions | The same as Campaigns |
| [Ad groups](#ad-groups) | Campaigns, Ad groups | Ad group, Campaign, Impr., Clicks, Cost, Conversions | Conv. value |
| [Assets](#assets) | Assets | Asset, Campaign, Ad group | Asset type, Performance label, Impr. |

At least one report must be there. All six give the best result.

## Search terms

What people typed before they saw your ad. This is the report that finds wasted spend and new keywords.

- Without it, mads cannot add negatives from real queries.
- **Match type** and **Added/Excluded** let mads mark each term as `keyword`, `negative`, `excluded` or `new`.
- mads keeps the 30 most expensive terms of each ad group.

## Search keywords

Every keyword with its numbers and Google's quality signals.

- **Match type** is required. A keyword export without it is not read as a keywords report.
- **Quality Score**, **Status reasons** and the two bid estimates let mads tell a keyword that is below the first page, rarely shown or low quality apart from one that just has no clicks.
- **Max. CPC** shows the bid the keyword ran with.

## Campaigns

One row per campaign.

- **Campaign status** and **Status reasons** are how mads finds a stalled campaign, such as one with no ads.
- **Budget** with **Cost** gives the real daily spend and the budget left unspent.
- **Search lost IS (budget)** and **Search lost IS (rank)** say whether a campaign loses impressions to budget or to Ad Rank. Add both.

## Campaigns by day

The same as Campaigns, segmented by **Day**.

- Use it when campaigns started on different days. mads counts the days from the first one with impressions, so a late campaign is not diluted.
- It can replace the Campaigns report. If both are there, the Campaigns numbers win.

## Ad groups

One row per ad group, with its own totals.

- It must have **Campaign** and **Ad group**.
- It must not have **Keyword**, **Day** or ad columns. With any of them it is read as another report, or skipped.
- It tells mads which groups ran, but nothing about their keywords. Export the keywords and search terms reports too.

## Assets

Headlines, descriptions and images, one row per asset.

- **Performance label** says which assets Google rates well, so mads keeps them and swaps the weak ones.

## Do not export the Ads report

The report with one row per ad (it has **Ad status**, **Ad type** and the headlines) is skipped. mads reads no ad-level numbers. The message names it:

```text
skipped: Relatório de anúncios.csv: no known report header (search terms, keywords, campaigns, ad groups or assets): an ads report has one row per ad, and mads reads no ad-level numbers. Export the search keywords and search terms reports instead
```

## Check before you spend tokens

`mads optimize` prints what it found before the missions start. Read these lines first:

```text
reports: 3 read (2026-10-01 to 2026-11-02, 33 days)
skipped: Relatório de anúncios.csv: no known report header ...
missing reports, keywords kept as they ran: SearchTerms
```

- **`reports: N read`** and the dates. They must be the range you meant.
- **`skipped`** names the file and says what it saw. Fix or remove that file.
- **`missing reports`** lists what the folder lacks. Without Keywords or SearchTerms, mads puts back every keyword that ran, with warning `W10`.

If a line is wrong, press Ctrl+C. Nothing has been billed before the `[run] ... started` line.

## Next

- [Optimize a live account](optimize.md): what mads reads and how the new run differs from the old.
- [Troubleshooting](../howto/troubleshooting.md): messages from the report reader.
