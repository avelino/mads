# Ads automation package

This page shows how to write the ads automation package, set it up once in Google Drive and Google Ads, create the image campaigns by script, and run the daily routine.

`mads generate`, `mads export` and `mads optimize` take `--layout drive-folders`. It writes a folder of spreadsheets and a Google Ads script instead of files 1 to 5 and `editor/account.csv`. Without the flag, `export` and `--resume` keep the layout the run was written with, and `optimize` keeps the layout of its base run.

```bash
mads generate business.toml --provider anthropic --model <model-id> --layout drive-folders
mads export out/<run-id> --layout drive-folders
```

mads writes local files and stops there. It never calls Google. Google Sheets formulas and a Google Ads script run the day to day. You paste the Search output into Google Ads Editor. The script creates the image campaigns itself.

## What it is

The package splits the Search campaigns into two tracks.

- **Automated campaigns** are the Search campaigns with `intent` `catalog`. Each catalog item gets its own ad group with one keyword and one ad. The `Catalog` tab of `0. Feed` drives them. When an item is added, retired or comes back, the `Daily/` files hold the change.
  The keyword is the planned keyword that matches the item name, else the item name. The other keywords the plan wrote for that catalog ad group are not in the package. A planned ad group that lists no catalog item stays as a fixed group of the same campaign.
- **Fixed campaigns** are every other Search campaign. Their ad groups, keywords, ads and negatives are written once, in the `Structure/` files.

- **Image campaigns** are the Performance Max, Demand Gen and App campaigns. `setupImageCampaigns` in `daily-sync.js` creates them from the `Image Campaigns` and `Image Assets` tabs of `0. Feed` and the pictures in `Images/`. You do not need Google Ads Editor for them. See [Image campaigns by script](#image-campaigns-by-script).

## Files

```text
out/<run>/google-ads/automation/<Business> Ads Automation/
  Images/
    logo.png
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

`<Business>` is `business.name` without the characters a file name cannot hold (`/ \ : * ? " < > |` and control characters). The script finds the folder by this exact name, so do not rename it. `S8` is written only when `google_ads.app_id` is set. `Images/` is written only when an image campaign has pictures.

| File | What it holds |
|---|---|
| `0. Feed` | The single source. Tabs `Settings`, `Catalog`, `Campaigns`, `Fixed Keywords`, `Ad Templates`, `Negatives`, `Feed`, `Account Status`, `Auto Negatives`, `Updated`, `Image Campaigns` and `Image Assets`. |
| `Images/` | The pictures of the image campaigns, one folder per `campaign_key`, and the logo. Copies of the files in `editor/images/`. |
| `README` | Tabs `Routine`, `How actions are decided`, `Files`, `Linking` and `Pending`. |
| `daily-sync.js` | The Google Ads script. `linkSheets` links the files once. `setupImageCampaigns` creates the image campaigns. `main` reads the account every night. |
| `D1` to `D3` | Ad groups of automated campaigns to create, pause or enable. |
| `D4` to `D6` | The keyword, the ad and the tracking template of each new automated ad group. |
| `D7` | Negatives that keep automated ad groups apart, rebuilt by the script every night. |
| `S1`, `S2` | The campaigns, created paused, and the file that enables them. |
| `S3` to `S6` | Ad groups, keywords, ads and tracking templates of the fixed campaigns. |
| `S7` | Campaign and ad group negatives. |
| `S8` | The app extension on the new campaigns. |

Every tab has a title on row 1, a sentence on row 2, a note on row 3, the header on row 4 and data from row 5. Each `D` and `S` file has two mirror tabs that copy `0. Feed` through `IMPORTRANGE`, plus one output tab. Every output cell is a formula, and rows a formula leaves empty stay blank. Never edit the mirror tabs.

Cells you must fill before going live have a yellow fill. The `Pending` tab of `README.xlsx` lists each one.

A successful `bulk` export removes the `automation/` folder. A successful `drive-folders` export removes files 1 to 5 and `editor/account.csv`. Pictures in `editor/images/` stay, and `Images/` holds a copy of each. A failed export (exit `3`) removes both sets.

## One-time setup

`README.xlsx` has the same steps in its `Linking` and `Routine` tabs.

Uploading alone links nothing. Until `linkSheets` runs, cell `A5` of every mirror tab holds `IMPORTRANGE("FEED_ID", ...)` with a literal `FEED_ID`, and the output tabs have nothing to read. This also holds when an agent uploads the folder for you. mads writes files before they exist in Drive, so it cannot know the id of `0. Feed`.

1. In Google Drive settings, turn on Convert uploads. Upload the whole folder without renaming it. The script cannot read an `.xlsx` that was not converted, and its log says so.
2. In Google Ads, Tools, Scripts, create a script and paste `daily-sync.js`.
3. Run `linkSheets` once. It finds the folder by name, points every mirror tab at `0. Feed` and logs each file as `linked` or `FAILED`. Running it again is safe.
4. When the account has image campaigns, run `setupImageCampaigns` in Preview, read the log, then run it. See [Image campaigns by script](#image-campaigns-by-script).
5. Open each `D` and `S` file and click Allow access. Google asks once per file, the first time it reads `0. Feed`. No API can do this step.
6. In Google Ads Editor, get the recent changes of the account. Paste the output tab of `S1` through `S7` (or `S8`), in that order. Hold `S2` until you are ready to go live, because it enables the new campaigns.
7. Fill what `Pending` lists, such as Customer ID and Default Max CPC. Review the Editor preview, then post.
8. Schedule `main` in the script to run daily, at night.
9. Check cell `A5` of the mirror tabs in each `D` and `S` file. It must hold data. `FEED_ID`, `#REF!` or a `Loading...` that never ends means step 3 or step 5 did not happen for that file.

`S1` creates campaigns paused even when `export.status` is `Enabled`. Fixed ad groups in `S3` are `Enabled`. Fixed keywords in `S4` and ads in `S5` are `Paused`, so you enable them after review.

## Image campaigns by script

`setupImageCampaigns` reads the `Image Campaigns` and `Image Assets` tabs of `0. Feed`, the pictures in `Images/` by file name, and the campaigns the account already has, by name.

`Image Campaigns` has one row per Performance Max, Demand Gen or App campaign of the plan. `Image Assets` has one row per text or picture of each asset group (ad group for Demand Gen and App). Every group of a Performance Max or Demand Gen campaign gets the logo and the business name.

1. In Google Ads, Tools, Scripts, open the script and select `setupImageCampaigns`.
2. Click Preview. Preview sends the same requests and applies none of them.
3. Read the log. It has one line per campaign: `created`, `budget updated`, `paused`, `unchanged`, or `FAILED <campaign>: <API error>`. A missing picture fails only its campaign, and the message names the file.
4. Fix what failed, then click Run.
5. Review each new campaign in Google Ads and enable it.

Each new campaign is one request with every operation it needs: assets, budget, campaign, asset groups or ad groups, ads and links. Google applies all of it or none of it, so a failure leaves nothing of that campaign. The script then moves to the next campaign.

New image campaigns are created paused. After that the script only updates their budget. It never edits their texts or pictures. Running it again is safe: a campaign that exists by name is never created twice.

mads cannot call the Google Ads API, so these requests were checked only against the API reference and in a Preview run. Read the Preview log before the first real run.

## Daily routine

1. Every night `main` reads the account. It writes `Account Status` (every ad group of the automated campaigns, `Enabled` or `Paused`) and `Auto Negatives`. It never changes the account.
2. In the morning the `Feed` tab compares what the catalog wants with what the account has and fills `next_step`.
3. Open `D1`, `D2` and `D3`. Paste into Editor only the files whose output tab has rows. When `D1` has rows, paste `D4`, `D5`, `D6` and `D7` too. Most days none have rows.

Daily keywords and ads are `Enabled`, because a new catalog item is live on the site that day.

To add or retire an item, edit the `Catalog` tab of `0. Feed`. A new row needs `item`, `campaign_key`, `active`, `keyword` and `final_url`. Set `active` to `No` and the next day pauses the item's ad group. mads fills `Catalog` once, from `catalog.csv`. After that a person or your app owns it.

## How actions are decided

One table in the code generates the `next_step` formula. The `How actions are decided` tab of `README.xlsx` renders the same table.

| Feed wants (`action`) | Account has (`current_status`) | `next_step` | Daily file |
|---|---|---|---|
| `Add` | ad group not in the account | `CREATE` | D1, D4, D5, D6 |
| `Add` | `Paused` | `ENABLE` | D3 |
| `Add` | `Enabled` | empty | none |
| `-` | `Enabled` | `PAUSE` | D2 |
| `-` | `Paused` | empty | none |
| `-` | ad group not in the account | empty | none |

`action` is `Add` when the `Catalog` row has an item and `active` is `Yes`, else `-`. `next_step` is empty on every fixed row.

`Auto Negatives` works on pairs of feed keywords. When keyword B contains keyword A as a phrase, B becomes a `Phrase` negative in A's ad group, plus B spelled without accents. Only ad groups of automated campaigns get them.

## Over a live account

A run from `mads optimize` writes the same package, but the structural rows change an account that already serves. The campaigns report decides what is live. The `action` column of `Campaigns` and of the fixed `Feed` rows is `Add`, `Edit` or `Pause`.

| File | Live item | New item | Item the new run dropped |
|---|---|---|---|
| S1 | `Edit`, Campaign status blank, so Editor keeps the live status | `Add`, `Paused` | `Edit`, `Paused` |
| S2 | No row | `Edit`, `Enabled` | No row |
| S3 | `Edit`, status blank | `Add`, `Enabled` | `Edit`, `Paused` |
| S4 | `Edit`, Status blank | `Add`, `Paused` | `Edit`, `Paused` |

`S5` and `S6` write a row only where the first fixed keyword of an ad group is new. Automated campaigns need no structural row.

`Image Campaigns` follows the same rule. The `action` of a live image campaign is `Edit`, and `setupImageCampaigns` updates its budget when it differs and changes nothing else. A new one is `Add` and is created paused. An image campaign the new plan dropped gets a `Pause` row, and the script pauses it when it is enabled. Assets of a campaign that already exists are not edited, so change them in Google Ads. An item the new plan dropped from a live automated campaign stays in `Catalog` with `active` `No`, so the daily track pauses its ad group.

## Account fields

```toml
[google_ads]
customer_id = "123-456-7890"
devices = "Mobile;Desktop;Tablet"
labels = ["estrutural"]
tracking_template = "{lpurl}?utm_campaign={mads_campaign}&utm_term={keyword}"
app_id = "com.vinellu.app"
```

Every key is optional. See the [business.toml reference](../reference/business-toml.md#google_ads).

- An empty `customer_id` leaves the Customer ID setting blank and adds warning `W11`. mads does not write the placeholder `000-000-0000`.
- `tracking_template` replaces `{mads_campaign}` with the campaign slug on each campaign row. The `Settings` tab is account wide, so there it becomes the ValueTrack parameter `{campaignid}`. When the key is empty, the template is `{lpurl}?` plus `export.url_suffix`.
- `devices` and `labels` are written when set. Labels are joined with `;`.
- `app_id` is a Google Play package or a numeric App Store id. Without it, `S8` is not written.

Budgets and Max CPC use a dot and two decimals (`50.00`, `1.50`).

## What is pending

The `Pending` tab of `README.xlsx` lists what this run lacks.

- Customer ID, when it is blank.
- App ID, when the package has automated campaigns and no app id.
- Default Max CPC, when `budget.max_cpc` is not set and the plan has no catalog ad group to take a bid from. Without `budget.max_cpc`, mads uses the median Max CPC of the catalog ad groups the plan bid on. The campaigns use Manual CPC, and Editor rejects an ad group without a bid.
- Headline 1 of each catalog item whose name is over 30 characters.
- Enable the image campaigns after review, when `setupImageCampaigns` creates any.
- Scheduling `main`, and Allow access in each file.

## Limits

| Code | When | Effect |
|---|---|---|
| `E29` | The automated block needs more than 1000 feed rows, or `Negatives` holds more than 2000 rows. | The export fails with exit `3` and writes no file. |
| `W11` | `google_ads.customer_id` is empty or `000-000-0000`. | The Customer ID setting stays blank. |
| `W12` | An image campaign lacks what its type needs: picture files, a logo for Performance Max or Demand Gen, or an app id for App. | The campaign is left out of `Image Campaigns` and `Image Assets`. The warning names what is missing. |
| `W13` | A catalog item name is over 30 characters. | Headline 1 of its ad falls back to the campaign's template headline in `Ad Templates`. |

The automated block holds `max(150, ceil(1.5 * catalog rows))` rows, so the catalog can grow without a new export. `main` throws when `Account Status` would pass 1000 rows or `Auto Negatives` 2000, because a half-read account turns into `CREATE` for ad groups that exist.

`setupImageCampaigns` sends one request per campaign. It does not edit assets of a campaign that exists, rotate assets by performance, upload video, or touch Shopping feeds.

Character limits and `--max-ad-groups` still apply. The package is built in memory first, so any error exits `3` and writes no file. See [Validation rules](../reference/validation-rules.md) for the exact messages.
