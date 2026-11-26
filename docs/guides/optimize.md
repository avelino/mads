# Optimize a live account

This page shows how to feed Google Ads reports back to mads so it rebuilds a running account from what the numbers say.

`mads optimize` starts from a finished run that you imported into Google Ads. It reads the reports you exported, runs the plan and every campaign mission again with those numbers, and writes a new run. You import the new Editor file over the live account. The original run is not changed.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --provider anthropic --model <model-id>
```

To change how much the account spends per day, pass `--daily-budget`. The plan mission splits the new amount across the campaigns, and the budgets running today stay as the baseline it compares against. A budget you changed in Google Ads after the import is read from the campaigns report, so the baseline is what the campaign ran with and not what the base run exported.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --daily-budget 500 --provider anthropic --model <model-id>
```

`optimize` starts from the input saved in the base run, not from `business.toml`. A change you made to the file after that run, such as a new app, does not reach it. `--daily-budget`, `--app`, `--status`, `--target-cpa`, `--target-cpi`, `--target-cps` and `--app-events` are the ones you can change here. To add the iOS app to an account that ran with Android only:

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --app google_play=com.example.app --app app_store=6751919708 --provider anthropic --model <model-id>
```

The live App campaign keeps its store (`E31` refuses moving it), the other store comes in as a new campaign, and the plan splits the app budget by the cost per install of each. An app that stays, with the same store and id, keeps its `optimize_for` and `conversion_action`. See [Android and iOS](image-campaigns.md#android-and-ios).

New campaigns leave `Paused` when the base run was generated with `export.status = "Paused"`, even if `business.toml` says `Enabled` now. Pass `--status enabled` to export the ones that are not in the account yet as enabled. A campaign that is already in the account comes enabled when the plan keeps it, even if it is paused there: the plan keeps what is worth the money and leaves out what is not.

## Set a target

Without a target, the plan decides what a good result is. With one, it has a number to hold each campaign to. Both keys are optional, in `[budget]`:

```toml
[budget]
daily = 850
currency = "BRL"
target_cpa = 40.0   # the most you pay for one conversion
target_cpi = 2.5    # the most you pay for one app install
target_cpa_in_app = 12.0   # the most you pay for one in-app action, such as a sign-up
```

Each campaign that ran gets a `vs_target` in what the plan reads:

- `within`: each conversion cost the target or less.
- `over`: each conversion cost more than the target.
- `no_result`: the campaign spent twice the target and converted nothing. The factor is a line mads draws, not a Google rule.

A campaign marked `over` or `no_result` gets no more daily budget than it ran with, thin or not (`E27`), and the plan leaves it out unless something broken explains the result. A `within` campaign that its budget limits is where the freed money goes. App campaigns are judged by `target_cpi`, or by `target_cpa_in_app` when the campaign has app events (see [Judge App campaigns by in-app actions](#judge-app-campaigns-by-in-app-actions)). Every other campaign is judged by `target_cpa`. A campaign without a target for its kind, or with too little spend to say, has no `vs_target`.

`optimize` reads the input saved in the base run, so a target you add to `business.toml` later does not reach it. Pass `--target-cpa`, `--target-cpi` and `--target-cps` (cost per sign-up, or per any in-app action) to set them in the new run. `mads init --force` keeps both keys in the file, like `max_cpc`.

## Judge App campaigns by in-app actions

An install is not always the result you pay for. An App campaign can hit `target_cpi` while each install that becomes an account costs many times more. Google's install count can also be higher than the installs your app sees, so the cost per install the store reports is a weak signal on its own.

Give mads the counts from your product analytics and it judges each App campaign by what an in-app action costs.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --app-events perf/app-events.csv --target-cps 12 --provider anthropic --model <model-id>
```

`--target-cps` sets `budget.target_cpa_in_app` in the new run, the way `--target-cpi` sets `target_cpi`. `--app-events` is a CSV you export from your own analytics. Keep it out of the reports folder, or mads lists it there as skipped.

| Column | What it holds |
|---|---|
| `date` | `YYYY-MM-DD`. A timestamp that starts with the date works too. |
| `store` | `google_play` or `app_store`. |
| `installs` | Installs your app counted that day for that store. |
| `in_app_actions` | In-app actions, such as sign-ups, counted that day for that store. |

The header names are English, in any letter case and any order. A comma, a semicolon or a tab separates the columns. Numbers can be `1020`, `1,020`, `1.020`, `1,020.50` or `1.020,50`. A file with a missing column, an unknown store, or a date or number mads cannot read stops the run before anything is written, and the message names the line.

```text
date,store,installs,in_app_actions
2026-10-03,google_play,120,9
2026-10-03,app_store,80,4
```

mads sums the rows that fall inside the date range of the reports, per store, and attaches the sums to each App campaign of that store. The plan reads these fields in the `app` block of the campaign.

| Field | What it says |
|---|---|
| `app_installs_seen` | Installs your analytics counted in the date range. |
| `in_app_actions` | In-app actions counted in the date range. |
| `install_to_action_pct` | In-app actions as a percent of installs seen. |
| `cost_per_in_app_action` | Cost of the store's App campaigns over `in_app_actions`. |
| `store_vs_app_install_ratio` | Google conversions over `app_installs_seen`. Well above 1 means Google counts installs your app never sees. |
| `store_cost` | Cost of the store's App campaigns in the date range. |

The numbers are per store. The file does not say which campaign an install came from, so two App campaigns of one store carry the same numbers and one shared cost.

With `target_cpa_in_app` set, an App campaign that has these numbers gets its `vs_target` from the cost per in-app action. That wins over `target_cpi`. `no_result` means the store's App campaigns spent twice the target and your app counted no action. The `E27` cap works as it does for any other verdict. A campaign whose store has no row in the file for the date range is judged by `target_cpi` as before, and mads lists it as judged without app events.

The plan is also told that a store whose installs rarely become in-app actions should not get more budget just because its cost per install is low. `mads optimize` prints one line per App campaign before the missions start, and `report.md` repeats it under Performance data.

To make an app optimize for an in-app action in the next import, set `optimize_for` and `conversion_action` on the app. See [Business file](business-file.md#app).

## Export the reports

Wait until the account has data. 14 days and 100 clicks per campaign is a line mads draws, not a Google rule: below it a campaign is marked thin and only its structure changes (see below). 30 conversions judge a campaign sooner. Google gives no fixed number either. A bid strategy needs a learning period of [1 to 2 conversion cycles](https://support.google.com/google-ads/answer/13020501?hl=en), Google advises [not measuring performance until it ends](https://support.google.com/google-ads/answer/6263057?hl=en), and it asks for [at least 4 to 6 weeks](https://support.google.com/google-ads/answer/13826584?hl=en) before reading an experiment. With a small budget, wait longer than 14 days.

In Google Ads, export each report as CSV for the same date range and put the files in one folder. The file names do not matter. mads tells the reports apart by their columns, in English or Portuguese.

The steps and the columns each report needs are in [Export Google Ads reports](export-google-ads-reports.md).

| Report | Where in Google Ads | Columns to add |
|---|---|---|
| Search terms | Insights and reports, Search terms | Match type, Added/Excluded, Conversions |
| Search keywords | Keywords, Search keywords | Quality Score, First page bid estimate, Top of page bid estimate, Status reasons |
| Campaigns | Campaigns | Campaign status, Budget, Bid strategy type, Status reasons, Search lost IS (budget), Search lost IS (rank) |
| Campaigns by day | Campaigns, segmented by Day | The same |
| Conversions by action | Campaigns, segmented by Conversion action | Conversion category, Conversion source, All conv. |
| Networks | Campaigns, segmented by Network | Impr., Clicks, Cost, Conversions |
| Ad groups | Ad groups | Ad group status, Conversions |
| Assets | Assets | Performance label |

None is required, but at least one must be there. The ad groups report gives each group its totals and nothing about keywords: without the keywords and search terms reports, keywords stay blind. An asset groups report is read the same way, one row per group. The Ads page export (one row per ad, with `Ad status` and `Ad type`) is skipped: mads reads no ad-level numbers. Export them for the same dates. Reports of different dates are read, but mads warns and leaves out the cost hidden from search terms, since campaign totals and term rows no longer add up. A CSV that is not one of these is listed as skipped and changes nothing. The two title lines, number formats such as `1.020,50` or `1,020.50`, `--` for empty and the `Total:` rows are handled.

Do not rename campaigns or ad groups in Google Ads. The name is what links a report row to the run.

## What mads reads from them

The reports are not sent to the agents as they are. One search terms export can have thousands of rows. mads builds a digest per campaign and per ad group and stores it in `workspace.json` under `live.performance`.

- **Campaigns.** Impressions, clicks, cost, conversions, value, CTR, average CPC, cost per conversion, the live status, the budget, the bid strategy type, Google's status reasons and the [share of impressions lost](https://support.google.com/google-ads/answer/7103314?hl=en) to budget (not enough budget) and to rank (Ad Rank too low). With a campaigns report, its numbers win. With only the by-day report, totals are summed, the lost shares are averaged over the days, and the budget, status and status reasons come from the last day.
- **Real spend.** `avg_daily_cost` is the cost per day and `budget_use_pct` is that cost over the daily budget. With the by-day report the days count from the first day with impressions (`days_running`), so a campaign that started mid-window is not diluted. Under 100 the budget is not what limits the campaign. `idle_budget` sums what the enabled campaigns leave unspent every day.
- **Ad groups.** Impressions, clicks, cost, conversions, value and the status per ad group. They win over the sum of the group's keywords. Without a campaigns report, campaign totals are summed from them.
- **Groups of older imports.** An enabled ad group of a campaign of the run that the run does not have. An earlier import left it serving next to the groups of this run, on the same searches. The agents see it marked `not_in_run`, and the import pauses it.
- **Bidding changed outside mads.** When the bid strategy type of the report is not the one the run exported, it was changed in Google Ads, by hand or by auto-applied recommendations. The reports cannot tell which. The agents are told, and the import keeps it.
- **Keywords.** The same numbers, the max CPC, the Quality Score and the bid estimates when the export has them, and three signals read from the [keyword status](https://support.google.com/google-ads/answer/2453978?hl=en) reasons: `below_first_page` (the keyword is active but not reaching the first page of results, below the [first page bid estimate](https://support.google.com/google-ads/answer/105665?hl=en)), `rarely_shown` (Google rarely shows it because of a low Quality Score) and `low_quality`. A Quality Score of 4 or less also counts as `low_quality`. Each keyword also carries its match type (`broad`, `phrase` or `exact`) and, when the export has a status column, its status. A plan holds only phrase and exact keywords, so a broad keyword in the account is one the plan never keeps.
- **Search terms.** The 30 most expensive terms of each ad group, each marked `keyword`, `negative`, `excluded` or `new`. Every group also gets the total cost of its terms with zero conversions. When a campaigns report is there, `hidden_terms_cost` is the campaign cost Google does not show by term: the report [leaves out terms with too little query activity](https://support.google.com/google-ads/answer/2472708?hl=en), for privacy.
- **Thin campaigns.** Under 14 days running (or in the date range without day rows) or under 100 clicks, and under 30 conversions. The agents are told to fix structure only there: bids under the first page, low quality, missing ads, mixed groups. They do not cut or reward anything because of its results. Its budget is another matter: money a thin campaign does not spend can go to a campaign that converts.
- **Stalled campaigns.** Enabled with Google's status reason "no ads", or 7 days or more without one impression. A stalled campaign is broken, not short of data, so it is never thin. On a live account a Demand Gen campaign ran 15 days with no ads while mads kept its budget, because zero clicks read as too little data.
- **Untracked campaigns.** 100 clicks or more, zero conversions, while another campaign of the account converts. Its result is not measured, or there is none. A Search campaign that lands on a site with no conversion tracking, next to an App campaign counting installs, is the usual case.
- **Conversion actions.** The actions each campaign counted, the five with most conversions first. An App campaign with clicks enough to judge and no action for its store is `untracked` with `untracked_reason` set to `no conversion action for this store`, even when no other campaign converts. mads prints that reason and the plan is told it gets no more budget. In an account with an App campaign, a Search or Demand Gen campaign that counts actions that are not installs gets `off_goal_actions`, the names of those actions. It is a warning: nothing is capped. See [Conversions by action](export-google-ads-reports.md#conversions-by-action) for how mads reads the store.
- **Networks.** For each campaign, the share of its cost and conversions per network and the CTR of each, in `networks`. A campaign that ran on one network gets no line. An App campaign whose Display CTR is above 15 percent, with at least 30 Display clicks, gets `accidental_display_clicks`: its clicks are likely accidental, so the plan treats its install count with caution and does not raise its budget on cost per install alone. The 15 percent and the 30 clicks are lines mads draws, not Google rules. Only an App campaign gets the flag.
- **Missing reports.** `missing_reports` lists the kinds of report the folder lacks. Without the keywords or search terms report, `upsert_ad_group` puts back every keyword that ran, with warning `W10`.

`mads optimize` prints these findings before the missions start, and `report.md` repeats them under Performance data.

Campaigns in the reports that are not part of the run, such as older campaigns of the account, are listed as not in this run and left alone. Their numbers are still read. See [Spend outside the run](#spend-outside-the-run).

## Spend outside the run

An older campaign nobody planned can spend a large part of the account and convert nothing. mads reads the campaigns and campaigns by day reports for every campaign outside the run and flags one when all of this holds:

- It is enabled. Paused and removed campaigns are never flagged.
- It has no conversion, and some campaign of the account converts. Without any conversion in the account, tracking is probably missing and the numbers say nothing.
- It has 100 clicks or more, the same bar as an untracked campaign, or it spent more than 10 percent of the cost of every campaign in the reports. Both numbers are lines mads draws, not Google rules. At exactly 10 percent it is not flagged.

`mads optimize` prints one finding with the share of the account cost and the clicks of each one, and `report.md` repeats it under Performance data. The plan is told that this money is outside the plan: the daily budget it splits does not include it, and it must not treat that spend as room to grow other campaigns.

By default nothing is exported for these campaigns. Pass `--pause-outside-run` to write a `Paused` row for exactly the flagged ones in the Editor file, and in S1 of the automation package under `--layout drive-folders`.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --pause-outside-run --provider anthropic --model <model-id>
```

The flag never pauses a campaign that is not flagged. A removed campaign never gets a row, and neither does a campaign of the run. Read the finding before you import. The `Changes` section of `report.md` lists each campaign the flag pauses.

## Drift since the last run

The account can change after you import a run. Google Ads can apply recommendations on its own, and people edit budgets. The reports cannot tell which, so mads calls it a change made outside mads (by hand or by auto-applied recommendations). It compares the campaigns report with what the base run exported and lists the differences:

| Drift | When it is flagged |
|---|---|
| Daily budget | The live budget differs from the exported one by more than 20 percent, up or down. At exactly 20 percent it is not flagged. |
| Bid strategy | The strategy type of the report is not the one the run exported. |
| Paused or removed | A campaign the base run exported enabled is paused or removed in the account. A campaign exported paused because of `export.status` is not flagged while it stays paused. |
| Enabled again | A campaign the base run dropped, so it exported it paused, is enabled in the account. |

mads also adds up the daily budgets of every enabled campaign of the account, in the run or not, and compares the total with the daily budget of the new run (`--daily-budget`, or the base run's budget without it). It warns when the total is more than 20 percent above it.

`mads optimize` prints a "drift since the last run" section before the missions start, with shares and no amounts. `report.md` has the same section. The plan is told that the budgets were changed outside mads, so the baseline it reads is the live one. The base run is not changed.

## What the agents see

The plan mission gets `live_account` (every campaign as it ran) and `performance` (campaign totals) in `get_business`. It can move budget between campaigns, split or merge ad groups and leave a campaign out. A group that had impressions keeps its name: the plan is refused with `E25` unless the campaign rationale says `Drop <group>: <reason>`. Editor matches by name, so a renamed group comes in as a new one, and Google measures [Quality Score per keyword from its past impressions](https://support.google.com/google-ads/answer/6167118?hl=en), showing "—" until a keyword has enough data.

Each campaign mission gets `live` and `performance` in `get_brief` as one line per ad group, then calls `get_ad_group_performance` for each group before it rebuilds it. The tools enforce it: rebuilding a group that ran before reading its detail fails with `LIVE_DETAIL`. That call returns the group's keywords, negatives and texts as they ran, the keywords that had traffic with their numbers, a count of the silent ones per signal, and the 20 most expensive search terms. One call with every group did not fit in what an agent CLI can read. See [MCP tools](../reference/mcp-tools.md).

A stalled or untracked campaign gets no more daily budget than it ran with, and an untracked one no more than its average daily cost rounded up: the plan is refused with `E27`. These flags win over thin. Cutting is always allowed, and the budget it frees goes to a campaign that converts at its budget limit.

The prompt asks the plan to cut any campaign that spends under its budget down to its real spend plus a tenth, thin or not, unless this run fixes what kept it from spending. A converting campaign that uses 90 percent or more of its budget takes what is freed, about 20 percent more per run, or up to 40 percent when it had 30 conversions or more in the date range. This part is a prompt rule, not a check in code. An untracked campaign also keeps its bids: `upsert_ad_group` refuses a higher `default_cpc` with `E28`.

When the reports show spend outside the run, `get_business` also has `outside_campaigns`. When they show drift, it has `drift`, and `live_rules` says the budgets were changed outside mads, so the plan starts from the live ones.

Image and app campaigns get the same fields. A picture whose image id, ratio and prompt stay is reused from the old run and costs nothing. Before this check a reused id lost its file, and one optimize run drew 10 unchanged pictures again.

## Import the result

Import `google-ads/editor/account.csv` with Google Ads Editor, as described in [Review and import](review-and-import.md). Do not upload the files in `google-ads/bulk/` on the web or import them into Editor: they would add every Search campaign a second time.

What the file does to the live account:

- A campaign that is live keeps its status from the campaigns report. A new campaign follows `export.status`. So does a campaign the reports show as removed: Google Ads keeps removed campaigns out of reach, and the import treats it as a new one.
- A campaign, ad group, asset group or keyword that the new run dropped comes as a row with status `Paused`. An import changes what the file lists: Editor reads a [status column with Enabled, Paused or Removed](https://support.google.com/google-ads/editor/answer/57747?hl=en), and [new statuses for existing items come through CSV import](https://support.google.com/google-ads/editor/answer/53082?hl=en). An item the file does not name keeps serving. mads writes these rows only for campaigns the reports show in the account and not removed: a row for a campaign that was never imported would create it.
- Keywords of a kept ad group are paused by what the keywords report lists. With that report, every enabled keyword it lists, of any match type, that the new run does not carry with the same text and match type comes as a `Paused` row. That includes broad keywords that Google Ads added with auto-applied recommendations or that Editor created next to a new keyword. A keyword the report lists as paused or removed, or does not list at all, gets no row: it is not serving, and a row for it makes Editor report an error when you post. Without the keywords report, mads pauses the keywords of the base run that the new run dropped, as before, and cannot see broad keywords. Keep the `Match type` and `Status` columns in the export.
- An enabled ad group of an older import, in a campaign the run keeps, comes as a row with status `Paused` too. The ad groups report must have the Ad group status column for mads to see it.
- An App campaign whose app has `optimize_for = "in_app_actions"` comes with `Campaign optimization` set to `In-app actions`, the conversion action and, when `target_cpa_in_app` is set, `Target CPA`. Check these columns in the Editor preview. They are not verified against a real export (see [Business file](business-file.md#app)).
- A campaign outside the run that `--pause-outside-run` flagged comes as a row with status `Paused`. Nothing else outside the run gets a row.
- A bid strategy changed outside mads stays. The campaign row comes with an empty `Bid Strategy Type`, and Editor changes nothing for an empty cell. The `Bid strategy type` cell of `bulk/1-campaign.csv` is empty for the same campaigns. The ad group CPCs of the run only count under Manual CPC.
- A responsive search ad with new texts comes in as another ad in the group, and the old one stays enabled. Check it in the Editor preview and pause the old ad by hand.
- A negative the new run dropped stays in Google Ads. Remove it by hand.

The `Changes` section of `report.md` lists each of these per campaign, so you know what to check in the Editor preview.

### With the automation package

`--layout drive-folders` writes the ads automation package in `google-ads/automation/` instead of `editor/account.csv`. Without the flag, the new run keeps the layout of the run it started from, and a run without a layout is `bulk`.

```bash
mads optimize out/<run-id> --reports perf/2026-10-17 --provider anthropic --model <model-id> --layout drive-folders
```

S1 edits the campaigns that are live and leaves their status as it is. New campaigns come in paused, and S2 enables only those. Campaigns the new run dropped come in S1 as `Paused`. So do the campaigns `--pause-outside-run` flagged. S3 and S4 do the same for fixed ad groups and keywords, with the same keyword rules as the Editor file, so a broad keyword to pause has the match type `Broad` in S4. An item dropped from a live automated campaign stays in `Catalog` as inactive, so the daily track pauses its ad group.

`setupImageCampaigns` updates the budget of a live image campaign and changes nothing else in it. A new image campaign is created paused. One the new run dropped is paused. See [Over a live account](../howto/google-ads-editor-drive-folders.md#over-a-live-account).

## Errors

```text
error: run has unfinished missions (campaign:vinellu-marca); finish it with `mads generate --resume <run-dir>` first
```

`optimize` needs a finished run. Exit code `1`.

```text
error: no Google Ads report in perf: export search terms, keywords or campaigns as CSV (no .csv file)
```

The folder has no CSV, or none of them is a report mads knows. The message lists each file with the reason. Exit code `1`. In both cases no run directory is created.
