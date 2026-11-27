# Safe on a live account

See why you can point mads at a Google Ads account that already spends money, and where each guard sits in the flow.

The model decides. Code checks every decision. The checks run at five points, from your input file to the import.

## Nothing is uploaded for you

mads has no Google Ads API access. It writes files. You import them, so a human looks at the account before anything changes in Google Ads. The files are the only output that touches your account.

## Before a model call

A typo in `business.toml` is an error before any agent runs. The input structs reject keys they do not know, so `languge` is not silently ignored. The run stops with exit code `2`, the code for input and flag errors. See [Exit codes and errors](../reference/exit-codes-and-errors.md).

## During a run

Agents work only through typed tools. A tool validates its arguments before it writes, and a failed call changes nothing. The agent gets `ok: false` and the list of issues, then tries again.

The rules behind those issues come from the account you gave, not from the model.

- **Money is in cents.** The campaign budgets must add up to `budget.daily` exactly, and no campaign gets less than 1.00 a day (`E06`). A bid above `budget.max_cpc` is refused (`E11`).
- **Landing pages come from you.** A final URL must be your `business.url`, a page you listed or a catalog URL (`E07`). With a `[focus]`, it must be one of the focus URLs (`E22`).
- **A run has a token cap.** `--max-tokens` limits input plus output tokens for the whole run, so a looping agent stops.

The full list of codes is in [Validation rules](../reference/validation-rules.md). For turns, timeouts and retries, see [Limits and retries](limits-and-retries.md).

## Before export

After the missions, plain code takes over. It adds cross negatives, validates the whole account again and requests every final URL.

One error and no file is written. The run ends with exit code `3` and `report.md` says `Not exported`. A URL that does not answer 2xx after redirects is an error (`E15`), unless you passed `--skip-url-check`, which the report states.

The export step also clears the CSVs of an earlier run in the same directory, including after a failure. A run that did not pass never leaves old files that look current.

## On import

Campaigns arrive paused. `export.status` defaults to `Paused`, and nothing serves until you enable a campaign in Google Ads. With `Enabled`, you opt in.

The import also acts on what a plan leaves out. Every live campaign, ad group, keyword or asset group that a new plan dropped comes as a row with status `Paused`. Nothing is deleted.

Two cases decide who keeps control of the account.

- **Campaigns outside the run are left alone.** `--pause-outside-run` is the only way mads writes to them, and then only a `Paused` row for an enabled campaign that spends without converting. A removed campaign never gets a row.
- **A bid strategy you changed in Google Ads stays.** When the live account bids with another strategy than the plan, the strategy cell is left empty and the import changes nothing there.

A campaign the new plan keeps is exported as enabled, even when it is paused in the account. The plan drops what should stay off, so keeping a campaign is a decision to run it.

## In optimize

`mads optimize` reads reports, not your account. It starts a new run from a finished one and never edits the base run.

Before the agents see the numbers, code sorts the campaigns.

- A campaign is thin when it has under 14 days or under 100 clicks and under 30 conversions. The agents get no verdict on thin data.
- A campaign is stalled when it is enabled with no ads, or has no impression for 7 days. That is broken, not short of data, so it is not thin.

Then three rules hold the agents back, whatever they want.

- `E25` refuses to drop an ad group that had impressions, unless the plan says why. Editor matches by name, so a rename restarts its Quality Score.
- `E27` refuses more budget for a stalled, untracked or over-target campaign.
- `E28` refuses a higher bid in an untracked campaign, because more clicks that never register as conversions only cost more.

Last, mads compares the live account with what the base run exported. A budget that moved by more than 20 percent, a changed bid strategy, a pause, a removal or a dropped campaign enabled again all become findings in the report. The reports cannot say who made the change, and the report says so.

To see how those findings shape the plan, read [How optimize decides](how-optimize-decides.md).

## Next

- [How a run works](how-a-run-works.md)
- [How optimize decides](how-optimize-decides.md)
- [Go live](../how-to/go-live.md)
