# Google Ads Editor file

Look up the exact format of `google-ads/editor/account.csv`, the one file you import into Google Ads Editor.

Source: `crates/mads-core/src/google/editor.rs`.

## Where it is written

| Item | Value |
|---|---|
| Path | `out/<run>/google-ads/editor/account.csv` |
| Layout | Only with the default `--layout bulk`. `--layout drive-folders` writes the [automation package](automation-package.md) instead |
| Empty account | No file |
| Pictures | `editor/images/`, not referenced by the file. Editor neither exports nor imports the link between an ad and its images |

The five CSVs in `google-ads/bulk/` are for the web bulk upload only. Never import both sets, see [Bulk upload format](bulk-upload-format.md).

## Encoding

| Property | Value |
|---|---|
| Character encoding | UTF-16 LE with a byte order mark (`FF FE`) |
| Separator | Tab |
| Line ending | LF |
| Quoting | A field is wrapped in double quotes when it holds a comma, a quote, a tab or a line break. Quotes inside are doubled |
| Money | Dot and 2 decimals, such as `250.00`. `export.decimal_comma` does not change it |
| Row type column | None. The filled columns say what a row is |
| Empty cell | Leaves the field as it is in the account |

## Verification level

| Rows | Status |
|---|---|
| Search, App | Match a real export of Editor 2.13.3 |
| Demand Gen | A file with Search, App and Demand Gen imported into Editor 2.13.3 with no error and every entity counted |
| Performance Max | Follow Google's documented headers, not imported yet |
| `Business name`, `Long headline 1` to `5` | Documented by Google, not seen in a real export |
| `Conversion action`, `Target CPA` | Added only for an in-app action goal. Names not verified against Editor |

## Columns

In this order:

| Group | Columns |
|---|---|
| Campaign | `Campaign`, `Campaign Type`, `Networks`, `Budget`, `Budget type`, `EU political ads`, `Languages`, `Bid Strategy Type`, `App campaign store`, `App campaign package name`, `Campaign optimization`, `Final URL suffix` |
| Ad group | `Ad Group`, `Max CPC`, `Max CPM`, `Target CPV`, `Target CPM`, `Ad Group Type` |
| Targeting | `Location`, `Keyword`, `Criterion Type` |
| Ad | `Final URL`, `Ad type`, `Headline 1` to `5`, `Description 1` to `5` |
| Asset group | `Asset Group`, `Headline 6` to `15` |
| Search ad | `Path 1`, `Path 2` |
| Status | `Campaign Status`, `Ad Group Status`, `Asset Group Status`, `Status` |
| Last | `Business name`, `Long headline 1` to `5` |
| Only with an in-app action goal | `Conversion action`, `Target CPA` |

The two in-app columns come after all the others, and only when some App campaign has `optimize_for = "in_app_actions"`. Without such a campaign the header stays the same.

## Rows per campaign type

Each campaign writes one campaign row, then its location rows, then the rows below.

| Campaign type | `Campaign Type` | Rows after the campaign and locations |
|---|---|---|
| Search | `Search` | Per ad group: an ad group row, one keyword row per keyword, one negative row per effective negative, one `Responsive search ad` row |
| App installs | `App - Installs` | Per asset group: an ad group row and an `App ad for installs` row |
| Demand Gen | `Demand Gen` | Per asset group: an ad group row and a `Demand Gen image ad` row |
| Performance Max | `Performance Max` | One asset group row per asset group |

The Search ad row is built from the ad group's responsive search ad merged with the brand kit when there is one.

## Campaign row

| Column | Value |
|---|---|
| `Campaign Status` | See [Status of a row](#status-of-a-row) |
| `Budget` | Daily budget, money format above |
| `Budget type` | `Daily` |
| `EU political ads` | `Has EU political ads` when `export.eu_political_ads` is true, else `Doesn't have EU political ads` |
| `Bid Strategy Type` | `Manual CPC`, `Maximize clicks` or `Maximize conversions`. Left empty when the live account's strategy differs from the plan, so an optimize run keeps it |
| `Languages` | The part of `business.language` before the first `-`. Not written for Demand Gen, which targets language on its ad groups |
| `Networks` | `Google search` for Search only |
| `Final URL suffix` | `export.url_suffix` with `{mads_campaign}` replaced by the campaign slug. Written for Search, Demand Gen and Performance Max, not for App |
| `Location` | One row per entry of `business.locations`, with `Status` `Enabled`. On the campaign, except Demand Gen, which gets one row per ad group |

Only `manual_cpc` bidding is accepted today.

## App campaign columns

| Column | Value |
|---|---|
| `App campaign store` | `Google Play` or `iTunes` |
| `App campaign package name` | The app id |
| `Campaign optimization` | `Installs`, or `In-app actions` for an in-app action goal |
| `Conversion action` | The app's conversion action, in-app goal only |
| `Target CPA` | `budget.target_cpa_in_app`, written only when set |

An App campaign without a matching app in `business.toml` gets none of these.

## Ad group row

| Column | Value |
|---|---|
| `Ad Group Status` | `Enabled` |
| `Max CPC` | The ad group's default CPC for Search, `0.01` for App and Demand Gen |
| `Max CPM`, `Target CPV`, `Target CPM` | `0.01`. Editor fills this when a bid does not apply, and warns "the ad group has no bids" without it |
| `Ad Group Type` | `Standard` |
| `Languages` | Demand Gen only, same value as above |

Positive keyword rows set `Criterion Type` to `Phrase` or `Exact`. Campaign negatives are written once per Search campaign, with `Ad Group` empty and `Criterion Type` `Campaign Negative Phrase` or `Campaign Negative Exact`. Ad-group negatives keep `Negative Phrase` or `Negative Exact` and the Ad Group filled. Their `Status` is `Enabled`. Campaign negatives are not copied onto every ad group: that form has been imported as a positive Phrase keyword.

## Asset group row

Performance Max only. It sets `Asset Group`, `Asset Group Status` `Enabled`, `Final URL`, `Business name`, `Headline` numbered from 1, `Long headline` numbered from 1 and `Description` numbered from 1.

Demand Gen ads also set `Final URL` and `Business name`. App ads set neither.

## Status of a row

`Campaign Status` follows these rules.

| Case | Value |
|---|---|
| Campaign the live account does not have | `export.status`, default `Paused` |
| Campaign the plan keeps that the account has Enabled or Paused | `Enabled` |
| Any other status in the account | `export.status` |

A paused live campaign the plan keeps comes back `Enabled`. The plan leaves out what should stay off, so a kept campaign is one worth the money. Ad groups, keywords, ads and asset groups of the plan are `Enabled`.

## Rows added by an optimize run

Editor never deletes on import, so a missing row would leave the old entity serving. `mads optimize` adds rows with status `Paused` for what the live account has and the new plan dropped.

| What | Row |
|---|---|
| Campaign dropped | Campaign row with `Campaign Status` `Paused` |
| Ad group dropped | Ad group row with `Ad Group Status` `Paused` |
| Enabled ad group the run does not know | The same, found from the ad group report. It covers a stray group an older import left in a kept campaign |
| Keyword dropped | Keyword row with `Status` `Paused` |
| Asset group of a Performance Max campaign dropped | Asset group row with `Asset Group Status` `Paused` |
| Asset group of another kind dropped | Ad group row with `Ad Group Status` `Paused` |
| Campaign outside the run | Campaign row with `Campaign Status` `Paused`, only with `--pause-outside-run`, only for the enabled ones the run flagged, never a removed one |

Keyword baseline:

- With a keywords report, it is the enabled keywords the report lists, of any match type, Broad included. A keyword the report shows as paused or removed gets no row.
- Without one, it is the base run's keywords.

A dropped ad group hides its keywords, so pausing the group is enough.

Left alone, with no row:

- A responsive search ad with new texts comes in next to the old one. The old one stays enabled.
- A negative keyword the new run dropped stays in the account.
- Sitelinks, callouts and snippets are validated but not exported.

## Next

- [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md)
- [Optimize a live account](../how-to/optimize-a-live-account.md)
- [Output files](output-files.md)
