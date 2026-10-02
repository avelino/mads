# Image campaigns

This page shows how mads builds Performance Max and Demand Gen campaigns with generated pictures, what it needs from you, and how to import the result.

## What you get

The plan agent picks a format for each campaign. Search stays the default. An image format is an option when the run has an image model and a logo.

| `kind` | Where the ads show | Bidding | When the agent picks it |
|---|---|---|---|
| `search` | Google search results, text only | `manual_cpc` | People already search for what you sell. |
| `demand_gen` | YouTube, Discover and Gmail feeds | `maximize_clicks`, or `maximize_conversions` with tracking | Demand has to be created and the product shows well in a picture. |
| `performance_max` | Search, YouTube, Display, Discover, Gmail and Maps | `maximize_conversions` | `conversion_tracking = true` and the budget lets Google learn. |

For an image campaign the agent writes the texts of each asset group and one brief per picture. mads then draws the pictures, crops them to Google's sizes and writes a CSV for Google Ads Editor.

The agent never sees a picture. It writes briefs, and code turns them into files after the missions end.

## When the agent picks images

The plan funds the Search campaigns with the best expected return first. An image campaign needs at least 20 percent of the daily budget, so the agent plans one only when that much is left. Performance Max also needs `conversion_tracking = true`. Demand Gen does not.

So a run with an image model and a logo can still end with Search only. The plan agent says why in its last message, in `transcripts/plan.*`.

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

Every picture is a paid API call. `--max-images` (default 40) caps the new pictures of one run. Briefs over the cap get no file, the report lists them, and the export stops with `E20`.

Pictures are kept. `mads generate --resume` and `mads export` reuse every file that still passes the checks and only draw what is missing. `mads export` has no image model, so a missing picture there is an `E20` error.

Each picture gets 2 attempts. A failure, a refused prompt or a reference photo that cannot be downloaded fails only that picture.

## Output

```text
out/<run>/google-ads/
  1-campaign.csv ... 5-responsive-search-ads.csv   Search campaigns only
  editor/
    image-campaigns.csv                            Performance Max and Demand Gen
    images/
      logo.png
      <campaign>/<asset-group>-<image-id>.jpg
```

Pictures are JPEG at Google's recommended size: landscape 1200x628, square 1200x1200, portrait 960x1200, vertical 1080x1920.

`report.md` has an `Images` section with the model, how many pictures were generated and reused, and the file of every brief.

## Import

The bulk upload page of Google Ads takes no image files. Google Ads Editor does: its CSV import reads image paths relative to the CSV.

1. Open Google Ads Editor and download the account.
2. Account, Import, From file, and pick `editor/image-campaigns.csv`. Keep the `images/` folder next to it.
3. Review the changes, then post them.

Upload files 1 to 5 for the Search campaigns as before. See [Review and import](review-and-import.md).

> The columns of `image-campaigns.csv` follow Google's documented Editor headers. They are not verified against a real Editor export yet. Check the import preview before posting.

## Rules

Image campaigns have their own limits, enforced by the tools and again before export. See `E16` to `E20`, `W06` and `W07` in [Validation rules](../reference/validation-rules.md).

They take no negatives, sitelinks, callouts or snippets in this version, and the cross negatives step skips them.
