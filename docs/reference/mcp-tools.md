# MCP tools

This page lists every tool an agent can call, with its arguments, result and errors.

These are the tools the agents call. The plan and campaign tools are in `crates/mads-core/src/tools`. The init tools are in `crates/mads-core/src/init`. API providers call them through the model's tool calling. The tool definitions live in one place. Agent CLI providers reach the same definitions through a local MCP server that mads starts for each mission. See [Agent CLIs](../guides/agent-clis.md).

A mission sees only its own toolset. Calling any other tool returns `UNKNOWN_TOOL`.

## Result format

Success.

```json
{"ok": true, "result": {}, "warnings": []}
```

Failure. The call changed nothing.

```json
{"ok": false, "errors": [{"code": "E01", "severity": "error", "path": "rsa.headlines[0]", "message": "34 chars, limit is 30"}], "warnings": []}
```

Bad arguments.

```json
{"ok": false, "errors": [{"code": "ARGS", "severity": "error", "path": "", "message": "<parser error>"}], "warnings": []}
```

Every tool has `additionalProperties: false`. An unknown field is an `ARGS` error. A mission also has a tool call budget of `--max-turns` times 4. After it, calls return `LIMIT`. Codes are in [Validation rules](validation-rules.md).

## Plan mission

Tools: `get_business`, `query_catalog`, `set_brand_kit`, `set_account_plan`, `finish`.

### get_business

No arguments. Returns the business profile, the budget and the rules every tool enforces.

```json
{"business": {}, "budget": {"daily": 50.0, "currency": "BRL", "max_cpc": 3.0}, "catalog_size": 4, "catalog_photos": 1, "image_campaigns": {"available": false, "reason": "no image model in this run (set --image-provider and its API key)"}, "rules": {}}
```

`rules` has the text limits, the count ranges, the intents (`brand`, `catalog`, `generic`, `competitor`), the match types (`phrase`, `exact`), `campaign_kinds` with the bid strategies of each kind, the snippet headers and `max_ad_groups`.

When `business.toml` has `[campaigns] formats`, the result also has `required_formats`, and `set_account_plan` refuses a plan without one of them (`E21`).

`focus` is the `[focus]` table of `business.toml`, or null. With a focus, every landing page must be a focus URL (`E22`), and a planned group without entity lands on the first focus URL instead of `business.url`. Both `get_brief` tools return it too.

`app` is the `[app]` table with its `store_url`, or null. `app_campaigns.available` says whether `app_installs` can be planned: it needs an image model and `[app]`, not a logo. The planned groups of an App campaign land on the store page.

`catalog_photos` counts the catalog items with an `image`. `image_campaigns.available` is `true` only when the run has an image model and `business.toml` has a logo. Otherwise `reason` says which one is missing, `no image model in this run (set --image-provider and its API key)` or `no logo: set [brand] logo in business.toml`.

When `business.toml` has a `[research]` table, the result also has `research`, the Markdown text of that file. Without notes the key is absent.

### query_catalog

| Argument | Type | Default | Description |
|---|---|---|---|
| `category` | string | `""` | Only items of this category. Exact, case-insensitive. Empty for all. |
| `contains` | string | `""` | Case-insensitive text searched in id, name and aliases. |
| `offset` | integer | `0` | Items to skip. |
| `limit` | integer | `50` | Page size. Clamped to 1 to 200. |

Result.

```json
{"total": 4, "offset": 0, "items": [], "categories": [{"name": "malbec argentino", "count": 2}]}
```

`items` are the catalog rows (`id`, `name`, `url`, `category`, `aliases`, `third_party`, `notes`). `categories` counts every category in the whole catalog, not only the filtered items.

### set_brand_kit

| Argument | Type | Required | Description |
|---|---|---|---|
| `headlines` | string array | yes | 8 to 12 headlines, 30 characters at most. |
| `descriptions` | string array | yes | 2 to 3 descriptions, 90 characters at most. |

Checks `E01`, `E02`, `E03`, `E04`, `E05`, `E13`, plus warnings `W01` and `W02`. Replaces the previous kit. Result `{"headlines": 10, "descriptions": 3}`.

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
| `daily_budget` | number | yes | In currency units, at most 2 decimals. All campaigns must sum to `budget.daily`. |
| `bid_strategy` | object | yes | `{"type": "..."}`. `search` takes `manual_cpc`, other types return `UNSUPPORTED`. `performance_max` and `app_installs` take `maximize_conversions`. `demand_gen` takes `maximize_clicks` or `maximize_conversions`. A mismatch is `E17`. |
| `rationale` | string | yes | Why this budget share and bidding. |
| `ad_groups` | array | yes | The planned ad groups of a Search campaign, the planned asset groups of an image campaign. |

Each ad group.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Ad group name. Unique in the campaign. |
| `theme` | string | yes | What the ad group is about. |
| `entity_ids` | string array | no | Catalog ids this ad group covers. |
| `final_url` | string | no | Landing page. Empty resolves to the entity URL (one entity) or `business.url`. |

Checks `E06`, `E07`, `E10`, `E11`, `E12`, `E13`, `E17`, `E18`, `E21`, unknown catalog ids (`E12`) and `UNSUPPORTED`. An image campaign in a run without an image model or a logo is `UNSUPPORTED`. With image campaigns available, a plan without one needs a rationale with `No image campaign: <reason>` (`NO_IMAGE_REASON`). With the app available, a plan without `app_installs` needs `No app campaign: <reason>` (`NO_APP_REASON`). Neither is asked when `business.toml` sets `[campaigns] formats`. Result.

```json
{"campaigns": [{"name": "Vinellu - Marca", "slug": "vinellu-marca", "ad_groups": 1}]}
```

### finish

No arguments. Fails with `E12` when the brand kit or the plan is missing. Result `{"campaigns": 2}`.

## Campaign mission

Tools: `get_brief`, `upsert_ad_group`, `set_campaign_negatives`, `set_assets`, `validate`, `finish`. A campaign mission reads and changes only its own campaign.

### get_brief

No arguments. Returns what the agent needs.

```json
{
  "campaign": {"name": "", "slug": "", "intent": "", "daily_budget": 0.0, "bid_strategy": {}, "rationale": "", "planned_ad_groups": []},
  "brand_kit": {},
  "business": {},
  "budget": {"currency": "BRL", "max_cpc": 3.0},
  "entities": [],
  "built_ad_groups": [{"name": "", "keywords": 0, "negatives": 0, "default_cpc": 0.0}],
  "campaign_negatives": [],
  "assets_set": false,
  "rules": {}
}
```

`entities` are the full catalog rows of the entities in this campaign's ad groups. `built_ad_groups` is what an earlier attempt already saved.

### upsert_ad_group

Creates or replaces one planned ad group.

| Argument | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | A planned ad group of this campaign. |
| `default_cpc` | number | yes | Currency units, at most 2 decimals. |
| `cpc_rationale` | string | no | Why this number. Shows in the report. |
| `keywords` | object | yes | The keyword spec. |
| `negatives` | array | no | Ad group negatives, each `{"text", "match"}`. |
| `rsa` | object | yes | The ad group specific ad texts. |

`keywords`.

| Field | Type | Required | Description |
|---|---|---|---|
| `variants` | string array | yes | 1 to 6 ways people write the name. |
| `modifiers` | string array | no | 0 to 10 intent words. |
| `exact_heads` | boolean | no, default `true` | Add every variant as exact match. |
| `extra` | array | no | 0 to 20 explicit `{"text", "match"}` keywords. |

`match` is `phrase` or `exact`.

`rsa`.

| Field | Type | Required | Description |
|---|---|---|---|
| `headlines` | string array | yes | 3 to 7, at most 30 characters. |
| `descriptions` | string array | yes | 1 to 2, at most 90 characters. |
| `path1` | string | no | At most 15 characters. Empty for none. |
| `path2` | string | no | Needs `path1`. |

Example call.

```json
{
  "name": "marca-vinellu",
  "default_cpc": 0.6,
  "cpc_rationale": "Termo de marca, concorrência baixa.",
  "keywords": {"variants": ["vinellu", "vinellu app"], "modifiers": ["baixar", "grátis"], "exact_heads": true},
  "negatives": [],
  "rsa": {
    "headlines": ["Vinellu: app de vinhos", "Baixe o Vinellu grátis", "Vinellu: sua adega"],
    "descriptions": ["Baixe o Vinellu e fotografe o rótulo de qualquer vinho. Grátis."],
    "path1": "app",
    "path2": ""
  }
}
```

mads expands the keywords, merges the RSA with the brand kit, validates the ad group and replaces any ad group with the same name. Result.

```json
{"ad_group": "marca-vinellu", "keywords": 8, "negatives": 0, "merged_headlines": 13, "merged_descriptions": 4}
```

Checks `E01` to `E05`, `E08`, `E09`, `E11`, `E12`, `E13`, `E14`, plus warnings `W01` to `W04`. Only issues on this ad group are reported.

### set_campaign_negatives

| Argument | Type | Required | Description |
|---|---|---|---|
| `negatives` | array of `{"text", "match"}` | yes | Replaces the campaign-level list. |

Checks `E01`, `E02`, `E08`, `E09`, `E13`. A negative that blocks any keyword built so far in the campaign fails with `E09`. Result `{"negatives": 8}`.

### set_assets

| Argument | Type | Required | Description |
|---|---|---|---|
| `sitelinks` | array | yes | 2 to 8. Each `{"text", "description1", "description2", "url"}`. Only `text` and `url` are required. |
| `callouts` | string array | yes | 2 to 10, at most 25 characters. |
| `snippets` | array | no | 0 to 2. Each `{"header", "values"}` with 3 to 10 values. |

Sitelink text has 25 characters at most, descriptions 35 (both or none), `url` must be allowed. `header` is one of `amenities`, `brands`, `courses`, `degree_programs`, `destinations`, `featured_hotels`, `insurance_coverage`, `models`, `neighborhoods`, `service_catalog`, `shows`, `styles`, `types`.

Checks `E01`, `E03`, `E05`, `E07`, `E13`, `E14`, plus `W01`, `W02` and `W05`. Result `{"saved": true}`.

### validate

No arguments. Validates the campaign as it stands and changes nothing. It always returns `ok: true`, with the issues inside the result.

```json
{"ok": true, "result": {"errors": [], "warnings": []}, "warnings": []}
```

### finish

No arguments. Needs every planned ad group, assets and zero errors in the campaign. Otherwise it returns the errors. Result `{"ad_groups": 4}`.

## Image campaign mission

A `performance_max`, `demand_gen` or `app_installs` campaign gets these tools instead: `get_brief`, `upsert_asset_group`, `set_image_briefs`, `validate`, `finish`. Its system prompt is `prompts/image-campaign.md`.

### get_brief

No arguments. Like the Search `get_brief`, with `design` (the text of `DESIGN.md`, when there is one), `planned_asset_groups`, `built_asset_groups` (`name`, `headlines`, `images`), `entities` with `has_photo`, and `image_rules` with the limits of each kind and the ratios.

### upsert_asset_group

Creates or replaces the texts of one planned asset group. Its image briefs are kept.

| Argument | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | A planned asset group of this campaign. |
| `business_name` | string | no | At most 25 characters. App campaigns leave it empty. |
| `headlines` | string array | yes | Performance Max 3 to 15 of 30 characters. Demand Gen 1 to 5 of 40. App 1 to 5 of 30. |
| `long_headlines` | string array | no | Performance Max only, 1 to 5 of 90 characters. |
| `descriptions` | string array | yes | Performance Max 2 to 5 of 90, one of them 60 or fewer. Demand Gen 1 to 5 of 90. |
| `search_themes` | string array | no | Performance Max only, 0 to 25 of 80 characters. |

The final URL comes from the plan. Checks `E01`, `E02`, `E03`, `E04`, `E05`, `E07`, `E12`, `E13`, plus `W01` and `W02`.

### set_image_briefs

Replaces every brief of one asset group. A replaced brief loses its file, so the image step draws it again.

| Argument | Type | Required | Description |
|---|---|---|---|
| `asset_group` | string | yes | An asset group already created with `upsert_asset_group`, else `E12`. |
| `images` | array | yes | Up to 20 briefs. |

Each brief.

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Slug, unique in the group. It becomes the file name. |
| `ratio` | string | yes | `landscape` (1.91:1), `square`, `portrait` (4:5) or `vertical` (9:16). |
| `prompt` | string | yes | 20 to 1500 characters, in English. What the picture shows. |
| `reference` | string | no | Catalog id whose photo the picture must show. Empty for none. |

Checks `E13`, `E16`, `E19`, plus `W06` and `W07`.

### validate and finish

Same as Search. `E20` is left out: the pictures are made after the mission. `finish` also needs briefs in every asset group (`E16`).

## Init mission

Used by `mads init`. Tools: `fetch_page`, `fetch_sitemap`, `search_site`, `write_business`, `write_research`, `write_design`, `add_catalog_items`, `finish`. See [Init from a URL](../guides/init-from-url.md).

With an agent CLI the init agent can also use the CLI's own web search. That tool is not a mads tool and does not go through the MCP server. See [Agent CLIs](../guides/agent-clis.md#web-search-in-init).

The agent only reaches the start host and its `www.` or apex sibling. `robots.txt` is respected. IP-literal hosts and hosts that resolve to loopback, private or link-local addresses are refused. `MADS_ALLOW_PRIVATE_HOSTS=1` lifts that for local development.

### fetch_page

| Argument | Type | Required | Description |
|---|---|---|---|
| `url` | string | yes | Absolute URL on the business website. |

Result.

```json
{"url": "https://vinellu.com/", "status": 200, "title": "", "description": "", "text": "", "links": [], "image": "https://vinellu.com/share.jpg", "theme_color": "#AD1457"}
```

`text` is the visible text, at most 8000 characters. `links` are absolute same-site URLs, at most 200. `image` is the page's `og:image` and `theme_color` its `<meta name="theme-color">`, each absent when the page has none. The theme color goes into `DESIGN.md`. mads also collects logo candidates from the page, which the agent does not see. The page and its links join the set of seen URLs. At most 30 pages per mission attempt. Errors `HOST`, `FETCH`, `LIMIT`.

### fetch_sitemap

| Argument | Type | Default | Description |
|---|---|---|---|
| `url` | string | `""` | Sitemap URL. Empty looks in `robots.txt`, then `/sitemap.xml`. |
| `contains` | string | `""` | Case-insensitive text the URL must contain. |
| `offset` | integer | `0` | URLs to skip. |
| `limit` | integer | `50` | Page size. Clamped to 1 to 200. |

Result `{"total": 8657, "offset": 0, "urls": [], "skipped": []}`. `total` counts URLs that match `contains`. `skipped` lists each sitemap file that could not be read as `url: reason`. Files up to 50 MB are read. Indexes are followed 2 levels, with at most 50 child sitemaps. `.xml.gz` is supported. The parsed result is cached for the mission. Returned URLs join the set of seen URLs. Errors `HOST`, `FETCH`.

### search_site

Finds the pages of a list of names in the sitemap. One call checks up to 50 names, so a long list of candidates costs one turn, not fifty.

| Argument | Type | Default | Description |
|---|---|---|---|
| `names` | string array | | The names to look for, as people write them. 1 to 50. Required. |
| `limit` | integer | `5` | URLs returned per name. Clamped to 1 to 20. |

Each name and each URL path are turned into slugs first. Accents and case drop, and `%C3%A9` in a URL counts as `é`. A URL matches when every word of the name is in its path. Shorter paths come first, so `alamos-malbec` beats `alamos-malbec-2022`. It reads the default sitemap and shares the `fetch_sitemap` cache. Returned URLs join the set of seen URLs, so they can become catalog items.

```json
{"checked": 2, "found": 1, "results": [{"name": "Alamos Malbec", "total": 2, "urls": []}, {"name": "Nope", "total": 0, "urls": []}], "skipped": []}
```

`checked` and `found` are what the agent saves as `names_checked` and `names_found` in the research. `skipped` is the same list as in `fetch_sitemap`. When it is not empty, a name with `total` 0 may still have a page. Errors `QUERY` (an empty list, more than 50 names, or a name with no letters or digits, with the path `names[i]`) and `FETCH`. A failed call marks no URL as seen.

### write_business

Saves the business profile. Replaces the previous one.

| Field | Type | Required |
|---|---|---|
| `name`, `url`, `language`, `goal`, `description` | string | yes |
| `locations` | string array | yes |
| `conversion_tracking` | boolean | no |
| `brand_terms`, `competitors`, `avoid` | string array | no |
| `pages` | array of `{"name", "url"}` | no |
| `focus` | `{"name", "urls", "terms"}` | no |
| `restricted` | string array | yes | Google Ads restricted content categories, `[]` for none. Values in [Restricted categories](../guides/restricted-categories.md). A call without it fails with `ARGS`. |

The draft is validated like `business.toml`. With `--focus`, `focus` must have a name and list the start URL (`E22`) and have `terms` (`E23`), and its URLs must have been seen (`E07`). Once a focus is set, `add_catalog_items` refuses items whose URL is not a focus URL (`E22`). Page URLs must have been fetched, listed in the sitemap or returned by `search_site`. Errors `E07` and `INPUT`. Result `{"saved": true}`. The budget and currency come from the CLI flags, not from the agent.

### write_research

Saves what the agent learned. Replaces the previous research.

| Field | Type | Required | Rule |
|---|---|---|---|
| `summary` | string | yes | 40 to 3000 characters. |
| `opportunities` | array | no | At most 20, best expected return first. |
| `open_questions` | string array | no | At most 10, each 1 to 300 characters. |

Each opportunity has these fields.

| Field | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters. |
| `intent` | string | yes | `brand`, `catalog`, `generic` or `competitor`. |
| `searches` | string array | yes | 1 to 10, each 1 to 80 characters. |
| `demand` | string | yes | `high`, `medium` or `low`. |
| `competition` | string | yes | `high`, `medium` or `low`. |
| `evidence` | string | yes | 1 to 600 characters. |
| `sources` | string array | no | At most 10 absolute http(s) URLs, on the site or elsewhere. |
| `names_checked` | integer | no | Names from outside the site looked up with `search_site`. Default `0`. |
| `names_found` | integer | no | How many of them have a page. Cannot pass `names_checked`. Default `0`. |

Every problem comes back at once, each with its path, such as `opportunities[0].demand`. Error code `INPUT`.

When the mission has web search, the research must cite at least 3 distinct pages outside the business site across all `sources`. Pages on the site, with or without `www.`, do not count. Fewer fail with the path `opportunities[].sources`. Without web search there is no such rule. A failed call keeps the previous research. Result `{"saved": true}`. mads writes the research as `research.md`.

### write_design

| Argument | Type | Required | Description |
|---|---|---|---|
| `style` | string | yes | The visual feel of the brand. 1 to 600 characters. |
| `imagery` | string | no | What the brand's own photos show. At most 600. |
| `voice` | string | no | How the brand talks. At most 600. |
| `avoid` | string array | no | What pictures must never show. At most 10. |

Optional. Errors `E13`. Replaces the previous draft. mads adds the colors itself and writes `DESIGN.md` after finish.

### add_catalog_items

| Argument | Type | Required | Description |
|---|---|---|---|
| `items` | array | yes | Catalog items. |

Each item has `name` and `url` (required) and `category`, `aliases` (string array), `third_party` (boolean), `notes` and `image`. The checks match `catalog.csv`. URLs must have been fetched, listed in the sitemap or returned by `search_site` (`E07`). An `image` must be the `image` of a fetched page (`E07`, path `items[<i>].image`). Duplicates by URL are skipped. More items than `--catalog-limit` fail with `E13`. Result `{"added": 20, "skipped": 0, "total": 20}`.

### finish

No arguments. Fails with `E12` when `write_business` or `write_research` was not called. The error path is `business` or `research`. Result `{"catalog_items": 20}`. After it succeeds, mads downloads the first logo candidate that passes Google's checks into `brand/logo.png`, then writes `business.toml`, `research.md` and, when there are items, `catalog.csv`.

## Progress summaries

Each call emits `tool_called` and `tool_finished` events. The `summary` is a short line a tool writes, such as `rotulo-alamos: 14 kw, 0 neg, rsa 15/4` or `plan: 2 campaigns, 5 ad groups`. The progress output shows it.
