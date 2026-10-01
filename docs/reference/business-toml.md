# business.toml reference

This page lists every key of `business.toml` with its type, default and validation rule.

Unknown keys are rejected in every table, so a typo fails fast. Input errors exit with code `2` and name the key.

## [business]

| Key | Type | Required | Default | Validation |
|---|---|---|---|---|
| `name` | string | yes | | 1 to 80 characters after trim |
| `url` | string | yes | | Absolute `http` or `https` URL |
| `language` | string | yes | | BCP 47 tag, such as `pt-BR`. The primary subtag has 2 or 3 letters, other subtags 2 to 8 letters or digits. The CSV `Language` column gets the primary subtag. |
| `locations` | string array | yes | | Exactly 1 entry, 1 to 80 characters. A Google location name such as `Brazil`. |
| `goal` | string | yes | | 1 to 200 characters |
| `description` | string | yes | | 20 to 4000 characters |
| `conversion_tracking` | bool | no | `false` | |
| `brand_terms` | string array | no | `[lowercase(name)]` | Each 1 to 80 characters |
| `competitors` | string array | no | `[]` | Each 1 to 80 characters |
| `avoid` | string array | no | `[]` | Each 1 to 80 characters |

Length counts Unicode characters, not bytes.

### [[business.pages]]

Key pages. Each is an allowed URL and a candidate sitelink. Repeat the table for each page.

| Key | Type | Required | Validation |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters |
| `url` | string | yes | Absolute `http` or `https` URL |

At most 20 pages (`business.pages: at most 20 pages`).

## [budget]

| Key | Type | Required | Default | Validation |
|---|---|---|---|---|
| `daily` | number | yes | | Greater than 0, at most 2 decimals |
| `currency` | string | yes | | 3 uppercase ASCII letters. Informational, not exported. |
| `max_cpc` | number | no | none | Greater than 0, at most 2 decimals |

## [export]

The whole table is optional.

| Key | Type | Default | Validation and effect |
|---|---|---|---|
| `status` | `"Paused"` or `"Enabled"` | `"Paused"` | Exact case. Written to the `Campaign status` column. |
| `url_suffix` | string | `utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}` | `{mads_campaign}` becomes the campaign slug. Any other `{...}` is a Google ValueTrack parameter and stays as written. |
| `eu_political_ads` | bool | `false` | Written as `Yes` or `No`. |
| `decimal_comma` | bool | `true` when the primary language subtag is `pt`, `es`, `fr`, `de` or `it`, else `false` | Prints CPCs and budgets with a comma. |

## [catalog]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | no | Path to `catalog.csv`, relative to the directory that holds `business.toml`. |

Without `[catalog]` the catalog is empty.

## Complete example

```toml
[business]
name = "Vinellu"
url = "https://vinellu.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "cadastros no app"
description = """
App social de vinhos. A foto do rótulo mostra nota, safra,
harmonização e reviews. Mais de 170 mil rótulos. Grátis.
"""
conversion_tracking = false
brand_terms = ["vinellu"]
competitors = ["Vivino"]
avoid = ["melhor do mundo"]

[[business.pages]]
name = "Baixe o app"
url = "https://vinellu.com/app"

[budget]
daily = 50
currency = "BRL"
max_cpc = 3.0

[export]
status = "Paused"
url_suffix = "utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}"
eu_political_ads = false

[catalog]
file = "catalog.csv"
```

## Allowed URLs

A final URL or sitelink URL is accepted only when it is in the allowed set. The set is `business.url`, every `business.pages[].url` and every catalog `url`. URLs compare after normalization. The scheme and host are lowercased, the default port and the fragment are removed, and the path and query stay as they are. Anything else fails with `E07`.

## Validation messages

```text
business.name: must be 1 to 80 chars, got 0
business.url: must be an absolute http(s) URL
business.language: must be a BCP 47 tag such as pt-BR
business.locations: exactly 1 location is supported in v1
business.goal: must be 1 to 200 chars, got <n>
business.description: must be 20 to 4000 chars, got <n>
business.pages: at most 20 pages
budget.daily: must be greater than 0 with at most 2 decimals
budget.currency: must be 3 uppercase letters
budget.max_cpc: must be greater than 0 with at most 2 decimals
```

Entries in lists name their index, for example `business.competitors[1]` or `business.pages[0].url`.

See [The business file](../guides/business-file.md) for how to choose the values.
