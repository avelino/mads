# Validation rules

Look up any code mads emits, what it checks and what the message says.

Errors block the export. Warnings go to `report.md`, to progress output and, on GitHub Actions, to annotations. Each issue has a `code`, a `path` into the account such as `campaigns[0].ad_groups[3].rsa.headlines[2]`, and a `message`. The checks live in the code, so the code wins over this page if they ever differ.

## Where rules run

| Stage | What happens |
|---|---|
| During a mission | Every tool validates its input before it changes state. A failing call returns the errors to the agent and changes nothing. |
| End of the run | mads validates the whole account again. Any error means exit code `3` and no CSV. See [exit codes and errors](exit-codes-and-errors.md). |
| URL check | `E15` runs after validation passes, and only without `--skip-url-check`. |
| `--layout drive-folders` | `E16`, `E18`, `E20` and `E24` on a campaign the package cannot build become `W12` and do not block. `E24` is also dropped when `google_ads.app_id` is set. The other layout keeps them as errors. |

## How text is measured

| What | Rule |
|---|---|
| Length | Unicode scalar values after NFC normalization, on the trimmed text. Google counts some CJK characters as 2. mads counts them as 1. |
| Comparison | Duplicates, avoid terms and negatives compare after `normalize`: NFC, lowercase, trim, inner whitespace collapsed to one space. |
| Focus terms | `E23` compares without accents or punctuation, so `sao paulo` matches `São Paulo`. |

## Errors

| Code | Rule | Details |
|---|---|---|
| `E01` | Text longer than its limit. | See [text limits](#text-limits). Message `<n> chars, limit is <limit>`. |
| `E02` | Required text is empty after trim. | Message `text is empty`. Same fields as `E01`. |
| `E03` | Duplicate text inside one list. | Brand kit headlines and descriptions, RSA headlines and descriptions, sitelink texts, callouts, snippet values, asset group headlines, long headlines, descriptions and search themes. Compared after `normalize`. Message `duplicate of [<index>]`. |
| `E04` | `!` in a headline. | Brand kit, RSA, Performance Max and App headlines. Demand Gen headlines and descriptions may hold it. Message `headlines cannot contain '!'`. |
| `E05` | Ad or asset text contains a `business.avoid` term. | Substring match after `normalize`. Checks headlines, descriptions, sitelink text and descriptions, callouts, snippet values and asset group texts. Not keywords or paths. Message `text contains avoided term '<term>'`. |
| `E06` | Budgets. | Campaign budgets do not sum to `budget.daily`, a campaign budget is under 1.00, or a planned budget is not above 0 with at most 2 decimals. |
| `E07` | URL outside the allowed set. | The set is `business.url`, `business.pages`, the catalog URLs and every `[focus]` URL. Applies to ad group, asset group and sitelink URLs. Messages `URL not in business.url, business.pages or catalog: <url>` and, in the plan, `URL not allowed: <url>`. In init it means a page, catalog item, focus URL, product or image the agent never fetched: `URL was not fetched or listed in the sitemap: <url>` or `image was not shown by a fetched page: <url>`. |
| `E08` | Bad keyword syntax. | More than 10 words, or one of ``! @ % ^ * ( ) { } ; ~ < > ? \ , [ ] "`` plus the backtick and the pipe. `=` is allowed. Applies to keywords and negatives. Messages `<n> words, limit is 10` and `character '<c>' is not allowed in keywords`. |
| `E09` | A negative blocks a keyword in its scope. | A phrase negative blocks a keyword whose words contain the negative's words in a row. An exact negative blocks a keyword with equal text. Ad group negatives apply to their ad group. Campaign negatives apply to every ad group of the campaign. Message `negative '<n>' blocks keyword '<k>'`. |
| `E10` | `maximize_conversions` while `business.conversion_tracking` is `false`. | Reached by Performance Max and Demand Gen. App campaigns are exempt. Message `maximize_conversions needs business.conversion_tracking = true`. |
| `E11` | CPC problems. | A CPC of 0, an ad group CPC that is not above 0 with at most 2 decimals, or a CPC above `budget.max_cpc` when it is set. Also checks the cap of `maximize_clicks`. |
| `E12` | Structure. | Duplicate campaign or ad group name. Two names with the same slug. A name with no letters or digits. An ad group or asset group not in the plan. A planned group missing at the end of the run. An ad group without keywords. A campaign with no planned ad groups. Unknown catalog id in `entity_ids`. Brand kit, account plan or campaign assets not set. In init, `finish` before `write_business` or `write_research`, and strategy products given when the strategy already sets them. |
| `E13` | Count out of range. | See [count limits](#count-limits). Message `<what>: <n> found, expected <min> to <max>`. Three messages have other shapes: `<planned> planned ad groups, limit is <max-ad-groups>`, `<n> found, expected <min> to <max>` for the keyword spec counts (variants, modifiers, extra; the path names the count), and `one description must have 60 characters or fewer` for Performance Max. In init it also covers `write 1 to <top> products, got <n>` and `<n> items would exceed the limit of <limit>`. |
| `E14` | `path2` without `path1`, or a sitelink with exactly one description. | Messages `path2 requires path1` and `a sitelink needs both descriptions or none`. |
| `E15` | URL check failed. | A final or sitelink URL did not answer 2xx after redirects. Message `URL check failed: HTTP <status>` or `URL check failed: unreachable`. The path is the URL. |
| `E16` | Image briefs of an asset group. | Performance Max needs 1 landscape and 1 square. Demand Gen needs 1 landscape or square. App needs 1 image. Performance Max and App take no vertical (9:16). An `id` must be a slug and unique in the group. A prompt has 20 to 1500 characters. At the end of the run every asset group needs briefs. |
| `E17` | The campaign does not fit its `kind`. | Search takes `manual_cpc`. Performance Max and App take `maximize_conversions`. Demand Gen takes `maximize_clicks` or `maximize_conversions`. An image or App campaign with ad groups, negatives or assets, or a Search campaign with asset groups. Messages `<strategy> does not fit a <kind> campaign` and `a <kind> campaign has no <part>`. |
| `E18` | Performance Max or Demand Gen campaign without a logo. | Message `image campaigns need a logo: set [brand] logo in business.toml`. |
| `E19` | `reference` names an unknown catalog item or one without `image`. | Messages `unknown catalog id '<id>'` and `catalog item '<id>' has no image to use as reference`. |
| `E20` | A brief has no picture at the end. | The image step could not make it, or `mads export` ran without the file. A brief over `--max-images` that its group can do without is removed, with a note in the report. Message `image '<id>' has no file`. Agents never see it, because pictures come after the missions. |
| `E21` | A format from `[campaigns] formats` has no campaign. | Message `business.toml asks for a <kind> campaign and the plan has none`. Path `campaigns`. |
| `E22` | A landing page outside `[focus]`. | Planned group, ad group and asset group final URLs must be one of `focus.urls`. Sitelinks may go elsewhere. Message `final URL is not a [focus] page: <url>`. In init it also covers a catalog item or strategy product outside the focus and `--focus` without a focus that holds the start URL. |
| `E23` | A keyword or search theme without the focus words. | With `focus.terms`, every keyword and search theme needs one term of each group. Message `'<text>' is not about the focus: add a word like <terms>`. In init it covers `--focus` without `focus.terms`. |
| `E24` | App campaign without its app. | Message `app campaigns need the app: set [app] store and id in business.toml`. With an app on each store and no `app_store` on the campaign, `the business has an app for each store: name the store of this campaign with app_store`. With no app for the store, `no app for the store <store>: add its [[app]] to business.toml`. App campaigns need no logo and link to the store page, so `E07`, `E18` and `E22` do not apply to them. |
| `E25` | A group that had impressions is missing from a campaign that continues. | Only in `mads optimize`. Editor matches by name, so a renamed group comes in as a new one, and Google measures [Quality Score per keyword from past impressions](https://support.google.com/google-ads/answer/6167118?hl=en). A line `Drop <group>: <reason>` in the campaign `rationale` lets it go. Path `campaigns[<i>].ad_groups`. |
| `E26` | Keyword spec `variants` look like different searches. | Checked by `upsert_ad_group`. Two variants belong together when, without accents or punctuation, they share a word of 3 or more letters, one holds the other (4 or more letters), they start with the same 4 letters, or one is the initials of the other. Message `variants look like <n> different searches: keep the spellings of one search in variants, put a synonym in extra, and give another search its own ad group`. |
| `E27` | A stalled, untracked or over-target campaign gets more daily budget than it ran with. | Only in `mads optimize`. Stalled means enabled with no ads, or 7 days without one impression. Untracked means 100 clicks or more and no conversion while another campaign converts, capped at the average daily cost rounded up when that is lower than the budget. Over target means each conversion cost more than `budget.target_cpa` (`target_cpi` for App, `target_cpa_in_app` for App with app events), or the campaign spent twice the target with no conversion. Without a target there is no over-target check. Cutting is allowed. Path `campaigns[<i>].daily_budget`. |
| `E28` | An ad group of an untracked campaign bids higher than it ran. | Only in `mads optimize`, checked by `upsert_ad_group`. Path `default_cpc`. Message `this campaign is untracked: ... Keep default_cpc at <cpc> or lower`. |
| `E29` | The automation package is over capacity. | Only with `--layout drive-folders`, at export. The automated block (Search campaigns with intent `catalog`) holds `max(150, ceil(1.5 * catalog rows))` feed rows. The limits are 1000 feed rows and 2000 rows in the `Negatives` tab. Blocks the export and writes no file. Messages `automation package over capacity: <n> catalog rows need <m> feed rows, over the limit of 1000` and `automation package over capacity: <n> negative keyword rows, over the limit of 2000`. Path empty. |
| `E30` | Two App campaigns for one store. | Message `a second App campaign for the store <store>: one campaign per store`. Path `campaigns[i].app_store`. A business with one app counts its only store, so campaigns with no `app_store` collide too. |
| `E31` | A live App campaign planned for another store. | Only in `mads optimize`. Google Ads does not move an App campaign to another app. Message `'<name>' runs on <store> in Google Ads and a campaign cannot change its app: keep it on <store>, and plan <other> as a new campaign with another name`. |
| `E32` | A strategy product with no catalog item. | Only in init with a strategy. `finish` compares pages without a leading `www.` and keeping the query string. Messages `strategy product has no catalog item: <url>` and `strategy has no products: write strategy.products in write_business`. Path `strategy.products`. |
| `E33` | Product campaigns get less than `min_share` percent of the non-brand budget. | Only with a strategy. The base is the daily budget of campaigns that are not Brand and not App. A product campaign has planned ad groups and every one is a product group, by `entity_ids` that all belong to the strategy's products, or, with empty `entity_ids`, by a landing page that is one of the products. The landing page is the group's `final_url`, else the first `[focus]` URL, else `business.url`. Fails when `product * 100 < min_share * base`, or when the base is 0 and `min_share` is above 0. Message `product campaigns get <share>% of the non-brand budget, the strategy asks for at least <min_share>%: move budget to campaigns that only hold <ids>`, with `<share>` rounded down. Path `campaigns`. |

### Text limits

| Text | Limit |
|---|---|
| Headline (RSA, brand kit, Performance Max, App) | 30 |
| Demand Gen headline | 40 |
| Performance Max long headline | 90 |
| Description | 90 |
| Performance Max description, at least one | 60 or fewer (`E13`) |
| Path 1 and path 2 | 15 |
| Sitelink text | 25 |
| Sitelink description | 35 |
| Callout | 25 |
| Snippet value | 25 |
| Asset group `business_name` | 25 |
| Search theme | 80 |
| Keyword and negative | 80 |
| Campaign, ad group and asset group name | 255 |

### Count limits

| What | Allowed |
|---|---|
| Campaigns in the account | 1 to 5 |
| Planned ad groups in the account | up to `--max-ad-groups` (default 50) |
| Brand kit headlines | 8 to 12 |
| Brand kit descriptions | 2 to 3 |
| Specific RSA headlines | 3 to 7 |
| Specific RSA descriptions | 1 to 2 |
| Keywords per ad group | 1 to 50 |
| Keyword spec `variants` | 1 to 6 |
| Keyword spec `modifiers` | 0 to 10 |
| Keyword spec `extra` | 0 to 20 |
| Negatives per scope (campaign or ad group) | 0 to 100 |
| Sitelinks | 2 to 8 |
| Callouts | 2 to 10 |
| Structured snippets | 0 to 2 |
| Values per snippet | 3 to 10 |
| Performance Max headlines | 3 to 15 |
| Performance Max long headlines | 1 to 5 |
| Performance Max descriptions | 2 to 5 |
| Performance Max search themes | 0 to 25 |
| Demand Gen headlines | 1 to 5 |
| Demand Gen descriptions | 1 to 5 |
| App headlines | 1 to 5 |
| App descriptions | 1 to 5 |
| Demand Gen and App long headlines, Demand Gen and App search themes | 0 |
| Pictures per asset group | 0 to 20 |

## Warnings

| Code | Rule | Details |
|---|---|---|
| `W01` | Ad or asset text contains a third-party term. | The terms are every `business.competitors` entry and the name and aliases of every catalog item with `third_party = true`. The match is on whole words, in order, after `normalize`. Message `third-party term '<term>' in ad text (trademark policy risk)`. |
| `W02` | A word of 4 or more letters fully in uppercase that is not part of a brand term. | Words split on any non-letter. Message `word in all caps`. |
| `W03` | The same keyword in two ad groups of one campaign: same normalized text, both Exact or both not Exact. mads writes only Phrase and Exact, so in practice it is the same text and match type. | Message `keyword '<k>' is also in ad_groups[<index>]`. |
| `W04` | Merged RSA with fewer than 15 headlines or fewer than 4 descriptions. | Message `merged ad has <h> headlines and <d> descriptions (15 and 4 recommended)`. |
| `W05` | Fewer than 4 sitelinks, fewer than 4 callouts or no structured snippet. | Message `recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet`. |
| `W06` | Fewer pictures than Google recommends for ad strength. | Performance Max 4 landscape, 4 square and 2 portrait. Demand Gen and App 1 of each. Message `recommended for ad strength: <missing>`. |
| `W07` | A prompt asks for text in the picture, or shows an object that carries writing. | Text words: `text`, `logo`, `caption`, `headline`, `words`, `lettering`, `typography`, `slogan`, `label that reads`. Writing objects: `menu`, `wine list`, `sign`, `signboard`, `billboard`, `poster`, `book`, `newspaper`, `magazine`, `screen`, `monitor`, `packaging`, `price tag`, `ticket`. Messages `prompt mentions '<word>': Google adds the text, keep the picture clean` and `prompt shows a <object>: the model writes invented text on it, ...`. |
| `W09` | A one-word phrase keyword in a campaign whose intent is not Brand. | It matches any search with that word. Message `'<keyword>' as phrase matches any search with that word: make it exact or add words`. |
| `W10` | An ad group that ran is rebuilt without some of its keywords, and the keywords report is missing. | Only in `mads optimize`. The tool puts the dropped keywords back. Path `keywords`. Message `kept <n> keywords that ran, no keywords report to judge them: <examples>`. |
| `W11` | `--layout drive-folders` and `google_ads.customer_id` is empty or `000-000-0000`. | Does not block. The Customer ID setting is left blank. Path `google_ads.customer_id`. |
| `W12` | `--layout drive-folders` and an image or App campaign lacks what its type needs. | Missing means no picture files, no logo (Performance Max and Demand Gen) or no app id (App, from `google_ads.app_id` or `[app]`). Does not block. The campaign is left out of the `Image Campaigns` and `Image Assets` tabs. Path `campaigns[<index>]`. Message `<type> campaign '<name>' is left out of the automation package: <missing>.` |
| `W13` | `--layout drive-folders` and a catalog item name of an automated campaign is over 30 characters. | Does not block. `headline_1` of that `Catalog` row stays empty, so headline 1 of its ad falls back to the template headline in `Ad Templates`. One warning per row. Path `catalog`. |
| `W14` | The `E33` share rule fails in `mads optimize`. | Does not block. Same message as `E33`. Path `campaigns`. |

`W08` is not emitted.

## Tool-only codes

These come from the tool layer. The agent sees them in tool results.

| Code | Meaning |
|---|---|
| `ARGS` | The arguments did not parse. Unknown field, missing field or wrong type. The message is the parser error. |
| `UNKNOWN_TOOL` | The tool does not exist or is not in this mission's toolset. |
| `LIMIT` | The mission used its tool call budget, `--max-turns` times 4. |
| `UNSUPPORTED` | A Search campaign with a bid strategy other than `manual_cpc`, or an image or App campaign the run cannot plan. The reason is one of `no image model in this run`, `no app` or `no logo`. |
| `NO_IMAGE_REASON` | An image kind is available, the plan has no image campaign, and no `rationale` contains `No image campaign: <reason>`. Not asked when `[campaigns] formats` is set. |
| `NO_APP_REASON` | An App campaign is available, the plan has none, and no `rationale` contains `No app campaign: <reason>`. Not asked when `[campaigns] formats` is set. |
| `LIVE_DETAIL` | In `mads optimize`, `upsert_ad_group` or `upsert_asset_group` for a group that ran, before this mission called `get_ad_group_performance` with it. |
| `NOT_FOUND` | The campaign of this mission no longer exists in the plan, or has no live data. |
| `PERSIST` | The workspace could not be saved to disk. |
| `HOST`, `FETCH` | Init only. The URL is outside the site, or the request failed. |
| `QUERY` | Init only, in the site search tool. `names` has 0 or more than the allowed names, or a name has no letters or digits. Paths `names` and `names[<i>]`. |
| `INPUT` | Init only. The draft does not parse like `business.toml` or `catalog.csv`, or a research field is out of range. |
| `EXPORT` | The final export failed, for example when the brand kit or the logo is missing. |

## Cross negatives

Before validation, the end step adds `business.brand_terms` as phrase campaign negatives to every campaign except Brand, and `business.competitors` to every campaign except Competitor. Image and App campaigns take no negatives and are skipped. A term that would trigger `E09` in a campaign is skipped and listed as a note in the Validation section of `report.md`.

## Next

- [Exit codes and errors](exit-codes-and-errors.md)
- [business.toml](business-toml.md)
- [Run limits](limits.md)
