# Use the ads automation package

Export a run as the ads automation package, link it once in Google Drive and Google Ads, and keep your catalog campaigns in sync with a daily script.

The package replaces `bulk/` and `editor/account.csv`. It is a folder of spreadsheets plus a Google Ads script. mads writes the files and never calls Google. Spreadsheet formulas and the script do the daily work. For what each file and tab holds, see the [Ads automation package reference](../reference/automation-package.md).

## Before you start

- A finished run with the status `Ready to import` in `report.md`. See [Review a run](review-a-run.md).
- A Google Ads account and Google Drive. See [Prepare your Google Ads account](prepare-google-ads.md).
- Google Ads Editor installed, for the Search campaigns. The image campaigns do not need it.
- A computer. Google Drive's conversion setting cannot be changed from a phone or tablet.

Exporting again calls no model and spends no tokens. `mads generate` and `mads optimize` do call a model. Both take the same `--layout` flag if you prefer to start a new run in this layout.

## Steps

### 1. Export the run as a package

```bash
mads export out/<run-id> --layout drive-folders
```

The package lands in `out/<run-id>/google-ads/automation/<Business> Ads Automation/`. `<Business>` is `business.name` without the characters a file name cannot hold. The script finds the folder by this exact name, so do not rename it.

An export in this layout removes `bulk/` and `editor/account.csv`. An export in the other layout removes `automation/`. Without `--layout`, `export` keeps the layout the run was written with. See [Output files](../reference/output-files.md).

### 2. Fill what is pending

Open `README.xlsx` and read its `Pending` tab. Cells you must fill before going live have a yellow fill. The tab lists only what applies to your run, for example the Customer ID or the Default Max CPC. You can also set `customer_id` and `budget.max_cpc` in `business.toml` and export again. See the [`[google_ads]` reference](../reference/business-toml.md#google_ads).

### 3. Upload the folder to Drive

1. In Google Drive settings, select Convert uploads to Google Docs editor format. Google describes it in [Upload files and folders to Google Drive](https://support.google.com/drive/answer/2424368).
2. Upload the whole `<Business> Ads Automation` folder without renaming anything.

The script cannot read a file that was not converted, and its log says so. Uploading links nothing yet. Until step 5, every mirror tab holds the literal `FEED_ID`.

### 4. Create the script

1. In Google Ads, open Tools, then Bulk actions, then Scripts.
2. Press the plus icon and select New script.
3. Paste the whole of `daily-sync.js` and save.
4. When Google asks, click Authorize.

Google documents these steps in [Get started with Google Ads scripts](https://developers.google.com/google-ads/scripts/docs/getting-started).

### 5. Link the files

Select `linkSheets` and click Run. It finds the folder by name, points every mirror tab at `0. Feed` and logs each file as `linked` or `FAILED`. Running it again is safe.

### 6. Allow access in each file

Open every file in `Daily/` and `Structure/`. Click Allow Access when Google asks. Google asks once per file, the first time it reads `0. Feed`. No script can do this for you. Google describes it in [IMPORTRANGE](https://support.google.com/docs/answer/3093340).

### 7. Create the image campaigns

Skip this step when the run has no Performance Max, Demand Gen or App campaign. The pictures are in `Images/`.

1. In the script, select `setupImageCampaigns` and click Preview. Preview is Google's test run of a script.
2. Read the log. It has one line per campaign: `created`, `budget updated`, `paused`, `unchanged`, or `FAILED <campaign>: <API error>`. A missing picture fails only its campaign and the message names the file.
3. Fix what failed, then select `setupImageCampaigns` and click Run.

Each new campaign goes up in one request. Google applies all of it or none of it, so a failure leaves nothing of that campaign. New campaigns are created paused. Running it again never creates a campaign twice, because it matches by name.

mads cannot call the Google Ads API. These requests were checked only against the API reference, so read the Preview log before the first real run.

### 8. Paste the Search output into Editor

1. In Google Ads Editor, get the recent changes of the account. See [Review recent account changes](https://support.google.com/google-ads/editor/answer/30582?hl=en).
2. In each of `S1` to `S7` (and `S8`, when it exists), copy the output tab. Select Account, then Import, then Paste text, and paste. Google describes this in [Import a CSV file](https://support.google.com/google-ads/editor/answer/30564?hl=en).
3. Go in the order `S1`, `S2`, `S3`, `S4`, `S5`, `S6`, `S7`, `S8`. Hold `S2` until you are ready to go live, because it enables the new campaigns.
4. Review the imported changes, then post.

`S1` creates campaigns paused even when `export.status` is `Enabled`. Fixed keywords in `S4` and ads in `S5` come in paused. Enable them after review. See [Go live](go-live.md).

### 9. Schedule the daily script

In the script, schedule `main` to run daily, at night. `main` reads the account and writes the `Account Status` and `Auto Negatives` tabs. It never changes the account. Google lists the scheduling options in [Get started with Google Ads scripts](https://developers.google.com/google-ads/scripts/docs/start).

## Run the daily routine

1. In the morning, the `Feed` tab of `0. Feed` compares what the catalog wants with what the account has and fills `next_step`.
2. Open `D1`, `D2` and `D3`. Paste into Editor only the files whose output tab has rows.
3. When `D1` has rows, paste `D4`, `D5`, `D6` and `D7` too.
4. Review and post. Most days no file has rows.

To add an item, add a row to the `Catalog` tab of `0. Feed` with `item`, `campaign_key`, `active`, `keyword` and `final_url`. To retire one, set `active` to `No`. The next day pauses its ad group. mads fills `Catalog` once, from `catalog.csv`. After that you or your app own it. The table that maps `action` and `current_status` to `next_step` is in the [reference](../reference/automation-package.md#how-actions-are-decided).

## After an optimize run

`mads optimize` keeps the layout of its base run and writes the same package. A campaign the new plan dropped is paused, and nothing is deleted. A campaign outside the run gets a `Pause` row only with `--pause-outside-run`. The rows for live, new and dropped items are in [Over a live account](../reference/automation-package.md#over-a-live-account). The steps are in [Optimize a live account](optimize-a-live-account.md).

Assets of an image campaign that already exists are not edited. Change them in Google Ads.

## Check it worked

- The log of `linkSheets` shows `linked` for every file and no `FAILED`.
- Cell `A5` of the mirror tabs in each `D` and `S` file holds data. `FEED_ID`, `#REF!` or a `Loading...` that never ends means step 5 or step 6 did not happen for that file.
- After step 8, Editor's preview lists your campaigns, ad groups, keywords and ads, and every new campaign is paused.
- After the first night, `Account Status` in `0. Feed` lists the ad groups of your automated campaigns.

## Next

- [Go live](go-live.md)
- [Optimize a live account](optimize-a-live-account.md)
- [Write a catalog](write-a-catalog.md)
