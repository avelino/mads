# Optimize a live account

Feed the Google Ads reports of a running account back to mads and get a new run that rebuilds the account from the numbers.

`mads optimize` never changes the run you pass it. It writes a new run directory with a new `google-ads/editor/account.csv` to import over the live account.

## Before you start

- A finished run that you imported into Google Ads. See [Import into Google Ads Editor](import-into-google-ads-editor.md).
- Enough data in the account. A campaign with under 14 days or under 100 clicks, and under 30 conversions, only gets structure fixes. See [Go live](go-live.md) for how long to wait and [How optimize decides](../explanation/how-optimize-decides.md) for the lines mads draws.
- The report CSVs in one folder. See [Export Google Ads reports](export-google-ads-reports.md).
- A provider that works. See [Choose a provider](choose-a-provider.md).
- The campaign and ad group names untouched. The name is what links a report row to the run.

## Steps

### 1. Pick the run to pass

Pass the run you imported last.

| Cycle | Run directory |
|---|---|
| First optimize | The `generate` run you imported. |
| Second optimize and every one after | The `optimize` run you imported last, not the original `generate` run. |

Drift compares the live account with what the run you pass exported. With the `generate` run in cycle 2, mads compares the account with a plan you replaced. An `optimize` run also keeps the account as it was before it ran, and that is how mads sees a campaign it dropped come back enabled. A `generate` run has no such record, so that check does nothing.

The run needs all its missions finished. Otherwise the command exits `1` and creates no run directory. Finish it first with `mads generate --resume <run-dir>` and the provider flags of the original run. See [Resume a run](resume-a-run.md).

### 2. Export the reports for the right dates

Export every report for the same date range and put the files in one folder. The folder holds only reports, and the file names do not matter.

- Cycle 1: from the day the account started serving to today.
- Cycle 2 and later: from the day you imported the last optimize run to today. Days before the import ran the older structure.

Reports with different dates are read, but mads warns and drops the cost hidden from search terms. The report kinds and their columns are in [Google Ads reports](../reference/google-ads-reports.md).

### 3. Run optimize

This step calls a model and spends your tokens. Every campaign mission runs again.

```bash
mads optimize out/<run-id> --reports perf/2026-11-03 --provider anthropic --model <model-id>
```

`<run-id>` is the run from step 1. Use the provider flags you use for `generate`. The new run lands in `out/`, or in the folder you pass with `--out`.

`optimize` starts from the input saved in the base run, not from `business.toml`. A key you changed in the file after that run does not reach it. These flags change the new run:

| Flag | What it does |
|---|---|
| `--daily-budget <n>` | New daily budget of the whole account. The plan splits it across campaigns. |
| `--app <STORE=ID>` | Replaces the apps. Repeat it for one app per store. |
| `--status <paused\|enabled>` | Status of campaigns that are not in the account yet. |
| `--target-cpa <n>` | The most you pay for one conversion. |
| `--target-cpi <n>` | The most you pay for one app install. |
| `--target-cps <n>` | The most you pay for one in-app action. |
| `--app-events <CSV>` | Your analytics counts, see step 4. |
| `--pause-outside-run` | See step 5. |

The full list and the rules for each value are in the [CLI reference](../reference/cli.md#mads-optimize).

A campaign that is already in the account and that the plan keeps comes out enabled, even when it is paused there. A new campaign follows `--status`, or the `export.status` of the base run.

### 4. Judge App campaigns by in-app actions (optional)

An install is not always the result you pay for. Export the daily counts from your own analytics and pass them with a target for one in-app action.

```bash
mads optimize out/<run-id> --reports perf/2026-11-03 --app-events perf/app-events.csv --target-cps 12 --provider anthropic --model <model-id>
```

Keep the file out of the reports folder. mads would list it there as skipped. It needs four columns, in any order and any letter case.

| Column | Value |
|---|---|
| `date` | `YYYY-MM-DD`. A timestamp that starts with the date works too. |
| `store` | `google_play` or `app_store`. |
| `installs` | Installs your app counted that day for that store. |
| `in_app_actions` | In-app actions, such as sign-ups, counted that day for that store. |

```text
date,store,installs,in_app_actions
2026-10-03,google_play,120,9
2026-10-03,app_store,80,4
```

A comma, a semicolon or a tab separates the columns. Numbers can be `1020`, `1,020`, `1.020`, `1,020.50` or `1.020,50`. A missing column, an unknown store, or a date or number mads cannot read stops the run before it creates a directory. The message names the line and the command exits `1`.

mads sums the rows inside the date range of the reports, per store. Two App campaigns of one store carry the same numbers, because the file does not say which campaign an install came from. To make an app optimize for an in-app action in the import, set `optimize_for` and `conversion_action` on the app. See [business.toml](../reference/business-toml.md). How the numbers are used is in [How optimize decides](../explanation/how-optimize-decides.md#judging-app-campaigns-by-in-app-actions).

### 5. Pause spend outside the run (optional)

An older campaign that nobody planned can spend a large part of the account and convert nothing. By default mads leaves campaigns outside the run alone. With `--pause-outside-run` it writes a `Paused` row for each enabled one it flagged.

```bash
mads optimize out/<run-id> --reports perf/2026-11-03 --pause-outside-run --provider anthropic --model <model-id>
```

Read the finding in the terminal before you import. The rule that flags a campaign is in [How optimize decides](../explanation/how-optimize-decides.md#spend-outside-the-run).

### 6. Read what the reports told mads

Before any mission starts, `optimize` prints what it read to stderr. Read it. A wrong date range or a skipped file shows up here, before the tokens are spent.

Example run (replay provider, no model was called). It ran with `--skip-url-check` and `--pause-outside-run`.

```text
reports: 8 read (2026-10-13 to 2026-11-02, 21 days)
not in this run: Vinellu - Search antiga
too little data, structure only: Vinellu - Rotulos, Vinellu - App de vinhos
stalled, no ads or no impression for 7 days, no more budget: Vinellu - Marca
budget unspent every day: 41.23
enabled outside this run and spending without conversions while the account converts, money the plan does not manage: Vinellu - Search antiga (47.21% of the account cost, 241 clicks)
paused by the import (--pause-outside-run): Vinellu - Search antiga
conversion actions of Vinellu - Rotulos: Cadastro concluído (6), Clique em baixar o app (1)
conversion actions of Vinellu - App de vinhos: Cadastro concluído (2), Clique em baixar o app (1)
networks of Vinellu - Rotulos: search 92% of cost, 100% of conversions, CTR 2.1%; search partners 8% of cost, 0% of conversions, CTR 1.33%
networks of Vinellu - App de vinhos: search 92% of cost, 100% of conversions, CTR 5.16%; search partners 8% of cost, 0% of conversions, CTR 3%
drift since the last run (changed outside mads (by hand or by auto-applied recommendations)):
  Vinellu - App de vinhos: daily budget is 67% higher than the last run exported
  Vinellu - App de vinhos: bids with `Maximizar cliques`, the last run exported Manual CPC
  the daily budgets of the enabled campaigns add up to 136% of the daily budget of this run
```

The line `reports: 8 read (...)` is the date range. The drift lines are what changed in the account since the base run exported it. The reports cannot tell whether a person or an auto-applied recommendation did it.

If you see `error: no Google Ads report in <dir>`, the folder has no CSV mads knows. See [Exit codes and errors](../reference/exit-codes-and-errors.md).

### 7. Read the Changes section

Open `report.md` in the new run and read `Changes` before you import. It lists, per campaign, what differs from the account that ran. The other sections are in [report.md](../reference/report-md.md).

Example run (replay provider, no model was called). A slice of the second cycle.

```markdown
## Changes

Compared with the account that ran. Each campaign rationale and CPC reason is in Budget and bids.

**Vinellu - Marca** is paused in Google Ads and comes enabled

- daily budget 5,00 to 4,00

**Vinellu - Rotulos**

- daily budget 32,00 to 38,00
- rotulo-alamos: keywords added `alamos malbec é bom` (exact)
- new ad group rotulo-norton
- ad group rotulo-trapiche dropped, paused by the import
```

Two things to look for.

- A plan that did nothing. The plan mission can finish on the account copied from the base run, with the live budgets, even when its `set_account_plan` call was refused. This happens with `E25`, when the plan drops an ad group that had impressions and the campaign rationale has no `Drop <group>: <reason>` line. The run still exits `0`. If every campaign reads `no change` or only shows budget moves, the plan was not applied. Open `transcripts/plan.jsonl` in the run directory to see the refusal, then run `optimize` again.
- Rows you did not expect. A dropped campaign, ad group or keyword comes as a `Paused` row. A campaign the plan keeps comes enabled. What each row does is in [Google Ads Editor file](../reference/editor-file.md#rows-added-by-an-optimize-run).

### 8. Import the new Editor file

Import `google-ads/editor/account.csv` from the new run with Google Ads Editor. See [Import into Google Ads Editor](import-into-google-ads-editor.md).

Do not upload the files in `google-ads/bulk/`, on the web or in Editor. They would add every Search campaign a second time.

Two things stay as they are after the import, so check them in the Editor preview.

- A responsive search ad with new texts comes in next to the old one. Pause the old one by hand.
- A negative keyword the new run dropped stays in Google Ads. Remove it by hand.

With `--layout drive-folders`, `optimize` writes the ads automation package instead of the Editor file. See [Use the automation package](use-the-automation-package.md).

## Check it worked

- The command exits `0` and the last line reads `done (exit 0)`.
- `report.md` has the line ``> Ready to import. Review the files in `google-ads/` before uploading.``
- `Changes` lists what you expect, campaign by campaign.
- After the import, Editor shows the campaigns the run dropped as paused.

Example run (replay provider, no model was called). The last line of the second cycle.

```text
10:21:18 [run] done (exit 0): 235000 in, 27200 out tokens, cost n/a, 4 missions finished
```

## Next

- [Export Google Ads reports](export-google-ads-reports.md)
- [How optimize decides](../explanation/how-optimize-decides.md)
- [Safe on a live account](../explanation/safe-on-a-live-account.md)
