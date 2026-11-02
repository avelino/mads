# Import into Google Ads Editor

This page shows how to take a finished run and load it into Google Ads Editor, attach the pictures and post it to your account.

## Install Google Ads Editor

Google Ads Editor is Google's free desktop app for Windows and macOS. [Download it here](https://ads.google.com/home/tools/ads-editor/), install it and sign in with the Google account that manages the ad account.

mads writes `google-ads/editor/account.csv` the way Editor writes its own exports. Its Search, App and Demand Gen rows went through a real import in Editor 2.13.3 with no error. Performance Max rows follow Google's documented headers and have not gone through a real import yet, so read the preview.

## Pick the file

| Your account | Import | Where |
|---|---|---|
| Any mix of Search, App, Demand Gen and Performance Max | `google-ads/editor/account.csv` | Google Ads Editor |
| Search only, no Editor | `google-ads/1-campaign.csv` to `5-responsive-search-ads.csv` | Web bulk upload, see [Review and import](review-and-import.md#upload-in-google-ads) |

Import one or the other, never both. `account.csv` already holds the Search campaigns, so uploading files 1 to 5 as well adds each one twice.

## Before you import

1. Read `report.md` first. The status line must say `Ready to import`. See [Review and import](review-and-import.md).
2. Check `export.status` in `business.toml`. The default is `Paused`, so new campaigns arrive paused and nothing serves until you enable it. Keep it for your first runs.
3. On a run made by [`mads optimize`](optimize.md), open the `Changes` section of `report.md`. It lists what the import will pause, add or leave alone.

## Import the account

1. Open Google Ads Editor and select the account.
2. Click Get recent changes, so Editor starts from what is live. Skipping this makes the import compare against old data.
3. Go to Account, Import, From file, and pick `google-ads/editor/account.csv`.
4. Read the import preview. The counts of campaigns, ad groups, keywords and ads must match what the Summary in `report.md` says. Fix anything flagged, then keep the changes.

For how Editor reads CSV files, see Google's page on [importing and exporting CSV files](https://support.google.com/google-ads/editor/answer/53082?hl=en).

## Attach the pictures

Pictures cannot travel in a CSV. Editor neither exports nor imports the link between an ad and its images. mads writes the texts and the structure, and you attach the pictures. Skip this section when the account has no App, Demand Gen or Performance Max campaign. In that case `editor/images/` does not exist.

1. Go to Account, Import, Image assets from files, and select the folder `google-ads/editor/images`.
2. Choose to import the image assets to the root folder. The pictures sit in one subfolder per campaign, which the account does not have. The default skips them and only `logo.png` comes in.
3. Open the Images table in `report.md`. It lists every ad and asset group with the files it takes.
4. For each one, open its Images field in Editor and pick those files. Demand Gen ads and Performance Max asset groups also take `logo.png`.

Do this before you post. An App or Demand Gen ad without its pictures fails to post, and its ad group goes up empty with no active ads. Editor adds the files to the library by name, and the name carries the ad group, so none overwrites another.

Pictures are JPEG at Google's recommended sizes. See [Image campaigns](image-campaigns.md).

## Post

1. Check the Errors count in Editor. It must be zero.
2. Click Post. Editor sends the changes to the account.
3. If an ad failed, filter Ads by errors, attach what is missing and post again. See [Troubleshooting](../howto/troubleshooting.md#an-app-or-demand-gen-ad-group-has-no-ads).

If Google refuses a keyword or an ad in a restricted category, the `Restricted categories` section of `report.md` has the exception text to paste. See [Restricted categories](restricted-categories.md#when-google-refuses-a-keyword-or-an-ad).

## What `mads optimize` does to a live account

The import changes what the file lists, and an item the file does not name keeps serving.

- A live campaign keeps its status. A new campaign follows `export.status`.
- A campaign, ad group, asset group or keyword that the new run dropped arrives as a row with status `Paused`.
- A responsive search ad with new texts comes in as another ad next to the old one. The old one stays enabled, so pause it by hand after you check the preview.
- A negative keyword that the new run dropped stays in Google Ads. Remove it by hand.

## Turn it on

Open the paused campaigns, check budget, bidding and location, and enable one at a time. A new campaign serves only after Google approves its ads.

Not in the files yet, so add them in the Google Ads interface:

- Sitelinks, callouts and structured snippets. mads validates them and does not export them.
- Conversion tracking. Without it Google Ads shows clicks and cost, but not which searches bring customers.

After 2 days, look at the keyword status and the first page bid estimate. After 14 days, export the reports and run [`mads optimize`](optimize.md) again.

## Without Google Ads Editor

`--layout drive-folders` writes an ads automation package instead of `account.csv`. A Google Ads script in Drive creates the campaigns, and you attach no picture by hand. See [Ads automation package](../howto/google-ads-editor-drive-folders.md).
