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
{"business": {}, "budget": {"daily": 50.0, "currency": "BRL", "max_cpc": 3.0}, "catalog_size": 4, "rules": {}}
```

`rules` has the text limits, the count ranges, the intents (`brand`, `catalog`, `generic`, `competitor`), the match types (`phrase`, `exact`), `bid_strategies` (`["manual_cpc"]`), the snippet headers and `max_ad_groups`.

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
| `intent` | string | yes | `brand`, `catalog`, `generic` or `competitor`. |
| `daily_budget` | number | yes | In currency units, at most 2 decimals. All campaigns must sum to `budget.daily`. |
| `bid_strategy` | object | yes | `{"type": "manual_cpc"}`. The schema also lists `maximize_clicks` and `maximize_conversions`, which return `UNSUPPORTED`. |
| `rationale` | string | yes | Why this budget share and bidding. |
| `ad_groups` | array | yes | The planned ad groups. |

Each ad group.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Ad group name. Unique in the campaign. |
| `theme` | string | yes | What the ad group is about. |
| `entity_ids` | string array | no | Catalog ids this ad group covers. |
| `final_url` | string | no | Landing page. Empty resolves to the entity URL (one entity) or `business.url`. |

Checks `E06`, `E07`, `E10`, `E11`, `E12`, `E13`, unknown catalog ids (`E12`) and `UNSUPPORTED`. Result.

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

## Init mission

Used by `mads init`. Tools: `fetch_page`, `fetch_sitemap`, `write_business`, `add_catalog_items`, `finish`. See [Init from a URL](../guides/init-from-url.md).

The agent only reaches the start host and its `www.` or apex sibling. `robots.txt` is respected. IP-literal hosts and hosts that resolve to loopback, private or link-local addresses are refused. `MADS_ALLOW_PRIVATE_HOSTS=1` lifts that for local development.

### fetch_page

| Argument | Type | Required | Description |
|---|---|---|---|
| `url` | string | yes | Absolute URL on the business website. |

Result.

```json
{"url": "https://vinellu.com/", "status": 200, "title": "", "description": "", "text": "", "links": []}
```

`text` is the visible text, at most 8000 characters. `links` are absolute same-site URLs, at most 200. The page and its links join the set of seen URLs. At most 30 pages per mission attempt. Errors `HOST`, `FETCH`, `LIMIT`.

### fetch_sitemap

| Argument | Type | Default | Description |
|---|---|---|---|
| `url` | string | `""` | Sitemap URL. Empty looks in `robots.txt`, then `/sitemap.xml`. |
| `contains` | string | `""` | Case-insensitive text the URL must contain. |
| `offset` | integer | `0` | URLs to skip. |
| `limit` | integer | `50` | Page size. Clamped to 1 to 200. |

Result `{"total": 8657, "offset": 0, "urls": []}`. `total` counts URLs that match `contains`. Indexes are followed 2 levels, with at most 50 child sitemaps. `.xml.gz` is supported. The parsed result is cached for the mission. Returned URLs join the set of seen URLs. Errors `HOST`, `FETCH`.

### write_business

Saves the business profile. Replaces the previous one.

| Field | Type | Required |
|---|---|---|
| `name`, `url`, `language`, `goal`, `description` | string | yes |
| `locations` | string array | yes |
| `conversion_tracking` | boolean | no |
| `brand_terms`, `competitors`, `avoid` | string array | no |
| `pages` | array of `{"name", "url"}` | no |

The draft is validated like `business.toml`. Page URLs must have been fetched or listed in the sitemap. Errors `E07` and `INPUT`. Result `{"saved": true}`. The budget and currency come from the CLI flags, not from the agent.

### add_catalog_items

| Argument | Type | Required | Description |
|---|---|---|---|
| `items` | array | yes | Catalog items. |

Each item has `name` and `url` (required) and `category`, `aliases` (string array), `third_party` (boolean) and `notes`. The checks match `catalog.csv`. URLs must have been fetched or listed in the sitemap (`E07`). Duplicates by URL are skipped. More items than `--catalog-limit` fail with `E13`. Result `{"added": 20, "skipped": 0, "total": 20}`.

### finish

No arguments. Fails with `E12` when `write_business` was not called. Result `{"catalog_items": 20}`. After it succeeds, mads writes `business.toml` and, when there are items, `catalog.csv`.

## Progress summaries

Each call emits `tool_called` and `tool_finished` events. The `summary` is a short line a tool writes, such as `rotulo-alamos: 14 kw, 0 neg, rsa 15/4` or `plan: 2 campaigns, 5 ad groups`. The progress output shows it.
