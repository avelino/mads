# Image and app campaigns

Understand when a run gets Performance Max, Demand Gen or App campaigns, and how mads keeps generated pictures on brand and honest about your product.

## Four formats, one choice per campaign

The plan agent picks a format for each campaign. Search stays the default. An image format is an option only when the run has what that format needs.

| `kind` | Where the ads show | Bidding | When the agent picks it |
|---|---|---|---|
| `search` | Google search results, text only | `manual_cpc` | People already search for what you sell. |
| `demand_gen` | YouTube, Discover and Gmail feeds | `maximize_clicks`, or `maximize_conversions` with tracking | Demand has to be created and the product shows well in a picture. |
| `performance_max` | Search, YouTube, Display, Discover, Gmail and Maps | `maximize_conversions` | `conversion_tracking = true` and the budget lets Google learn. |
| `app_installs` | Google Play or App Store, YouTube, Search and Display | `maximize_conversions` | The goal is installs and the business has an app. |

The bid strategy follows the format. A campaign with a strategy that does not fit its kind is refused with `E17`. The agent also gives each format a job: Search captures demand that exists, Demand Gen creates new demand, Performance Max scales what converts. It never plans two campaigns that chase the same people with the same message.

Performance Max and Demand Gen need a logo and an image model. An App campaign needs an image model and an `[app]`. The planning tool does not ask for a logo, because Google shows the app's name and icon from the store, but listing `app_installs` in `[campaigns] formats` still makes `business.toml` fail to load without `[brand] logo`. When a format is missing what it needs, the agent is told why and plans the others. For the exact keys, see the [business.toml reference](../reference/business-toml.md). To set it up, see [Add image and app campaigns](../how-to/add-image-campaigns.md).

## The agent writes briefs, code draws

For an image campaign the agent writes the texts of each asset group and one brief per picture. It never sees a picture. After the missions end, code sends the briefs to the image model, crops the results to Google's sizes and writes the whole account, Search included, into one Google Ads Editor file.

That split keeps the agent cheap and the pictures checkable. Every limit on texts and pictures is enforced by the tools while the agent works and again before export. A missing picture stops the export with `E20` instead of shipping an empty ad. The codes are in [Validation rules](../reference/validation-rules.md).

## When the agent picks images

With an image model and a logo, the default is one image campaign with at least 20 percent of the daily budget, reserved before the Search campaigns split the rest. The agent may skip it only with a reason. Then the rationale of its largest campaign starts with `No image campaign:`, and the report shows the reason under Budget and bids. A plan with neither an image campaign nor that reason is refused with `NO_IMAGE_REASON`. The point is to make the choice visible, not to force the format.

With an app, the default is an `app_installs` campaign. It optimizes for installs, and a Demand Gen or Search ad that says "download the app" does not. The agent may skip it only with `No app campaign: <reason>` in a rationale, such as a goal that is sales on the site. Otherwise the plan is refused with `NO_APP_REASON`.

When you list formats yourself in `[campaigns] formats`, neither reason is asked. Your list decides, and the plan must hold at least one campaign of each listed kind. The agent still picks the budget share and the groups. `mads init` and `mads generate` warn when `formats` leaves out `app_installs` while an app is set.

Performance Max also needs `conversion_tracking = true`. Demand Gen does not, which is why it can run before tracking exists.

The agent plans 1 to 3 asset groups per image campaign, because each group needs its own pictures and every picture is a paid call.

## One app, two stores

An App campaign promotes one app. When the app is on both stores, the plan creates one App campaign per store, each with its own store page and budget. A second campaign for the same store is refused with `E30`.

The agent splits the app budget by what the numbers say:

- With reports for both stores, the cost per install decides. The cheaper store gets more, and the other stays large enough to keep learning.
- A store with no history gets a test share. It is enough to collect installs in a few days, at most 30 percent of the app budget, and never more than the store with history until its cost per install is known.
- With no history on either store, the split is even, unless the research names the platform of the audience. Then it leans toward that platform by at most 60 percent.

The two stores count installs differently, so the agent treats the comparison as a rough guide and not as proof. See [How optimize decides](how-optimize-decides.md) for how a live account changes this, including judging by in-app actions instead of installs.

In an optimize run, an App campaign that already runs keeps its store, because Google Ads cannot change the app of an existing campaign (`E31`).

## Pictures that look like your brand

A model asked for "a bottle of wine" draws a stock photo of the category. `DESIGN.md` exists to prevent that, and three things work together.

- The colors come from code, not from the model. `mads init` reads the dominant colors of the downloaded logo and the `theme-color` the site declares, and writes them under `## Colors`. The agent adds style, imagery, voice and what to avoid from what it read.
- The image campaign agent reads the whole file. It is told to put a brand color on the main object of most scenes, to follow the style and imagery for mood and light, and never to show what the file says to avoid.
- mads adds a `Brand colors:` line to every prompt itself, in case a brief forgets them. The line goes after the brief. The reference note comes next when the brief has a photo, and a no-text clause always closes the prompt.

The no-text clause exists because Google overlays the ad text and the logo itself. A picture with letters in it would collide with them.

Without a `[design]` section, pictures get no brand colors. Colors are a hint for the object, not a logo: the agent never asks for a logo or a name on the product.

## Real photos beat invented products

An image model asked to draw a specific product invents its label, package or screen. A wine that does not exist in a picture is worse than no picture.

mads avoids it with a rule the agent follows. When a picture shows a specific catalog item, the brief points at that item with `reference`, and the item's real photo goes to the model along with the prompt. The model places the real product in the scene the brief describes, so the brief says where the product sits and not what it looks like. Only items that have a photo can be referenced, and a `reference` to one without a photo fails with `E19`.

A brief without `reference` shows people, places and results, not the product. `mads init` fills each item's photo from the `og:image` of its page, so a catalog built from a site usually has them. The column is described in the [catalog reference](../reference/catalog-csv.md).

## Pictures cost money, so they are kept

Every picture is a paid API call, which shapes three behaviors. A run has a cap on new pictures, shared across asset groups so one group with many briefs cannot leave another with none. A picture that passes the checks is reused by `mads generate --resume` and by `mads export`, and only what is missing gets drawn. Each picture gets two attempts, and a failure, a refused prompt or a reference photo that cannot be downloaded fails only that picture. See [Limits and retries](limits-and-retries.md) and the [CLI reference](../reference/cli.md) for the flag.

## Why pictures are attached by hand

Pictures cannot travel in a file. The web bulk upload takes no image files, and Google Ads Editor neither exports nor imports the link between an ad and its images. So mads writes the texts and the structure, and you attach the pictures. The only way around it is the automation package, whose Google Ads script creates image campaigns through the Google Ads API. See [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md) and [Use the automation package](../how-to/use-the-automation-package.md).

Image campaigns take no negatives, sitelinks, callouts or snippets in this version, and the cross negatives step skips them.

## Next

- [Add image and app campaigns](../how-to/add-image-campaigns.md)
- [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md)
- [Campaign strategies](strategies.md)
