# Drive folder layout

This page shows how to write the structural Google Ads Editor files on disk and paste them by hand.

`mads generate` and `mads export` take `--layout`. The default is `bulk`, the five web files plus `editor/account.csv`. `drive-folders` writes a different folder and does not write those files. Without the flag, `export` and `--resume` keep the layout the run was written with. `mads optimize` refuses `drive-folders`, because these files only add and a live account needs pause rows.

The folder holds Search campaigns only. A Performance Max, Demand Gen or app install campaign is left out of every file, B1 and B2 included, and warning `W12` names it. Export that run with `--layout bulk` to get those campaigns.

```bash
mads generate business.toml --provider anthropic --model <model-id> --layout drive-folders
mads export out/<run-id> --layout drive-folders
```

Nothing is uploaded. The files stay in the run directory. You open Google Ads Editor and import them.

## Where the files go

```text
google-ads/drive/B - Estrutural/
  B1-campanhas.csv
  B2-status-campanha.csv
  B3-grupos.csv
  B4-keywords.csv
  B5-anuncios.csv
  B6-utm.csv
  B7-negativas.csv
  B8-extensao-app.csv
  LEIA-ME.md
```

`B8-extensao-app.csv` is omitted when `google_ads.app_id` is empty. `LEIA-ME.md` is UTF-8 and lists the paste order in Portuguese. The CSV files are UTF-16 LE with a byte order mark, tab separated, with LF line endings. That is the same encoding as `editor/account.csv`.

A successful export of `bulk` removes this folder. A successful export of `drive-folders` removes files 1 to 5 and `editor/account.csv`. Pictures in `editor/images/` stay. A failed export (exit `3`) removes both CSV sets.

## Paste order

Paste each file once, at setup, in Google Ads Editor under Account, Import, From file. The order is B1 through B8. Read `LEIA-ME.md` first.

| File | What it does |
|---|---|
| B1 | Creates the campaigns, all paused. |
| B2 | Sets those campaigns to Enabled. Hold this file until you go live. |
| B3 | Creates the ad groups. One group can hold several keywords. |
| B4 | Creates the keywords of each group. |
| B5 | Creates the responsive search ad of each group. |
| B6 | Writes the tracking template of each group. |
| B7 | Writes campaign negatives into every ad group, then the group's own negatives. A group created later in Editor does not inherit them. |
| B8 | Creates the app extension on each Search campaign. |

Campaigns are paused in B1 even when `export.status` is `Enabled`. B2 is an edit that sets Campaign status to Enabled. Ad groups are Enabled. Keywords in B4 and ads in B5 are Paused. Headline 1 position is 1. Enabling the campaign does not enable a paused keyword or ad. B1 has no `EU political ads` column. Confirm that in the Editor import preview before you post.

Editor matches campaigns and ad groups by name. These files do not carry Campaign ID or Ad group ID.

Search campaigns get rows in B3 through B8. Other campaign types get a row in B1 and B2 only. Character limits and `--max-ad-groups` still apply. An error still exits `3` and writes no file.

## Columns

B1, B3, B4 and B5 use the headers of the sheets this layout was copied from.

B2, B6, B7 and B8 use column names from [CSV file columns](https://support.google.com/google-ads/editor/answer/57747). The account id column is the one in [Make changes to multiple accounts](https://support.google.com/google-ads/editor/answer/7412706). The app row also follows [About app assets](https://support.google.com/google-ads/answer/2402582).

| File | Header |
|---|---|
| B2 | Action, Customer ID, Campaign, Campaign status |
| B6 | Action, Customer ID, Campaign, Ad group, Tracking template |
| B7 | Action, Customer ID, Campaign, Ad group, Keyword, Criterion Type |
| B8 | Action, Customer ID, Campaign, Link Text, App ID / Package name, App store, Final URL |

B2 writes Action `Edit` and Campaign status `Enabled`. The other files write Action `Add`.

B7 writes Criterion Type as `Negative Phrase` or `Negative Exact`. Campaign negatives are copied into every ad group of that campaign, campaign negatives first, with duplicates dropped. A group you add later in Editor does not get those rows.

B8 is a campaign-level asset, so the file has no Ad group column. A package name that contains a dot goes to Google Play. Digits only go to the Apple App Store. Link text is `Baixar o app` plus the business name when that fits in 25 characters, otherwise `Baixar o app`.

Budgets and Max CPC use a dot and two decimals (`50.00`, `1.50`). Keyword Max CPC is left blank. The bid is the ad group's Max CPC.

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

- An empty `customer_id` leaves the column blank and adds warning `W11`. The export still succeeds. mads does not write the placeholder `000-000-0000`. That value is rejected when you put it in the file.
- `tracking_template` replaces `{mads_campaign}` with the campaign slug. When the key is empty, the template is `{lpurl}?` plus `export.url_suffix`.
- `devices` and `labels` are written when set. Labels are joined with `;`, so a label cannot contain `;`.
- `app_id` is a Google Play package or a numeric App Store id. Without it, B8 is not written.

## Before you go live

`LEIA-ME.md` lists the same checks.

- Customer ID, when the column is blank.
- Max CPC. Manual CPC needs a bid or Editor refuses the ad group.
- The budget of each campaign.
- Final URL of the keywords and the ads.
- App id and the store URL, when you use B8.

This is the structural track only. mads does not write the daily feed, and it does not run an Ads script.
