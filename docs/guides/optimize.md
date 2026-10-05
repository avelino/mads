# Optimize a live account

This page shows how to feed Google Ads reports back to mads so it rebuilds a running account from what the numbers say.

`mads optimize` starts from a finished run that you imported into Google Ads. It reads the reports you exported, runs the plan and every campaign mission again with those numbers, and writes a new run. You import the new Editor file over the live account. The original run is not changed.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --provider anthropic --model <model-id>
```

## Export the reports

Wait until the account has data. 14 days or 100 clicks per campaign is a line mads draws, not a Google rule: below it a campaign is marked thin and only its structure changes (see below). Google gives no fixed number either. A bid strategy needs a learning period of [1 to 2 conversion cycles](https://support.google.com/google-ads/answer/13020501?hl=en), Google advises [not measuring performance until it ends](https://support.google.com/google-ads/answer/6263057?hl=en), and it asks for [at least 4 to 6 weeks](https://support.google.com/google-ads/answer/13826584?hl=en) before reading an experiment. With a small budget, wait longer than 14 days.

In Google Ads, export each report as CSV for the same date range and put the files in one folder. The file names do not matter. mads tells the reports apart by their columns, in English or Portuguese.

| Report | Where in Google Ads | Columns to add |
|---|---|---|
| Search terms | Insights and reports, Search terms | Match type, Added/Excluded, Conversions |
| Search keywords | Keywords, Search keywords | Quality Score, First page bid estimate, Top of page bid estimate, Status reasons |
| Campaigns | Campaigns | Campaign status, Budget, Status reasons, Search lost IS (budget), Search lost IS (rank) |
| Campaigns by day | Campaigns, segmented by Day | The same |
| Ads and assets | Ads, Assets | Performance label |

None is required, but at least one must be there. Export them for the same dates. Reports of different dates are read, but mads warns and leaves out the cost hidden from search terms, since campaign totals and term rows no longer add up. A CSV that is not one of these is listed as skipped and changes nothing. The two title lines, number formats such as `1.020,50` or `1,020.50`, `--` for empty and the `Total:` rows are handled.

Do not rename campaigns or ad groups in Google Ads. The name is what links a report row to the run.

## What mads reads from them

The reports are not sent to the agents as they are. One search terms export can have thousands of rows. mads builds a digest per campaign and per ad group and stores it in `workspace.json` under `live.performance`.

- **Campaigns.** Impressions, clicks, cost, conversions, value, CTR, average CPC, cost per conversion, the live status, Google's status reasons and the [share of impressions lost](https://support.google.com/google-ads/answer/7103314?hl=en) to budget (not enough budget) and to rank (Ad Rank too low). With a campaigns report, its numbers win. With only the by-day report, totals are summed and the lost shares are averaged over the days.
- **Keywords.** The same numbers, the max CPC, the Quality Score and the bid estimates when the export has them, and three signals read from the [keyword status](https://support.google.com/google-ads/answer/2453978?hl=en) reasons: `below_first_page` (the keyword is active but not reaching the first page of results, below the [first page bid estimate](https://support.google.com/google-ads/answer/105665?hl=en)), `rarely_shown` (Google rarely shows it because of a low Quality Score) and `low_quality`. A Quality Score of 4 or less also counts as `low_quality`.
- **Search terms.** The 30 most expensive terms of each ad group, each marked `keyword`, `negative`, `excluded` or `new`. Every group also gets the total cost of its terms with zero conversions. When a campaigns report is there, `hidden_terms_cost` is the campaign cost Google does not show by term: the report [leaves out terms with too little query activity](https://support.google.com/google-ads/answer/2472708?hl=en), for privacy.
- **Thin campaigns.** Under 14 days in the date range, or under 100 clicks. The agents are told to fix structure only there: bids under the first page, low quality, missing ads, mixed groups. They do not cut or reward anything because of its results.

Campaigns in the reports that are not part of the run, such as older campaigns of the account, are listed as not in this run and left alone.

## What the agents see

The plan mission gets `live_account` (every campaign as it ran) and `performance` (campaign totals) in `get_business`. It can move budget between campaigns, split or merge ad groups and leave a campaign out. A group that had impressions keeps its name: the plan is refused with `E25` unless the campaign rationale says `Drop <group>: <reason>`. Editor matches by name, so a renamed group comes in as a new one, and Google measures [Quality Score per keyword from its past impressions](https://support.google.com/google-ads/answer/6167118?hl=en), showing "—" until a keyword has enough data.

Each campaign mission gets `live` and `performance` in `get_brief` as one line per ad group, then calls `get_ad_group_performance` for each group before it rebuilds it. The tools enforce it: rebuilding a group that ran before reading its detail fails with `LIVE_DETAIL`. That call returns the group's keywords, negatives and texts as they ran, the keywords that had traffic with their numbers, a count of the silent ones per signal, and the 20 most expensive search terms. One call with every group did not fit in what an agent CLI can read. See [MCP tools](../reference/mcp-tools.md).

Image and app campaigns get the same fields. A picture whose image id stays is reused from the old run and costs nothing.

## Import the result

Import `google-ads/editor/account.csv` with Google Ads Editor, as described in [Review and import](review-and-import.md). Do not upload files 1 to 5 on the web: they would add every Search campaign a second time.

What the file does to the live account:

- A campaign that is live keeps its status from the campaigns report. A new campaign follows `export.status`. So does a campaign the reports show as removed: Google Ads keeps removed campaigns out of reach, and the import treats it as a new one.
- A campaign, ad group, asset group or keyword that the new run dropped comes as a row with status `Paused`. An import changes what the file lists: Editor reads a [status column with Enabled, Paused or Removed](https://support.google.com/google-ads/editor/answer/57747?hl=en), and [new statuses for existing items come through CSV import](https://support.google.com/google-ads/editor/answer/53082?hl=en). An item the file does not name keeps serving. mads writes these rows only for campaigns the reports show in the account and not removed: a row for a campaign that was never imported would create it.
- A responsive search ad with new texts comes in as another ad in the group, and the old one stays enabled. Check it in the Editor preview and pause the old ad by hand.
- A negative the new run dropped stays in Google Ads. Remove it by hand.

The `Changes` section of `report.md` lists each of these per campaign, so you know what to check in the Editor preview.

## Errors

```text
error: run has unfinished missions (campaign:vinellu-marca); finish it with `mads generate --resume <run-dir>` first
```

`optimize` needs a finished run. Exit code `1`.

```text
error: no Google Ads report in perf: export search terms, keywords or campaigns as CSV (no .csv file)
```

The folder has no CSV, or none of them is a report mads knows. The message lists each file with the reason. Exit code `1`. In both cases no run directory is created.
