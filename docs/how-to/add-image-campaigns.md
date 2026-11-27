# Add image and app campaigns

Set up a run so mads can plan Performance Max, Demand Gen and App campaigns next to Search, and draw the pictures they need.

Without these steps the plan uses Search only. To see how the agent chooses a format, read [Image and app campaigns](../explanation/image-campaigns.md).

## Before you start

- A `business.toml` that loads. See [Write the business file](write-the-business-file.md), or [draft it from your site](draft-from-your-site.md).
- A provider for the planning agent. See [Choose a provider](choose-a-provider.md).
- For Performance Max or Demand Gen, a square logo file. For App campaigns, the store id of your app.
- An API key for an image model, `GEMINI_API_KEY` or `OPENAI_API_KEY`.

Two steps below call a model and spend your tokens and your image quota. Step 7 says which.

## Steps

### 1. Add the logo

Performance Max and Demand Gen show your logo next to every ad. mads never draws one. When the agent chooses the formats, an App campaign needs no logo. If you list `app_installs` yourself in step 4, the file needs a logo too.

If you ran `mads init`, it saved a logo from your site to `brand/logo.png` and wrote the table for you. Open the file and check it is your real logo. If init found none, it prints this line.

Example run (replay provider, no model was called). The site was a local copy at `http://127.0.0.1:8765`.

```text
No logo found on the site: add [brand] logo to business.toml for image campaigns
```

Put your logo in the project and point `business.toml` at it.

```toml
[brand]
logo = "brand/logo.png"
```

The file must be PNG or JPEG, square, at least 144 pixels on the smallest side and at most 150 KB. A logo that breaks one of these stops `mads generate` with a `brand.logo` error that names the file. The exact checks are in the [`[brand]` reference](../reference/business-toml.md#brand).

### 2. Add the app

Skip this step if you do not advertise an app.

An App campaign promotes one app. Add `[app]` with the store and the id.

```toml
[app]
store = "google_play"
id = "com.example.app"
```

The id is the package name for Google Play, and the numeric id for the App Store. When the app is on both stores, write one `[[app]]` per store. The plan then creates one App campaign per store.

```toml
[[app]]
store = "google_play"
id = "com.example.app"

[[app]]
store = "app_store"
id = "6751919708"
```

`mads init` writes this for you when your site links to the app on a store. It keeps a store you wrote by hand when the site does not link that store. The keys `optimize_for` and `conversion_action` are in the [`[app]` reference](../reference/business-toml.md#app-and-app). To judge the app by in-app actions after launch, see [Optimize a live account](optimize-a-live-account.md).

### 3. Pick an image model

Set one key in the shell that runs `mads generate`.

```bash
export GEMINI_API_KEY=...
export OPENAI_API_KEY=...
```

The default `--image-provider auto` uses Gemini when `GEMINI_API_KEY` is set, then OpenAI when `OPENAI_API_KEY` is set. With neither, there is no image model and every image format is off. A key already in your shell turns on paid pictures by itself, so run `mads providers` to see what mads will use. Pick one with `--image-provider gemini` or `--image-provider openai`, or turn pictures off with `--image-provider none`. The table is in the [CLI reference](../reference/cli.md#mads-providers).

App campaigns need an image model too. With no model, the plan has no App campaign.

### 4. Choose the formats yourself, or let the agent choose

With a logo or an app, and an image model, the agent decides. By default it plans one image campaign with at least 20 percent of the daily budget, and an App campaign when the business has an app. It can skip either one only with a written reason, and the report shows that reason.

To decide yourself, list the formats.

```toml
[campaigns]
formats = ["search", "demand_gen", "app_installs"]
```

The plan must then hold at least one campaign of each listed format. Two rules apply when the file loads.

- Every format except `search` needs `[brand] logo`, `app_installs` included. A list with `app_installs` and no logo fails with `App - Installs needs [brand] logo`.
- `performance_max` needs `conversion_tracking = true`.

Demand Gen does not need tracking, so it can run before tracking exists. The full list of checks is in the [`[campaigns]` reference](../reference/business-toml.md#campaigns).

### 5. Set the brand look

Pictures must look like your brand and not like a stock photo. `DESIGN.md` carries the identity, and `business.toml` points to it.

```toml
[design]
file = "DESIGN.md"
```

`mads init` writes the file. The colors under `## Colors` come from the logo and from the theme color your site declares. The agent adds the style and what to avoid. Edit it freely. mads reads every hex code under `## Colors` and adds them to the prompt of every picture. Keep the file under 8000 characters. Without `[design]`, pictures get no brand colors.

### 6. Add real product photos

An image model asked to draw a specific product invents its label. To show the real product, give the catalog item a photo.

Fill the `image` column of `catalog.csv` with an absolute `http` or `https` URL. `mads init` fills it from the `og:image` of each item page. A picture brief can then name that item, and mads sends the real photo to the model with the prompt. A brief that names an item with no photo fails with `E19`. The column rules are in the [catalog reference](../reference/catalog-csv.md#image), and the how-to is [Write a catalog](write-a-catalog.md).

### 7. Generate

This step calls the planning model and the image model. It spends your tokens and your image quota.

```bash
mads generate business.toml --provider <provider> --max-images 12
```

`--max-images` caps the new pictures of one run. The default is 40. The cap is shared across asset groups, so one group with many briefs cannot leave another without any. A brief over the cap is dropped when its group has enough pictures without it. If the group cannot do without it, the export stops with `E20`. Run `mads generate --resume <run-dir>` with a higher cap to draw it. Each picture gets two attempts. Pictures already on disk are reused. See [Run limits](../reference/limits.md).

## Check it worked

Open `report.md` in the run directory.

- The `Format` column of the Summary table shows `Performance Max`, `Demand Gen` or `App - Installs` for the new campaigns.
- An `Images` section lists the model, how many pictures were generated and reused, and the file of every brief.
- The pictures are in `google-ads/editor/images/`, one subfolder per campaign, plus `logo.png`.

If every row says `Search`, the run had no image model, no logo, or no app. Run `mads providers`, then check the logo and `[app]` against steps 1 to 3. When the agent skipped images on purpose, a campaign rationale starts with `No image campaign:` or `No app campaign:` and gives the reason. Read the rationales under Budget and bids.

Pictures cannot travel in a CSV file. After the check, attach them in Google Ads Editor. The steps are in [Import into Google Ads Editor](import-into-google-ads-editor.md).

## Next

- [Import into Google Ads Editor](import-into-google-ads-editor.md)
- [Review a run](review-a-run.md)
- [Image and app campaigns](../explanation/image-campaigns.md)
