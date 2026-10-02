# Image campaigns

This page shows how mads builds Performance Max, Demand Gen and App campaigns with generated pictures, what it needs from you, and how to import the result.

## What you get

The plan agent picks a format for each campaign. Search stays the default. An image format is an option when the run has an image model and a logo.

| `kind` | Where the ads show | Bidding | When the agent picks it |
|---|---|---|---|
| `search` | Google search results, text only | `manual_cpc` | People already search for what you sell. |
| `demand_gen` | YouTube, Discover and Gmail feeds | `maximize_clicks`, or `maximize_conversions` with tracking | Demand has to be created and the product shows well in a picture. |
| `performance_max` | Search, YouTube, Display, Discover, Gmail and Maps | `maximize_conversions` | `conversion_tracking = true` and the budget lets Google learn. |
| `app_installs` | Google Play or App Store, YouTube, Search and Display | `maximize_conversions` | The goal is installs and `business.toml` has `[app]`. |

An App campaign needs `[app]` instead of a logo: Google shows the app's name and icon from the store, and every ad links to the store page. `mads init` writes `[app]` when the site links to its app on Google Play or the App Store.

```toml
[app]
store = "google_play"   # or "app_store"
id = "com.vinellu.app"  # the package name, or the numeric App Store id
```

For an image campaign the agent writes the texts of each asset group and one brief per picture. mads then draws the pictures, crops them to Google's sizes and writes the whole account, Search included, into one Google Ads Editor file.

The agent never sees a picture. It writes briefs, and code turns them into files after the missions end.

## When the agent picks images

With an image model and a logo, the default is one image campaign with at least 20 percent of the daily budget, reserved before the Search campaigns get the rest. The agent may leave it out only with a reason: then a campaign rationale starts with `No image campaign:`, and the report shows it under Budget and bids. A plan without an image campaign and without that reason is refused (`NO_IMAGE_REASON`).

With `[app]`, the default is an `app_installs` campaign, because it optimizes for installs and a Demand Gen ad that says "download the app" does not. The agent may leave it out only with `No app campaign: <reason>` in a rationale (`NO_APP_REASON`). With `[campaigns] formats` neither reason is asked: your list decides. `mads init` and `mads generate` warn when `formats` leaves out `app_installs` while `[app]` is there. Performance Max also needs `conversion_tracking = true`. Demand Gen does not.

To decide yourself, list the formats in `business.toml`:

```toml
[campaigns]
formats = ["search", "demand_gen"]
```

The plan must then have at least one campaign of each. The agent still picks the budget share and the groups. See [business.toml reference](../reference/business-toml.md#campaigns).

## What mads needs

Two things. Without either one the plan can only use Search, and `get_business` tells the agent why.

**A logo.** Google shows it next to every image ad. mads never generates one. `mads init` downloads it from the site into `brand/logo.png` and writes:

```toml
[brand]
logo = "brand/logo.png"
```

Replace the file with your real logo when the downloaded one is wrong. It must be PNG or JPEG, square, at least 144x144 and at most 150 KB. `mads generate` refuses a logo that breaks any of these with a `brand.logo` error.

**An image model.** Set one key:

```bash
export GEMINI_API_KEY=...   # gemini-2.5-flash-image by default
export OPENAI_API_KEY=...   # gpt-image-1 by default
```

`--image-provider auto` (the default) uses Gemini when `GEMINI_API_KEY` is set, then OpenAI when `OPENAI_API_KEY` is set, else no image model. Pick one with `--image-provider gemini` or `--image-provider openai`, a model with `--image-model`, or turn images off with `--image-provider none`.

`mads providers` lists the image providers and the key each one is missing:

```text
IMAGE PROVIDER         STATUS
gemini                 missing GEMINI_API_KEY
openai                 missing OPENAI_API_KEY
none                   ready
```

## Brand identity

Pictures must look like your brand, not like a stock photo of your category. `DESIGN.md` carries the identity:

```markdown
# Design: Buser

## Colors

- Pink #EE395D: logo
- Dark pink #AD1457: theme color of https://www.buser.com.br

## Style

Young, colorful, close to the customer.
```

- `mads init` writes it. The colors come from code: the dominant colors of the downloaded logo and the `theme-color` the site declares. The agent adds `Style`, `Imagery`, `Voice` and `Avoid` with the `write_design` tool, from what it read.
- Edit it freely. Fix a color, add one, rewrite the style. mads reads every hex code under `## Colors`.
- The image campaign agent reads the whole file and puts a brand color on the main object of most scenes.
- Every prompt sent to the image model also ends with the colors, such as `Brand colors: pink (#EE395D), dark pink (#AD1457).`, in case a brief forgets them.

Without `[design]` in `business.toml`, pictures get no brand colors.

## Real product photos

An image model asked to draw a specific product invents its label, package or screen. mads avoids that.

Give a catalog item a real photo in the `image` column of `catalog.csv`. `mads init` fills it from the `og:image` of the item page. A brief can then set `reference` to that item's id: the photo goes to the model, which places the real product in the scene the brief describes.

A brief without `reference` shows people, places and results, not the product. A `reference` to an item without a photo fails with `E19`.

## Cost and limits

Every picture is a paid API call. `--max-images` (default 40) caps the new pictures of one run. The cap is shared across asset groups: the first picture of every group, then the second of every group, and so on, so a group with many briefs cannot leave another with none. A brief over the cap is removed when its asset group still has Google's minimum pictures without it, and the report notes it. One the group cannot do without gets no file and stops the export with `E20`. `mads generate --resume <run-dir>` with a higher cap draws it.

Pictures are kept. `mads generate --resume` and `mads export` reuse every file that still passes the checks and only draw what is missing. `mads export` has no image model, so a missing picture there is an `E20` error.

Each picture gets 2 attempts. A failure, a refused prompt or a reference photo that cannot be downloaded fails only that picture.

## Output

```text
out/<run>/google-ads/
  1-campaign.csv ... 5-responsive-search-ads.csv   Search campaigns, for the web bulk upload
  editor/
    account.csv                                    every campaign, for Google Ads Editor
    images/
      logo.png
      <campaign>/<asset-group>-<image-id>.jpg
```

`account.csv` is written the way Editor writes its own exports: UTF-16 with a byte order mark, tab separated, money as `250.00`. Its Search and App rows are checked against a real Editor 2.13.3 export of a live account.

Pictures are JPEG at Google's recommended size: landscape 1200x628, square 1200x1200, portrait 960x1200, vertical 1080x1920.

`report.md` has an `Images` section with the model, how many pictures were generated and reused, and the file of every brief, by campaign and ad or asset group. It is the list you follow to attach them.

## Import

Pictures cannot travel in a file. The web bulk upload takes no image files, and Google Ads Editor neither exports nor imports the link between an ad and its images: an App ad with four images comes out of Editor with no image column at all. So mads writes the texts and the structure, and you attach the pictures.

1. Open Google Ads Editor and get the recent changes of the account.
2. Account, Import, From file, and pick `google-ads/editor/account.csv`. Review the changes and keep them.
3. Account, Import, Image assets from files, and select the folder `google-ads/editor/images`. Choose "import image assets to the root folder": the pictures sit in one subfolder per campaign, which the account does not have, and the default ("do not import") skips them, leaving only `logo.png`. Editor adds them to the library with their file names, which carry the ad group, so none overwrites another.
4. For every ad and asset group in the Images table of `report.md`, open its Images field and pick the files listed there. Demand Gen ads and Performance Max asset groups also take `logo.png`. Do it before posting: an App or Demand Gen ad without its pictures fails to post, and its ad group goes up empty, with no active ads.
5. Post the changes. If an ad failed, filter Ads by errors in Editor, attach what is missing and post again. See [Troubleshooting](../howto/troubleshooting.md#an-app-or-demand-gen-ad-group-has-no-ads).

`account.csv` holds the Search campaigns too, so do not also upload files 1 to 5 on the web: that creates every Search campaign twice. Files 1 to 5 are for an account with Search only and no Editor. See [Review and import](review-and-import.md).

> A file with Search, App and Demand Gen campaigns imported into Google Ads Editor 2.13.3 with no error, and every count in the preview matched the file. Performance Max rows follow Google's documented headers and have not gone through a real import yet. Check the import preview.

## Rules

Image campaigns have their own limits, enforced by the tools and again before export. See `E16` to `E20`, `E24`, `W06` and `W07` in [Validation rules](../reference/validation-rules.md).

They take no negatives, sitelinks, callouts or snippets in this version, and the cross negatives step skips them.
