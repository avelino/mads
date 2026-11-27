# How optimize decides

Predict what `mads optimize` keeps, cuts and leaves alone when it turns Google Ads reports into a new plan.

## Two kinds of rules

Some rules are code. A tool refuses the call, the agent gets an error and has to fix it. Other rules are prompt text. The agent reads them and usually follows them, but nothing stops it.

This split matters on a live account. Code protects what must never happen. The prompt carries judgment. Each section below says which one it is.

## What optimize starts from

`mads optimize` reads the finished base run and never writes to it. It copies the input saved in that run into a new run and starts the plan and campaign missions again, now with the reports in their tools.

The input comes from the base run, not from your current `business.toml`. A key you added to the file after the run does not reach the new run. The command line overrides a few keys for it: budget, apps, export status, the three targets, app events, pausing outside campaigns and the layout.

The budgets are the ones running today. If someone changed a budget in Google Ads after the import, the campaigns report carries the new number and the new run starts from it. The base run's number is only kept to detect [drift](#drift-since-the-last-run).

A run with unfinished missions cannot be a base. A folder with no report mads recognizes stops the command before a run directory exists. Both exit with code `1`.

## Digest, not dumps

One search terms export can have thousands of rows. The agents get a digest instead, one entry per campaign and per ad group, stored in `workspace.json` under `live.performance`.

- Campaign and ad group totals, status, budget, bid strategy type, Google's status reasons and the share of impressions lost to budget and to rank.
- Real spend. `avg_daily_cost` is cost per day and `budget_use_pct` is that over the daily budget. Under 100, the budget is not what limits the campaign.
- Keywords with their match type, Quality Score and bid estimates, plus three signals read from status reasons: `below_first_page`, `rarely_shown` and `low_quality`. A Quality Score of 4 or less also counts as `low_quality`.
- The 30 most expensive search terms of each ad group, each marked `keyword`, `negative`, `excluded` or `new`.
- `hidden_terms_cost`, the campaign cost that Google does not show by term. It is set only when the campaigns and search terms reports cover the same days.
- `missing_reports`, the kinds of report the folder lacks.

Campaign names and ad group names are the key between a report row and the run. That is why renaming them in Google Ads breaks the link.

When a campaign mission asks for one group with `get_ad_group_performance`, it gets the 20 most expensive search terms of that group, not the 30 kept in the digest.

## What counts as too little data

Three flags mark a campaign that the numbers cannot judge. They are set before the plan sees it.

**Thin.** A campaign with under 14 days of data or under 100 clicks, and under 30 conversions. Its numbers are noise. The agents are told to fix structure only: bids under the first page, low quality, missing ads, mixed groups. They do not cut, pause or grow anything because of its results. The 14 days, 100 clicks and 30 conversions are lines mads draws, not Google rules.

Thin has one exception for money. A thin campaign that does not spend its budget can still lose the unspent part, because unspent budget is not a result.

**Stalled.** Enabled, and either Google says it has no ads or a week passed without one impression. A stalled campaign is broken, not short of data, so it is never thin. The agents are told to fix what Google reports or cut the budget.

**Untracked.** 100 clicks or more, zero conversions, while another campaign of the run converts. Its result is not measured, or there is none. A Search campaign that lands on a site with no conversion tracking, next to an App campaign counting installs, is the usual case.

An App campaign can also be untracked for a narrower reason. When a conversion actions report is there, the campaign has 100 clicks or more and no action counts for its store, it is untracked with the reason `no conversion action for this store`. mads reads the store from the words Android, Google Play, iOS, iPhone or App Store in the action name, category or source. An action that names no store counts for both.

The signals are also read from the other reports. The conversion actions report lists the actions each campaign counted, the five with most conversions first. In an account that has an App campaign, a Search or Demand Gen campaign that counts actions that are not installs gets `off_goal_actions`. That is a warning, nothing is capped. The networks report gives each campaign its share of cost and conversions per network. An App campaign whose Display CTR is above 15 percent, with at least 30 Display clicks, gets `accidental_display_clicks`. The plan treats its installs with caution and does not raise its budget on cost per install alone. Both numbers are lines mads draws.

## How the plan is held back

Three checks run in code when the plan is saved.

**E27, budget caps.** A stalled campaign gets no more daily budget than it ran with. An untracked one gets no more than it ran with and no more than its average daily cost rounded up. A campaign over its target gets no more than it ran with. The plan sees the cap as `budget_cap` in `get_business`, so it splits the budget with the caps in mind instead of finding them one refused call at a time. Cutting is always allowed.

**E25, group names.** An ad group that had impressions keeps its name. Editor matches by name, so a renamed group comes in as a new one, and its keywords start without Quality Score history. To drop a group on purpose, the campaign rationale needs a line `Drop <group>: <reason>`.

**E28, bids in untracked campaigns.** A higher `default_cpc` in an untracked campaign is refused. Its clicks bring no conversion anyone can see, so a higher bid only buys more of them.

Two more guards sit in the campaign tools. A group that ran cannot be rebuilt before the mission reads its detail with `get_ad_group_performance`, or the call fails with `LIVE_DETAIL`. And without a keywords report, the keywords that ran are put back with warning `W10`, because nothing justifies dropping them.

The rest is prompt. The plan is asked to cut a campaign that spends under its budget down to its real spend plus a tenth, unless this run fixes what kept it from spending. It is asked to give the freed money, and the idle budget of the account, to a converting campaign that uses 90 percent or more of its budget, in steps of about 20 percent per run, or up to 40 percent with 30 conversions or more in the date range. No check enforces these numbers.

The code that refuses is listed in [Validation rules](../reference/validation-rules.md), and the agent tools in [MCP tools](../reference/mcp-tools.md).

## Targets

Without a target, the plan decides what a good result is. With one, it has a number to hold each campaign to. There are three, all optional, all in `[budget]`.

- `target_cpa` for every campaign that is not an App campaign.
- `target_cpi` for an App campaign.
- `target_cpa_in_app` for an App campaign that has app event numbers.

Each campaign that ran gets a `vs_target` verdict.

- `within`, each conversion cost the target or less.
- `over`, each conversion cost more than the target.
- `no_result`, the campaign spent twice the target or more and converted nothing. The factor is a line mads draws.

A campaign with no target for its kind, or with too little spend to say, has no verdict. `over` and `no_result` feed the E27 cap above, thin or not. The money they free goes to a `within` campaign that its budget limits.

`mads init --force` keeps all three targets in the file, like `max_cpc`.

## Judging App campaigns by in-app actions

An install is not always the result you pay for. An App campaign can hit its install target while the installs that become accounts cost many times more. Google's install count can also be higher than the installs your app sees.

When you give mads the counts from your own analytics, it attaches them to each App campaign of the same store, summed over the date range of the reports. The plan then reads the installs your app saw, the in-app actions, the share between them, the cost per in-app action and the ratio of Google's conversions to installs seen. A ratio well above 1 means Google counts installs your app never sees.

With `target_cpa_in_app` set, the verdict comes from the cost per in-app action of the whole store, and it wins over `target_cpi`. The plan is also told that a store whose installs rarely become actions should not get more budget just because its cost per install is low.

The numbers are per store. The file does not say which campaign an install came from, so two App campaigns of one store carry the same numbers and one shared cost. A campaign whose store has no rows in the date range is judged by `target_cpi` as before, and mads lists it as judged without app events.

The column layout of the analytics file is in the [how-to](../how-to/optimize-a-live-account.md).

## Spend outside the run

An older campaign nobody planned can spend a large part of the account and convert nothing. mads reads every campaign of the reports that the run does not manage and flags one when all of this holds.

- It is enabled. Paused and removed campaigns are never flagged.
- It has no conversion, and some campaign of the account converts. Without any conversion in the account, tracking is probably missing and the numbers say nothing.
- It has 100 clicks or more, or it spent more than 10 percent of the cost of every campaign in the reports. At exactly 10 percent it is not flagged.

The plan is told that this money is outside the plan. The daily budget it splits does not include it, and it must not treat that spend as room to grow other campaigns.

By default nothing is exported for these campaigns. Pausing them is opt-in, and it writes a `Paused` row for exactly the flagged ones. A campaign of the run never gets such a row, and neither does a removed one. The reasoning is in [Safe on a live account](safe-on-a-live-account.md).

## Drift since the last run

After an import the account keeps changing. Google Ads can apply recommendations on its own, and people edit budgets. The reports cannot tell which, so mads calls it a change made outside mads, by hand or by auto-applied recommendations. It compares the campaigns report with what the base run exported.

- **Daily budget.** The live budget differs from the exported one by more than 20 percent, up or down. At exactly 20 percent it is not flagged.
- **Bid strategy.** The strategy type in the report is not the one the run exported.
- **Paused.** A campaign the base run exported enabled is paused in the account. A campaign exported paused because of `export.status` is not flagged while it stays paused.
- **Removed.** A campaign of the base run is removed in the account, whatever status it was exported with.
- **Enabled again.** A campaign the base run dropped, and so exported paused, is enabled in the account.

mads also adds up the daily budgets of every enabled campaign of the account, in the run or not, and warns when the total is more than 20 percent above the daily budget of the new run.

The plan is told that budgets moved outside mads, so it plans from the live ones. It is also told that a campaign paused or removed there still comes back enabled if it keeps it, so it should keep it only if it is worth the money.

A bid strategy changed outside mads is never overwritten. The import leaves it as it is, and under an automatic strategy the ad group CPCs of the run are not in use.

## What the new run does to the account

Optimize rebuilds what the run lists and pauses what it drops. A campaign, ad group or keyword the new run dropped comes as a `Paused` row. An item the file does not name keeps serving. A new responsive search ad comes in next to the old one, and a dropped negative stays in Google Ads. The reasons are in [Safe on a live account](safe-on-a-live-account.md), and the exact rows are in the [Editor file](../reference/editor-file.md) reference.

Image and app campaigns get the same live fields. A picture whose image id, ratio and prompt stay is reused from the old run and costs nothing. See [Image and app campaigns](image-campaigns.md).

## Next

- [Optimize a live account](../how-to/optimize-a-live-account.md)
- [Export Google Ads reports](../how-to/export-google-ads-reports.md)
- [Safe on a live account](safe-on-a-live-account.md)
