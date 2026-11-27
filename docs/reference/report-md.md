# report.md

Look up every section of `report.md`, in the order `mads` writes them, and when each one appears.

`mads generate`, `mads export`, `mads generate --resume` and `mads optimize` write `report.md` at the root of the run directory. The text is fixed in `crates/mads-core/src/report.rs` and `report_live.rs`. Money columns use `export.decimal_comma` and the `budget.currency` of `business.toml`.

## Status line

Every report has one status line under the title. The status comes from the exit code of the run.

| Exit code | Status line |
|---|---|
| `0` | `Ready to import. Review the files in google-ads/ before uploading.` |
| `3` | `Not exported: the account has errors. Fix them and run mads export <run-dir>, or generate again.` |
| any other | `Incomplete: some missions failed. Run mads generate --resume <run-dir> to retry only those.` |

## Section order

| # | Section | Appears when |
|---|---|---|
| 1 | Import notice (a quote line before the title) | The layout is the default bulk layout and the status is `0`. |
| 2 | Title and run line | Always. The title is `mads report: <business name>`. The line holds run id, date, provider and model (`default` when none is set). |
| 3 | Status line | Always. |
| 4 | Summary | Always. |
| 5 | Performance data | `mads optimize` runs only. |
| 6 | Drift since the last run | `mads optimize` runs with drift to report. It is written right after Performance data. |
| 7 | Budget and bids | Always. |
| 8 | Strategy | The run follows a strategy. |
| 9 | Changes | `mads optimize` runs only. |
| 10 | Validation | Always. |
| 11 | URL check | Always. |
| 12 | Restricted categories | `business.toml` lists a restricted category. |
| 13 | Images | The account has a campaign that is not Search. |
| 14 | Usage | Always. |
| 15 | How to import | Always. |
| 16 | After launch | Always. |

## Import notice

The notice is `Import only google-ads/editor/account.csv into Google Ads Editor. Do not import the web bulk CSVs (google-ads/bulk/) into Editor.` A run with the drive-folders layout has no notice.

## Summary

One row per campaign.

| Column | Value |
|---|---|
| Campaign | Campaign name. |
| Format | `Search`, `Performance Max`, `Demand Gen` or `App - Installs`. |
| Intent | `brand`, `catalog`, `generic` or `competitor`. |
| Daily budget | Budget and currency. |
| Bidding | `Manual CPC`, `Maximize clicks` or `Maximize conversions`. |
| Groups | Ad groups plus asset groups. |
| Keywords | Keywords across all ad groups. |

## Performance data

This section opens with the report window (`Reports from <start> to <end> (<n> days).`) and one bullet per report file with its kind and row count. Kinds are `SearchTerms`, `Keywords`, `Campaigns`, `CampaignsByDay`, `AdGroups`, `Assets`, `ConversionActions` and `Networks`. Details are in [Google Ads reports](google-ads-reports.md).

It adds these lines only when they apply.

| Line | Meaning |
|---|---|
| `skipped <file>` | A CSV that no report type matched. |
| `<file>: app events from the product analytics` | The file read through `--app-events`. |
| `The reports cover different dates...` | The reports have different windows, so cost hidden from search terms is left out. |
| `Not part of this run, left as they are: ...` | Live campaigns outside the run. |
| `Too little data to judge results (under 14 days or 100 clicks, and under 30 conversions)...` | A campaign with thin data. Only its structure changed. |
| `Check the conversion action of ...` | A campaign pays for clicks whose result Google Ads does not see. |
| `What the numbers say:` | The findings, one bullet each. |

## Drift since the last run

Lists what changed in the live account since the base run exported it. The cause is stated as `changed outside mads (by hand or by auto-applied recommendations)`. One bullet per difference.

| Bullet shape | Meaning |
|---|---|
| `<name>: daily budget is <n>% higher\|lower than the last run exported` | Budget changed. |
| `<name>: bids with <strategy>, the last run exported <strategy>` | Bid strategy changed. |
| `<name>: paused, the last run exported it enabled` | Paused by hand. |
| `<name>: removed in Google Ads` | Deleted live. |
| `<name>: enabled, the last run dropped it and exported it paused` | Enabled by hand. |
| `the daily budgets of the enabled campaigns add up to <n>% of the daily budget of this run` | Total budget gap. |

## Budget and bids

Opens with a fixed warning that budget shares and CPCs are estimates. Then, for each campaign, its rationale and one bullet per ad group with its CPC and the reason.

## Strategy

Shows the strategy name and intent, the product URLs, and the line `Product campaigns got <n>% of the non-brand budget (asked for at least <min>%).` See [Strategies](../explanation/strategies.md) for the why.

## Changes

Compares the new account with the account that ran. One block per campaign.

| Block | Meaning |
|---|---|
| `**<name>**: new campaign.` | The base run had no such campaign. |
| `**<name>**: no change.` | Nothing differs. |
| `**<name>**` with bullets | Budget, ad group, keyword, asset group and negative changes. |
| `**<name>** is paused in Google Ads and comes enabled` | The live campaign was paused and the import enables it. |
| `**<name>**: dropped, paused by the Editor import.` | The plan no longer has it. |
| `**<name>**: outside this run, paused by the import (--pause-outside-run).` | Written only with that flag. |

## Validation

Without issues the section reads `No errors and no warnings.` Otherwise it has `Errors`, `Warnings` and `Notes` groups. Errors and warnings are grouped by code as `**CODE (count)**`. Each code lists up to 5 examples as `path: message`, then `and <n> more`. The codes are in [Validation rules](validation-rules.md).

## URL check

A table of URL and status. The status is the HTTP code or `unreachable`. The section reads `skipped` when the check did not run.

## Restricted categories

Names the policies listed in `business.toml`, explains that Google may refuse items at upload, and gives a ready exception request to paste into Editor. See [Restricted categories](../explanation/restricted-categories.md).

## Images

Shows the image model, how many images were generated and how many reused. A table follows with the columns Campaign, Asset group, Image, Ratio, Reference and File. A picture with no file shows `missing`. A line about `editor/images/logo.png` appears for Demand Gen and Performance Max when the business has a logo.

## Usage

One row per mission with attempts, input tokens, output tokens, cost and result. The result is `finished`, `failed: <reason>`, `pending` or `running`. Cost is `n/a` when the provider reports none. A `Total` line closes the section.

## How to import

The text depends on the layout and the run.

| Case | Content |
|---|---|
| Default layout | Editor steps for `google-ads/editor/account.csv`, then the web bulk steps for files 1 to 5 of `google-ads/bulk/`. The first line says whether campaigns arrive paused or enabled, from `export.status`. |
| Default layout, optimize run | Editor steps only. The bulk files would add every Search campaign again. |
| Image campaigns | Extra steps to import `google-ads/editor/images` and attach pictures by hand. |
| Performance Max | A note that its rows have not gone through a real import. |
| Drive-folders layout | The setup steps and the morning routine of the [automation package](automation-package.md). |

The steps are in [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md).

## After launch

| Block | Appears when |
|---|---|
| `After 2 days` (bid estimate columns, Below first page bid) | Always. |
| No conversion tracking warning | A Search campaign exists, `conversion_tracking` is off and the reports show no Search conversion. |
| `No ads` warning | The account has image campaigns. |
| `After 14 days` (seven reports to export, then `mads optimize <run-dir> --reports <folder>`) | Always. |

## Next

- [Review a run](../how-to/review-a-run.md)
- [Output files](output-files.md)
- [Exit codes and errors](exit-codes-and-errors.md)
