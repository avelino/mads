# MCP tools

Look up every tool an agent can call in a mads mission, with its arguments, result and the codes it returns.

The plan, campaign and image campaign tools live in `crates/mads-core/src/tools`. The init tools live in `crates/mads-core/src/init/tools.rs`. API providers call them through the model's tool calling. Agent CLI providers reach the same definitions through a per-mission MCP server. For how a run uses them, see [How a run works](../explanation/how-a-run-works.md) and [Agent CLIs](../explanation/agent-clis.md).

## Missions and toolsets

A mission sees only its own tools. Any other name returns `UNKNOWN_TOOL`.

| Mission | Tools |
|---|---|
| Plan | `get_business`, `query_catalog`, `set_brand_kit`, `set_account_plan`, `finish` |
| Campaign (kind `search`) | `get_brief`, `upsert_ad_group`, `set_campaign_negatives`, `set_assets`, `validate`, `finish` |
| Campaign (kind `performance_max`, `demand_gen` or `app_installs`) | `get_brief`, `upsert_asset_group`, `set_image_briefs`, `validate`, `finish` |
| Both campaign kinds in `mads optimize` | the list above plus `get_ad_group_performance`, second in the list |
| Init | `fetch_page`, `fetch_sitemap`, `search_site`, `write_business`, `write_research`, `write_design`, `add_catalog_items`, `finish` |

A campaign mission reads and changes only its own campaign.

## Result format

Success.

```json
{"ok": true, "result": {}, "warnings": []}
```

Failure. The call changed nothing.

```json
{"ok": false, "errors": [{"code": "E01", "severity": "error", "path": "rsa.headlines[0]", "message": "34 chars, limit is 30"}], "warnings": []}
```

Every tool takes `additionalProperties: false`. An unknown field or a bad type is an `ARGS` error with the parser message. Schemas have no `$ref`, `anyOf`, `oneOf` or nullable types.

## Codes that belong to the tools

The `E` and `W` codes are in [Validation rules](validation-rules.md). These codes come from the tool layer.

| Code | Returned when |
|---|---|
| `ARGS` | The arguments do not parse. The message is the parser error. |
| `UNKNOWN_TOOL` | The tool is not in the mission's toolset. |
| `LIMIT` | The mission made more than `--max-turns` times 4 calls, or `fetch_page` hit the page limit. |
| `NOT_FOUND` | The campaign is gone from the plan, `get_ad_group_performance` ran on a campaign with no live data, or it got a group neither the run nor the reports know. |
| `LIVE_DETAIL` | In `mads optimize`, `upsert_ad_group` or `upsert_asset_group` got a group that ran before the mission read it with `get_ad_group_performance`. |
| `UNSUPPORTED` | `set_account_plan` got a Search campaign with a bid strategy other than `manual_cpc`, or an image or App campaign the run cannot build. |
| `NO_IMAGE_REASON` | Image campaigns are available and the plan has none and no rationale contains `No image campaign:`. |
| `NO_APP_REASON` | The app is available and the plan has no `app_installs` campaign and no rationale contains `No app campaign:`. |
| `PERSIST` | The workspace could not be saved to disk. |
| `HOST` | An init URL is not on the business website. |
| `FETCH` | An init page or sitemap could not be read. |
| `QUERY` | `search_site` got a bad list of names. |
| `INPUT` | An init draft failed the same checks as `business.toml` and `catalog.csv`, or a research field is out of range. |

`NO_IMAGE_REASON` and `NO_APP_REASON` are not asked when `business.toml` sets `[campaigns] formats`.

## Plan mission

### get_business

No arguments. Returns the business, the budget and the rules every tool enforces.

| Key | Content |
|---|---|
| `business` | The business profile. |
| `budget` | `daily`, `currency`, `max_cpc`, `target_cpa`, `target_cpi`. `target_cpa_in_app` only when set. |
| `catalog_size` | Number of catalog items. |
| `catalog_photos` | Items that have an `image`. |
| `image_campaigns` | `available`, and `reason` when `false`. Covers Performance Max and Demand Gen. |
| `app_campaigns` | `available`, and `reason` when `false`. |
| `apps` | One entry per app, with `store`, `id` and `store_url`. Empty without an app. |
| `focus` | The `[focus]` table, or `null`. |
| `rules` | See below. |
| `strategy` | Only with a campaign strategy. `name`, `intent`, `min_share`, `product_ids`, `prose`. |
| `research` | Only when `business.toml` has research notes. The Markdown text. |
| `required_formats` | Only when `[campaigns] formats` is set. |
| `live_account`, `performance` | Only in `mads optimize`. See below. |

The `reason` of `image_campaigns` and `app_campaigns` is one of these.

| Reason | Applies to |
|---|---|
| `no image model in this run (set --image-provider and its API key)` | Both. |
| `no logo: set [brand] logo in business.toml` | `image_campaigns`. |
| `no app: set [app] store and id in business.toml` | `app_campaigns`. |

`rules` holds these keys.

| Key | Value |
|---|---|
| `headline_max_chars` | 30 |
| `description_max_chars` | 90 |
| `path_max_chars` | 15 |
| `sitelink_text_max_chars` | 25 |
| `sitelink_description_max_chars` | 35 |
| `callout_max_chars` | 25 |
| `snippet_value_max_chars` | 25 |
| `headline_exclamation_allowed` | `false` |
| `brand_kit` | Headlines 8 to 12, descriptions 2 to 3 |
| `rsa_specific` | Headlines 3 to 7, descriptions 1 to 2 |
| `keywords_per_ad_group` | 1 to 50 |
| `keyword_spec` | `variants` 1 to 6, `modifiers` 0 to 10, `extra` 0 to 20 |
| `negatives_per_scope` | 0 to 100 |
| `campaigns` | 1 to 5 |
| `sitelinks`, `callouts`, `snippets` | `2 to 8 (4 or more recommended)`, `2 to 10 (4 or more recommended)`, `0 to 2, 3 to 10 values each` |
| `snippet_headers` | The allowed header names |
| `intents` | `brand`, `catalog`, `generic`, `competitor` |
| `match_types` | `phrase`, `exact` |
| `campaign_kinds` | The bid strategies of each kind, and what a kind needs |
| `max_ad_groups` | The `--max-ad-groups` value |

`campaign_kinds` is this.

| Kind | Bid strategies | Needs |
|---|---|---|
| `search` | `manual_cpc` | none |
| `performance_max` | `maximize_conversions` | `conversion_tracking` |
| `demand_gen` | `maximize_clicks`, `maximize_conversions` | none |
| `app_installs` | `maximize_conversions` | `[app]` in `business.toml` |

In a `mads optimize` run, `live_account.campaigns` lists each campaign as it ran with `name`, `kind`, `intent`, `daily_budget`, `bid_strategy` and `ad_groups` (names). `performance` has these keys.

| Key | Content |
|---|---|
| `window` | The report window. |
| `thin_rule` | A sentence. Defines `thin`. |
| `live_rules` | A sentence per rule the reports triggered. |
| `idle_budget` | Daily budget that goes unspent. |
| `missing_reports` | Report kinds that were not read. |
| `ignored_campaigns` | Campaigns the run skipped. |
| `campaigns` | One entry per campaign, below. |
| `outside_campaigns` | Only when enabled campaigns outside the run spend without converting. `name`, `cost_share_pct`, `clicks`, `conversions`, `paused_by_the_import`. |
| `drift` | Only when the live account differs from what the last run exported. |

Each entry of `performance.campaigns` has the campaign totals without ad group detail.

| Key | Content |
|---|---|
| `name`, `live_status`, `status_reasons`, `budget`, `bid_strategy` | Live state. |
| `metrics` | Totals. |
| `lost_to_budget_pct`, `lost_to_rank_pct` | Impression share lost. |
| `days_running`, `avg_daily_cost`, `budget_use_pct` | Spend. |
| `thin`, `stalled`, `untracked` | Flags. |
| `untracked_reason` | An App campaign with no conversion action for its store. |
| `hidden_terms_cost` | Cost the search terms report does not list. |
| `conversion_actions` | At most five, when the conversion action report was read. |
| `off_goal_actions` | When the conversion action report was read. |
| `networks` | Cost share, conversion share and CTR per network, when the network report was read. |
| `accidental_display_clicks` | Only when `true`. |
| `app` | With app events. `store`, `app_installs_seen`, `in_app_actions`, `install_to_action_pct`, `cost_per_in_app_action`, `store_vs_app_install_ratio`, `store_cost`. |
| `vs_target` | `within`, `over` or `no_result`. Only with a target and enough numbers. |
| `budget_cap` | Only when `E27` limits the campaign. The most daily budget it may get. |

`performance.campaigns[].vs_target` uses `target_cpi` for App campaigns and `target_cpa` for the others. With app events and `target_cpa_in_app`, an App campaign is judged by in-app actions. See [How optimize decides](../explanation/how-optimize-decides.md).

### query_catalog

| Argument | Type | Default | Description |
|---|---|---|---|
| `category` | string | `""` | Only items of this category. Exact, case-insensitive. Empty for all. |
| `contains` | string | `""` | Case-insensitive text searched in id, name and aliases. |
| `offset` | integer | `0` | Items to skip. |
| `limit` | integer | `50` | Page size, clamped to 1 to 200. |

```json
{"total": 4, "offset": 0, "items": [], "categories": [{"name": "malbec argentino", "count": 2}]}
```

`items` are catalog rows as stored: `id`, `name`, `url`, `category`, `aliases`, `third_party`, `notes` and `image`. `total` counts the filtered items. `categories` counts every non-empty category of the whole catalog. See [catalog.csv](catalog-csv.md).

### set_brand_kit

| Argument | Type | Required | Description |
|---|---|---|---|
| `headlines` | string array | yes | 8 to 12, at most 30 characters. |
| `descriptions` | string array | yes | 2 to 3, at most 90 characters. |

Replaces the previous kit. Result `{"headlines": 10, "descriptions": 3}`.

Codes: `E01`, `E02`, `E03`, `E04`, `E05`, `E13`, `W01`, `W02`.

### set_account_plan

| Argument | Type | Required | Description |
|---|---|---|---|
| `campaigns` | array | yes | 1 to 5 campaigns. Replaces the previous plan. |

Each campaign.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Campaign name. Its slug is the mission id suffix. |
| `kind` | string | no | `search` (default), `performance_max`, `demand_gen` or `app_installs`. |
| `intent` | string | yes | `brand`, `catalog`, `generic` or `competitor`. |
| `daily_budget` | number | yes | Currency units, at most 2 decimals. All campaigns sum to `budget.daily`. |
| `bid_strategy` | object | yes | `{"type": "manual_cpc"}`, `{"type": "maximize_clicks"}` or `{"type": "maximize_conversions"}`. |
| `rationale` | string | yes | Why this budget share and bidding. |
| `app_store` | string | no | `app_installs` only. `google_play`, `app_store` or `auto` (default). |
| `ad_groups` | array | yes | Planned ad groups of a Search campaign, planned asset groups of an image or App campaign. |

Each ad group.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Unique in the campaign. |
| `theme` | string | yes | What the group is about. |
| `entity_ids` | string array | no | Catalog ids the group covers. An unknown id is `E12`. |
| `final_url` | string | no | Landing page. Empty resolves to the entity URL (one entity), else the first `[focus]` URL, else `business.url`. An App campaign always lands on its store page. |

`auto` takes the store of the only app. With an app on each store, each App campaign names its store (`E24`).

Result.

```json
{"campaigns": [{"name": "Vinellu - Marca", "slug": "vinellu-marca", "ad_groups": 1}]}
```

Codes: `E01`, `E06`, `E07`, `E10`, `E12`, `E13`, `E17`, `E18`, `E21`, `E22`, `E24`, `E25`, `E27`, `E30`, `E31`, `E33`, `UNSUPPORTED`, `NO_IMAGE_REASON`, `NO_APP_REASON`. Warning `W14`. `E25`, `E27` and `E31` apply only in `mads optimize`. With a strategy, `E33` becomes `W14` in `mads optimize`, and the plan is stored.

### finish

No arguments. Fails with `E12` when the brand kit or the plan is missing. Result `{"campaigns": 2}`.

## Search campaign mission

### get_brief

No arguments.

| Key | Content |
|---|---|
| `campaign` | `name`, `slug`, `intent`, `daily_budget`, `bid_strategy`, `rationale`, `planned_ad_groups`. |
| `brand_kit`, `business`, `focus` | As in `get_business`. |
| `budget` | `currency`, `max_cpc`. |
| `entities` | Full catalog rows of the entities in this campaign's planned groups. |
| `built_ad_groups` | `name`, `keywords`, `negatives`, `default_cpc` of each group already saved. |
| `campaign_negatives` | The campaign level negatives. |
| `assets_set` | `true` once `set_assets` succeeded. |
| `rules` | As in `get_business`. |
| `strategy` | Only with a campaign strategy. `intent`, `product_campaign`, `prose`. |
| `live`, `performance`, `live_detail` | Only in `mads optimize`. See below. |

In `mads optimize`, `live` is the campaign as it ran (`daily_budget`, `negatives`, `ad_groups` with `name`, `default_cpc`, `keywords` and `negatives` as counts, and `asset_groups` as names). It is `null` for a campaign that did not run. `performance` is the campaign entry described under `get_business`, with `ad_groups` replaced by one summary per group, plus `window`, `thin_rule` and `live_rules`. `live_detail` is a hint to call `get_ad_group_performance`.

A summary has `name`, `metrics`, `keywords`, `keywords_with_traffic`, `signals` (counts of `below_first_page`, `rarely_shown`, `low_quality`), `search_terms_total`, `cost_without_conversions`, and `not_in_run: true` for a group that serves in Google Ads from an older import.

### get_ad_group_performance

Only in `mads optimize`. Read only. Works for Search, Performance Max, Demand Gen and App campaign missions.

| Argument | Type | Required | Description |
|---|---|---|---|
| `ad_group` | string | yes | Name of an ad group or asset group of this campaign as it ran. Compared after `normalize`. |

```json
{"ad_group": "", "live": {"name": "", "default_cpc": 0.0, "final_url": "", "keywords": ["\"phrase\"", "[exact]"], "negatives": [], "rsa": {}}, "performance": {"keywords": [], "keywords_without_traffic": {"count": 0, "below_first_page": 0, "rarely_shown": 0, "low_quality": 0, "examples": []}, "search_terms": [], "search_terms_total": 0, "cost_without_conversions": 0.0}}
```

| Key | Content |
|---|---|
| `live` | The group as it ran. For an asset group, the asset group itself. `null` when only the reports know the group. |
| `performance.keywords` | Only keywords with impressions. `text`, `match_type`, `metrics`, and when present `signals`, `max_cpc`, `quality_score`, `first_page_bid`, `top_of_page_bid`. |
| `performance.keywords_without_traffic` | `count`, the three signal counts, and up to 5 `examples`. |
| `performance.search_terms` | The 20 most expensive terms. `text`, `match_type`, `state`, `metrics`. |
| `performance.search_terms_total` | Terms the group has before the cut. |
| `performance.cost_without_conversions` | Cost of terms with no conversion. |
| `performance.assets` | Labels Google gave the texts. Only when an assets report was read. |
| `performance.metrics` | Group totals. Only when an ad groups report was read. |

Metrics with a zero or missing value are left out. A name that neither the run nor the reports know fails with `NOT_FOUND`, and the message lists the groups that ran.

### upsert_ad_group

Creates or replaces one planned ad group.

| Argument | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | A planned ad group of this campaign. Compared after `normalize`. |
| `default_cpc` | number | yes | Currency units, at most 2 decimals, above 0. |
| `cpc_rationale` | string | no | Shows in the report. |
| `keywords` | object | yes | The keyword spec. |
| `negatives` | array | no | Ad group negatives, each `{"text", "match"}`. |
| `rsa` | object | yes | The ad group specific ad texts. |

`keywords`.

| Field | Type | Required | Description |
|---|---|---|---|
| `variants` | string array | yes | 1 to 6 ways people write one search. Variants of different searches fail `E26`. |
| `modifiers` | string array | no | 0 to 10 intent words. |
| `exact_heads` | boolean | no, default `true` | Adds every variant as exact match. |
| `extra` | array | no | 0 to 20 explicit `{"text", "match"}` keywords. |

`match` is `phrase` or `exact`. A one-word variant is phrase only in a `brand` campaign, or when `exact_heads` is `false`. Otherwise it is exact only.

`rsa`.

| Field | Type | Required | Description |
|---|---|---|---|
| `headlines` | string array | yes | 3 to 7, at most 30 characters. |
| `descriptions` | string array | yes | 1 to 2, at most 90 characters. |
| `path1` | string | no | At most 15 characters. Empty for none. |
| `path2` | string | no | At most 15 characters. Needs `path1`. |

mads expands the keywords, merges the RSA with the brand kit, validates the ad group and replaces any ad group with the same name. Result.

```json
{"ad_group": "marca-vinellu", "keywords": 8, "negatives": 0, "merged_headlines": 13, "merged_descriptions": 4}
```

Codes: `E01`, `E02`, `E03`, `E04`, `E05`, `E07`, `E08`, `E09`, `E11`, `E12`, `E13`, `E14`, `E22`, `E23`, `E26`. Warnings `W01`, `W02`, `W03`, `W04`, `W09`. In `mads optimize` also `LIVE_DETAIL`, `E28` and `W10`. Only issues on this ad group are reported.

### set_campaign_negatives

| Argument | Type | Required | Description |
|---|---|---|---|
| `negatives` | array of `{"text", "match"}` | yes | Replaces the campaign-level list. 0 to 100. |

Result `{"negatives": 8}`. Codes: `E01`, `E02`, `E08`, `E09`, `E13`. `E09` fires when a negative blocks a keyword already built in the campaign.

### set_assets

| Argument | Type | Required | Description |
|---|---|---|---|
| `sitelinks` | array | yes | 2 to 8. Each `{"text", "description1", "description2", "url"}`. Only `text` and `url` are required. |
| `callouts` | string array | yes | 2 to 10, at most 25 characters. |
| `snippets` | array | no | 0 to 2. Each `{"header", "values"}` with 3 to 10 values of at most 25 characters. |

Sitelink `text` has 25 characters at most. Descriptions have 35, both or none. `url` must be an allowed URL. `header` is one of `amenities`, `brands`, `courses`, `degree_programs`, `destinations`, `featured_hotels`, `insurance_coverage`, `models`, `neighborhoods`, `service_catalog`, `shows`, `styles`, `types`.

Result `{"saved": true}`. Codes: `E01`, `E02`, `E03`, `E05`, `E07`, `E13`, `E14`. Warnings `W01`, `W02`, `W05`.

### validate

No arguments. Validates the campaign as it stands and changes nothing. It always returns `ok: true`, with the issues inside `result`.

```json
{"ok": true, "result": {"errors": [], "warnings": []}, "warnings": []}
```

### finish

No arguments. Needs every planned ad group, the assets and zero errors, else it returns the errors. Result `{"ad_groups": 4}`.

## Image and App campaign mission

Used for `performance_max`, `demand_gen` and `app_installs` campaigns. The tools are `get_brief`, `upsert_asset_group`, `set_image_briefs`, `validate` and `finish`. See [Image and app campaigns](../explanation/image-campaigns.md).

### get_brief

No arguments. Like the Search `get_brief`, with these differences.

| Key | Content |
|---|---|
| `campaign` | Adds `kind`. Lists `planned_asset_groups` instead of `planned_ad_groups`. |
| `entities` | `id`, `name`, `url`, `category`, `notes`, `has_photo`. |
| `built_asset_groups` | `name`, `headlines`, `images` as counts. |
| `design` | The text of `DESIGN.md`. Only when there is one. |
| `image_rules` | Limits per kind, the ratios with their pixel sizes, and the brief rules. |

There is no `budget`, `campaign_negatives` or `assets_set`.

### upsert_asset_group

Creates or replaces the texts of one planned asset group. Its image briefs are kept.

| Argument | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | A planned asset group of this campaign. |
| `business_name` | string | no | At most 25 characters. Not checked for App campaigns. |
| `headlines` | string array | yes | See the table below. |
| `long_headlines` | string array | no | Performance Max only, 1 to 5 of at most 90 characters. |
| `descriptions` | string array | yes | See the table below. |
| `search_themes` | string array | no | Performance Max only, 0 to 25 of at most 80 characters. |

| Kind | Headlines | Descriptions |
|---|---|---|
| `performance_max` | 3 to 15 of at most 30, no `!` | 2 to 5 of at most 90, one of at most 60 |
| `demand_gen` | 1 to 5 of at most 40 | 1 to 5 of at most 90 |
| `app_installs` | 1 to 5 of at most 30, no `!` | 1 to 5 of at most 90 |

The final URL comes from the plan. Result `{"saved": "asset group <name>"}`.

Codes: `E01`, `E02`, `E03`, `E04`, `E05`, `E07`, `E12`, `E13`, `E22`, `E23`. Warnings `W01`, `W02`. Briefs kept from an earlier call are checked again, so `E16`, `E19`, `W06` and `W07` can appear. In `mads optimize` also `LIVE_DETAIL`.

### set_image_briefs

Replaces every brief of one asset group.

| Argument | Type | Required | Description |
|---|---|---|---|
| `asset_group` | string | yes | An asset group already created with `upsert_asset_group`, else `E12`. |
| `images` | array | yes | 0 to 20 briefs. |

Each brief.

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Slug, unique in the group. It becomes the file name. |
| `ratio` | string | yes | See the table below. |
| `prompt` | string | yes | 20 to 1500 characters. |
| `reference` | string | no | Catalog id whose item has an `image`. Empty for none. |

| Ratio | Size written | Kinds |
|---|---|---|
| `landscape` | 1200 x 628 | all |
| `square` | 1200 x 1200 | all |
| `portrait` | 960 x 1200 | all |
| `vertical` | 1080 x 1920 | `demand_gen` only |

A brief keeps its generated file when its `id`, `ratio` and `prompt` match a brief the group had, or one the live run had. Any other change makes the image step draw it again. Result `{"saved": "<group>: <n> image briefs"}`.

Codes: `E16`, `E19`, `E13`. Warnings `W06`, `W07`.

### validate and finish

`validate` works as in the Search mission. `finish` also fails with `E16` for an asset group without briefs. Neither reports `E20`, because the pictures are made after the mission. `finish` result `{"asset_groups": 2}`.

## Init mission

Used by `mads init`. See [Draft from your site](../how-to/draft-from-your-site.md).

With an agent CLI, the init agent can also use the CLI's own web search. That tool is not a mads tool and does not go through the MCP server.

| Rule | Value |
|---|---|
| Reachable hosts | The start host and its `www.` or apex sibling. |
| `robots.txt` | Respected for page fetches. |
| Refused hosts | IP literals, and names that resolve to loopback, private, link-local, CGNAT, multicast or unspecified addresses. |
| Override | `MADS_ALLOW_PRIVATE_HOSTS` set to a non-empty value. |
| Pages per mission attempt | 30. |
| Call budget | `--max-turns` times 4. |

### fetch_page

| Argument | Type | Required | Description |
|---|---|---|---|
| `url` | string | yes | Absolute URL on the business website. |

```json
{"url": "https://vinellu.com/", "status": 200, "title": "", "description": "", "text": "", "links": [], "image": "https://vinellu.com/share.jpg", "theme_color": "#AD1457"}
```

`text` is the visible text, at most 8000 characters. `links` are absolute same-site URLs, at most 200. `image` is the `og:image` and `theme_color` the `theme-color` meta tag, each absent when the page has none. Logo candidates and app store links are collected but not shown to the agent. The page and its links join the set of seen URLs. Codes: `HOST`, `FETCH`, `LIMIT`.

### fetch_sitemap

| Argument | Type | Default | Description |
|---|---|---|---|
| `url` | string | `""` | Sitemap URL. Empty looks in `robots.txt`, then `/sitemap.xml`. |
| `contains` | string | `""` | Case-insensitive text the URL must contain. |
| `offset` | integer | `0` | URLs to skip. |
| `limit` | integer | `50` | Page size, clamped to 1 to 200. |

Result `{"total": 8657, "offset": 0, "urls": [], "skipped": []}`. `total` counts URLs that match `contains`. `skipped` lists each sitemap file that could not be read, as `url: reason`.

| Sitemap limit | Value |
|---|---|
| File size | 50 MB |
| Index depth | 2 levels |
| Child sitemaps | 50 |
| URLs kept | 500,000 |
| Compression | `.xml.gz` is read |

The parsed result is cached for the mission. Returned URLs join the set of seen URLs. Codes: `HOST`, `FETCH`.

### search_site

Finds the pages of a list of names in the default sitemap.

| Argument | Type | Default | Description |
|---|---|---|---|
| `names` | string array | required | 1 to 50 names. |
| `limit` | integer | `5` | URLs returned per name, clamped to 1 to 20. |

A name and each URL path are turned into slugs first. A URL matches when every word of the name is in its slug. Shorter slugs come first. It shares the `fetch_sitemap` cache. Returned URLs join the set of seen URLs.

```json
{"checked": 2, "found": 1, "results": [{"name": "Alamos Malbec", "total": 2, "urls": []}, {"name": "Nope", "total": 0, "urls": []}], "skipped": []}
```

`checked` and `found` are the values to save as `names_checked` and `names_found` in the research. `skipped` is the same list as in `fetch_sitemap`. Codes: `QUERY` (an empty list, more than 50 names, or a name with no letters or digits, with the path `names[i]`), `FETCH`.

### write_business

Saves the business profile. Replaces the previous one.

| Field | Type | Required |
|---|---|---|
| `name`, `url`, `language`, `goal`, `description` | string | yes |
| `locations` | string array | yes |
| `restricted` | string array | yes |
| `conversion_tracking` | boolean | no |
| `brand_terms`, `competitors`, `avoid` | string array | no |
| `pages` | array of `{"name", "url"}` | no |
| `focus` | `{"name", "urls", "terms"}` | no |
| `strategy` | `{"products"}` | no |

`restricted` takes `alcohol`, `gambling`, `healthcare`, `financial_services`, `political`, `sexual_content`, or `[]` for none. A call without it fails with `ARGS`. See [Restricted categories](../explanation/restricted-categories.md).

The draft is rendered and parsed like `business.toml`, so the [business.toml](business-toml.md) rules apply. Page and focus URLs must have been fetched, listed in the sitemap or returned by `search_site` (`E07`). The budget and currency come from the CLI flags.

| Situation | Code |
|---|---|
| `--focus` and `focus` has no name or lacks the start URL | `E22` |
| `--focus` and `focus.terms` is empty | `E23` |
| Strategy products given when the task already sets them | `E12` |
| A strategy product outside the focus | `E22` |
| More strategy products than `--top` | `E13` |
| A strategy with no products | `E32` |
| The draft fails `business.toml` parsing | `INPUT` |

Result `{"saved": true}`.

### write_research

Saves what the agent learned. Replaces the previous research.

| Field | Type | Required | Rule |
|---|---|---|---|
| `summary` | string | yes | 40 to 3000 characters. |
| `opportunities` | array | no | At most 20, best expected return first. |
| `open_questions` | string array | no | At most 10, each 1 to 300 characters. |

Each opportunity.

| Field | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters. |
| `intent` | string | yes | `brand`, `catalog`, `generic` or `competitor`. |
| `searches` | string array | yes | 1 to 10, each 1 to 80 characters. |
| `demand` | string | yes | `high`, `medium` or `low`. |
| `competition` | string | yes | `high`, `medium` or `low`. |
| `evidence` | string | yes | 1 to 600 characters. |
| `sources` | string array | no | At most 10 absolute http(s) URLs. |
| `names_checked` | integer | no | Default `0`. |
| `names_found` | integer | no | Default `0`. Cannot pass `names_checked`. |

Every problem comes back at once as `INPUT`, each with its path, such as `opportunities[0].demand`.

With web search in the mission, the `sources` of all opportunities must cite at least 3 distinct pages outside the business site. Pages on the site, with or without `www.`, do not count. Fewer fail `INPUT` with the path `opportunities[].sources`. A failed call keeps the previous research. Result `{"saved": true}`.

### write_design

Optional.

| Argument | Type | Required | Description |
|---|---|---|---|
| `style` | string | yes | 1 to 600 characters. |
| `imagery` | string | no | At most 600. |
| `voice` | string | no | At most 600. |
| `avoid` | string array | no | At most 10. |

Replaces the previous draft. Codes: `E13`. Result `{"saved": true}`.

### add_catalog_items

| Argument | Type | Required | Description |
|---|---|---|---|
| `items` | array | yes | Catalog items. |

Each item has `name` and `url` (required) and `category`, `aliases` (string array), `third_party` (boolean), `notes` and `image`. The same rules apply as for [catalog.csv](catalog-csv.md). A duplicate URL is skipped.

| Situation | Code |
|---|---|
| `url` not fetched, listed in the sitemap or returned by `search_site` | `E07`, path `items[<i>].url` |
| `image` is not the `image` of a fetched page | `E07`, path `items[<i>].image` |
| A focus is set and `url` is not a focus URL | `E22` |
| The total would pass `--catalog-limit` | `E13` |
| The URL is not absolute http(s), or the CSV does not parse | `INPUT` |

Result `{"added": 20, "skipped": 0, "total": 20}`.

### finish

No arguments. Fails with `E12` when `write_business` or `write_research` was not called, with the path `business` or `research`. With a strategy it also fails with `E32`, path `strategy.products`, when a product has no catalog item. Result `{"catalog_items": 20}`.

After a successful `finish`, mads writes `business.toml`, `research.md`, `catalog.csv` (when there are items), `DESIGN.md`, `STRATEGY.md` (with a strategy) and `brand/logo.png` (when a logo candidate passes). See [Output files](output-files.md).

## Progress summaries

Each call emits `tool_called` and `tool_finished` events. The `summary` is a short line the tool writes, such as `rotulo-alamos: 14 kw, 0 neg, rsa 15/4` or `plan: 2 campaigns, 5 ad groups`.

## Next

* [Validation rules](validation-rules.md) has every `E` and `W` code.
* [How a run works](../explanation/how-a-run-works.md) shows where each mission runs.
* [Exit codes and errors](exit-codes-and-errors.md) covers what the CLI prints when a mission fails.
