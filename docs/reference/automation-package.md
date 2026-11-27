# Ads automation package

Look up every file, tab, rule and limit of the package that `--layout drive-folders` writes.

`mads generate`, `mads export` and `mads optimize` take `--layout bulk|drive-folders`. Without the flag, `generate` writes `bulk`, `export` and `--resume` keep the layout the run was written with, and `optimize` keeps the layout of its base run. For the steps to set it up in Drive and Google Ads, see [Use the ads automation package](../how-to/use-the-automation-package.md).

mads writes local files and never calls Google. The `--help` text of `--layout` calls the package "the local Search automation package". It also carries the image campaigns (Performance Max, Demand Gen and App), which the script creates.

## Tracks

| Track | Campaigns | Where it lives |
|---|---|---|
| Automated | Search campaigns with `intent` `catalog`. One ad group per catalog item, one keyword, one ad. | `Catalog` tab of `0. Feed`, changed daily through the `D` files. |
| Fixed | Every other Search campaign. | Written once, in the `S` files. |
| Image | Performance Max, Demand Gen and App campaigns. | `Image Campaigns` and `Image Assets` tabs, created by `setupImageCampaigns` in `daily-sync.js`. |

The keyword of an automated ad group is the planned keyword that matches the item name, else the item name. Other planned keywords of that ad group are not in the package. A planned ad group that lists no catalog item stays a fixed group of the same campaign.

## Files

```text
out/<run>/google-ads/automation/<Business> Ads Automation/
  Images/
    logo.<ext>
    <campaign_key>/<file>.jpg
  0. Feed.xlsx
  README.xlsx
  daily-sync.js
  Daily/
    D1. New Ad Groups.xlsx
    D2. Pause Ad Groups.xlsx
    D3. Enable Ad Groups.xlsx
    D4. New Keywords.xlsx
    D5. New Ads.xlsx
    D6. Tracking Templates.xlsx
    D7. Ad Group Negatives.xlsx
  Structure/
    S1. New Campaigns.xlsx
    S2. Campaign Status.xlsx
    S3. Ad Groups.xlsx
    S4. Keywords.xlsx
    S5. Ads.xlsx
    S6. Tracking Templates.xlsx
    S7. Negatives.xlsx
    S8. App Extension.xlsx
```

`<Business>` is `business.name` without `/ \ : * ? " < > |` and control characters, with spaces trimmed and collapsed. An empty result gives the folder `Ads Automation`. The script finds the folder by this exact name.

`S8` is written only when `google_ads.app_id` is set. `Images/` is written only when an image campaign has pictures. The pictures are copies of the files in `editor/images/`, which stay where they are.

| File | What it holds | Output tab |
|---|---|---|
| `0. Feed` | The single source. Tabs `Settings`, `Catalog`, `Campaigns`, `Fixed Keywords`, `Ad Templates`, `Negatives`, `Feed`, `Account Status`, `Auto Negatives`, `Updated`, `Image Campaigns`, `Image Assets`. | None |
| `README` | Tabs `Routine`, `How actions are decided`, `Files`, `Linking`, `Pending`. | None |
| `daily-sync.js` | The Google Ads script. Functions `main`, `linkSheets`, `setupImageCampaigns`. | None |
| `D1` | Ad groups of automated campaigns to create. | `Ad Groups` |
| `D2` | Ad groups to pause. | `Ad Groups` |
| `D3` | Paused ad groups to enable. | `Ad Groups` |
| `D4` | The keyword of each new automated ad group. | `Keywords` |
| `D5` | The ad of each new automated ad group. | `Ads` |
| `D6` | The tracking template of each new automated ad group. | `Tracking Templates` |
| `D7` | Negatives that keep automated ad groups apart, from `Auto Negatives`. | `Ad Group Negatives` |
| `S1` | New campaigns. | `New Campaigns` |
| `S2` | Enables the new campaigns. | `Campaign Status` |
| `S3` | Ad groups of fixed campaigns. | `Ad Groups` |
| `S4` | Keywords of fixed campaigns. | `Keywords` |
| `S5` | Ads of fixed campaigns. | `Ads` |
| `S6` | Tracking templates of fixed campaigns. | `Tracking Templates` |
| `S7` | Campaign and ad group negatives. | `Negative Keywords` |
| `S8` | The app extension on the new campaigns. | `App Extension` |

`S1` and `S7` name their output tab `New Campaigns` and `Negative Keywords` because `Campaigns` and `Negatives` are their mirror tabs.

## Tab layout

| Row | Content |
|---|---|
| 1 | Title |
| 2 | Sentence |
| 3 | Note |
| 4 | Header |
| 5 on | Data |

Each `D` and `S` file has mirror tabs that copy `0. Feed` through `IMPORTRANGE`, plus one output tab. Every output cell is a formula. Until `linkSheets` runs, `IMPORTRANGE("FEED_ID", ...)` holds the literal `FEED_ID`.

Cells a person must fill before going live have a yellow fill. The `Pending` tab lists them.

## Script functions

| Function | Does |
|---|---|
| `linkSheets` | Finds the folder by name, points every mirror tab at `0. Feed` and logs each file as `linked` or `FAILED`. Safe to run again. |
| `setupImageCampaigns` | Creates the image campaigns. Logs one line per campaign: `created`, `budget updated`, `paused`, `unchanged` or `FAILED <campaign>: <reason>`. |
| `main` | Reads the account and writes the `Account Status` and `Auto Negatives` tabs. Never changes the account. |

Constants in `main`: `Account Status` holds at most 1000 rows and `Auto Negatives` at most 2000. The script throws when a read would pass either cap.

`setupImageCampaigns` sends one request per campaign with every operation it needs. A failure leaves nothing of that campaign. A new campaign is created paused. An existing campaign, matched by name, only gets its budget updated. Texts and pictures of an existing campaign are not edited. A campaign with action `Pause` is paused when it is enabled. It does not rotate assets, upload video or touch Shopping feeds.

## How actions are decided

One table in the code (`next_step.rs`) generates the `next_step` formula of the `Feed` tab. The `How actions are decided` tab of `README.xlsx` renders the same table.

| `action` | `current_status` | `next_step` | Daily file |
|---|---|---|---|
| `Add` | ad group not in the account | `CREATE` | D1, D4, D5, D6 |
| `Add` | `Paused` | `ENABLE` | D3 |
| `Add` | `Enabled` | empty | none |
| `-` | `Enabled` | `PAUSE` | D2 |
| `-` | `Paused` | empty | none |
| `-` | ad group not in the account | empty | none |

`action` is `Add` when the `Catalog` row has an item and `active` is `Yes`, else `-`. `next_step` is empty on every fixed row. `Account Status` supplies `current_status`.

`Auto Negatives` works on pairs of feed keywords. When keyword B contains keyword A as a phrase, B becomes a `Phrase` negative in A's ad group, plus B spelled without accents. Only ad groups of automated campaigns get them.

## Statuses written

| File | Rows | Status |
|---|---|---|
| `D1`, `D4`, `D5` | New | `Enabled` |
| `D2` | Pause | `Paused` |
| `D3` | Enable | `Enabled` |
| `S1` | New campaign | `Paused`, even when `export.status` is `Enabled` |
| `S2` | New campaign | `Enabled` |
| `S3` | New ad group | `Enabled` |
| `S4` | New keyword | `Paused` |
| `S5` | New ad | `Paused` |

## Over a live account

A run from `mads optimize` writes the same package. The `action` column of the `Campaigns` tab and of the fixed `Feed` rows is `Add`, `Edit` or `Pause`. A `Pause` is written as `Edit` with status `Paused`. What counts as live, new and dropped is decided by the same code as the Editor file, see [Editor file](editor-file.md).

| File | Live item | New item | Item the new run dropped |
|---|---|---|---|
| `S1` | `Edit`, status blank | `Add`, `Paused` | `Edit`, `Paused` |
| `S2` | No row | `Edit`, `Enabled` | No row |
| `S3` | `Edit`, status blank | `Add`, `Enabled` | `Edit`, `Paused` |
| `S4` | `Edit`, status blank | `Add`, `Paused` | `Edit`, `Paused` |

A blank status leaves the status the account has.

| Case | Row |
|---|---|
| Campaign outside the run, with `--pause-outside-run`, enabled in the reports and not in the new account | A `Pause` row in `Campaigns`, so `S1` writes `Edit` and `Paused`. The row carries only the campaign key and name. |
| Campaign outside the run, without the flag | No row. |
| Campaign the run removed | Never touched. |
| Dropped ad group of a fixed Search campaign | A `Pause` row with no keyword, so `S3` pauses the group. |
| Dropped keyword of a fixed Search campaign | A `Pause` row with the keyword, so `S4` pauses the keyword and `S3` keeps the group. |
| Dropped item of an automated campaign | The item stays in `Catalog` with `active` `No`, so the daily track pauses its ad group. |
| Image campaign | `Edit` when live, `Add` when new, `Pause` when dropped. |

With a keywords report, `S4` pauses the enabled keywords that report lists and the new run does not carry, Broad included. Without it, `S4` pauses the keywords the base run had and the new run dropped. Keywords of automated campaigns have no row. `S5` and `S6` write a row only where the first fixed keyword of an ad group is new.

Which campaigns `--pause-outside-run` flags is described in [Optimize a live account](../how-to/optimize-a-live-account.md).

## Account fields

All keys of `[google_ads]` are optional. Their types and validation are in the [business.toml reference](business-toml.md#google_ads).

| Key | Effect in the package |
|---|---|
| `customer_id` | The `Customer ID` setting. Empty or `000-000-0000` leaves it blank and adds `W11`. |
| `tracking_template` | Replaces `{mads_campaign}` with the campaign slug on each campaign row. In `Settings` (account wide) it becomes `{campaignid}`. When empty, the template is `{lpurl}?` plus `export.url_suffix`, or the suffix as is when it starts with `{lpurl}`. |
| `devices` | Written on the campaign rows when set. |
| `labels` | Written when set, joined with `;`. |
| `app_id` | Fills `App ID` and `App store` in `Settings`. A value of only digits is `Apple App Store`, anything else is `Google Play`. Without it, `S8` is not written. |

`Settings` also holds `Domain` (`business.url`), `Keyword match type` (`Phrase`) and `Default Max CPC`. `Default Max CPC` is `budget.max_cpc`. Without it, it is the median bid of the automated ad groups the plan wrote, the lower of the two middle values when the count is even. It is blank when the plan has none.

Budgets and Max CPC use a dot and two decimals.

## Pending tab

The `Pending` tab of `README.xlsx` lists what the run lacks. It covers the Customer ID, the App ID, the Default Max CPC, headline 1 of catalog items over 30 characters, enabling the image campaigns after review, scheduling `main` and the Allow access step in each file. Each entry appears only when it applies.

## Limits and codes

| Code | When | Effect |
|---|---|---|
| `E29` | The automated block needs more than 1000 feed rows, or `Negatives` holds more than 2000 rows. | The export fails and no file is written. |
| `W11` | `google_ads.customer_id` is empty or `000-000-0000`. | The Customer ID setting stays blank. |
| `W12` | An image campaign lacks what its type needs: picture files, a logo for Performance Max or Demand Gen, or an app id for App. | The campaign is left out of `Image Campaigns` and `Image Assets`. |
| `W13` | A catalog item name is over 30 characters. | `headline_1` of its `Catalog` row stays empty. Headline 1 of its ad falls back to the template headline in `Ad Templates`. |

The automated block holds `max(150, ceil(1.5 * catalog rows))` rows. `E29` applies when that number passes 1000. The messages are in [Validation rules](validation-rules.md).

The package is built in memory first. Any error returns no file. Character limits and `--max-ad-groups` still apply.

## Export cleanup

Every export first removes the CSVs of `bulk/` and `editor/`, the five files an older version wrote in the run folder, and the whole `automation/` folder. It then writes the new layout. A failed export therefore leaves none of the sets. `editor/images/` stays.

## Next

* [Use the ads automation package](../how-to/use-the-automation-package.md)
* [Optimize a live account](../how-to/optimize-a-live-account.md)
* [Output files](output-files.md)
