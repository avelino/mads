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
| `restricted` | string array | no | `[]` | Google Ads restricted content categories: `alcohol`, `gambling`, `healthcare`, `financial_services`, `political`, `sexual_content`. Any other value is a parse error. `mads init` always writes it, `[]` included. See [Restricted categories](../guides/restricted-categories.md). |

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
| `decimal_comma` | bool | `true` when the primary language subtag is `pt`, `es`, `fr`, `de` or `it`, else `false` | Prints CPCs and budgets with a comma. The drive-folder files always use a dot, because that is how Editor writes money. |

## [google_ads]

Optional. Used by `--layout drive-folders`. The whole table can be absent. `mads init --force` keeps a table you wrote.

| Key | Type | Default | Validation and effect |
|---|---|---|---|
| `customer_id` | string | empty | 10 digits, or `123-456-7890`. Empty leaves the Customer ID column blank and adds warning `W11` on a drive-folder export. `000-000-0000` and `0000000000` are rejected. |
| `tracking_template` | string | empty | 1 to 2048 characters when set. `{mads_campaign}` becomes the campaign slug. Empty means the file uses `{lpurl}?` plus `export.url_suffix`. |
| `devices` | string | empty | 1 to 80 characters when set. Written as given, for example `Mobile;Desktop;Tablet`. |
| `labels` | string array | `[]` | Each 1 to 80 characters. Joined with `;` into the Label column. |
| `app_id` | string | empty | Google Play package (`com.example.app`) or a numeric App Store id. Empty skips `B8-extensao-app.csv`. |

## [catalog]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | no | Path to `catalog.csv`, relative to the directory that holds `business.toml`. |

Without `[catalog]` the catalog is empty.

## [research]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | no | Path to the research notes, relative to the directory that holds `business.toml`. `mads init` writes `research.md` and this key. |

The file is Markdown. `generate` reads it and the plan agent gets it from `get_business` as `research`. At most 20000 characters, because it goes into every plan prompt. A longer file fails with the key `research.file`. A missing file is an error. Without `[research]` the plan runs without notes.

## [brand]

| Key | Type | Required | Description |
|---|---|---|---|
| `logo` | path | no | Square logo, relative to the directory that holds `business.toml`. `mads init` writes `brand/logo.png` and this key when it finds a logo on the site. |

The logo must be PNG or JPEG, square within 1 percent, at least 144x144 pixels and at most 150 KB. Anything else fails with the key `brand.logo` and the reason, for example `brand.logo: logo.png: 400x200 is not square`. A missing file is an error.

Image campaigns (Performance Max and Demand Gen) need it. Without `[brand]` the plan can only use Search. See [Image campaigns](../guides/image-campaigns.md).

## [app]

| Key | Type | Required | Description |
|---|---|---|---|
| `store` | string | yes | `google_play` or `app_store`. |
| `id` | string | yes | The package name on Google Play (`com.example.app`), the numeric id on the App Store. |

`app_installs` campaigns need it (`E24`). Their ads link to the store page. `mads init` writes it when a fetched page links to the app, Google Play first. `init --force` keeps a hand-written `[app]` when the site links to none.

## [focus]

| Key | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The offer as people say it, 1 to 80 characters. |
| `urls` | string array | yes | 1 to 10 absolute URLs: the page of the offer and its close variants. |
| `terms` | array of string arrays | no | Up to 5 groups of words, 1 to 10 each. Every keyword must contain one word of each group (`E23`), so a search for another offer is never bought. |

With `[focus]` the account advertises that one offer, not the whole business. Every final URL must be one of `urls` (`E22`), and the agents keep keywords and ads about the offer. Sitelinks can still use `business.pages`. `mads init --focus` writes it.

```toml
[focus]
name = "Ônibus Belo Horizonte <> São Paulo"
urls = [
  "https://www.buser.com.br/onibus/belo-horizonte-mg/sao-paulo-sp",
  "https://www.buser.com.br/onibus/sao-paulo-sp/belo-horizonte-mg",
]
terms = [["bh", "belo horizonte"], ["sp", "sao paulo"]]
```

With these terms `clickbus bh sp` is accepted and a bare `clickbus` is refused, because it also matches searches for every other route.

## [design]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | no | Path to `DESIGN.md`, relative to the directory that holds `business.toml`. `mads init` writes both. |

`DESIGN.md` is the brand identity of image campaigns. The image campaign agent gets the whole file in `get_brief`, and every prompt sent to the image model gets the colors listed under its `## Colors` heading. At most 8000 characters, a longer file fails with the key `design.file`. See [Image campaigns](../guides/image-campaigns.md#brand-identity).

## [campaigns]

| Key | Type | Required | Description |
|---|---|---|---|
| `formats` | string array | no | Formats the account must have, at least one campaign each: `search`, `performance_max`, `demand_gen`. Without `[campaigns]` the plan agent picks. |

The plan agent still decides budget shares, intents and groups. A plan without one of the formats is refused with `E21`, and the agent fixes it.

Checked when the file loads, with the key `campaigns.formats`:

- an empty list, or a format listed twice
- an image format without `[brand] logo`: `Demand Gen needs [brand] logo`
- `performance_max` without `conversion_tracking = true`

`mads generate` also exits `2` when an image format is listed and the run has no image model.

```toml
[campaigns]
formats = ["search", "demand_gen"]
```

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

[research]
file = "research.md"

[brand]
logo = "brand/logo.png"

[design]
file = "DESIGN.md"
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
google_ads.customer_id: use 10 digits or 123-456-7890
google_ads.customer_id: 000-000-0000 is a placeholder: leave customer_id empty or set the real account id
google_ads.app_id: Google Play takes a package name such as com.example.app, the App Store a numeric id
```

Entries in lists name their index, for example `business.competitors[1]` or `business.pages[0].url`.

See [The business file](../guides/business-file.md) for how to choose the values.
