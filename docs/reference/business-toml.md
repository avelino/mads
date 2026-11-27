# business.toml reference

Look up every table and key of `business.toml` with its type, default and the rule mads checks when it loads the file.

Unknown keys are rejected in every table. A bad file exits with code `2` and the message names the key. See [Exit codes and errors](exit-codes-and-errors.md). For how to choose the values, see [Write the business file](../how-to/write-the-business-file.md).

## Conventions

- Lengths count Unicode characters after trimming spaces at both ends.
- Paths in `file` and `logo` keys are relative to the directory that holds `business.toml`.
- Money keys are numbers greater than 0 with at most 2 decimals. mads stores them as integer cents.
- A list entry that fails names its index, for example `business.competitors[1]` or `business.pages[0].url`.

## Tables

| Table | Required | Repeats |
|---|---|---|
| `[business]` | yes | no |
| `[[business.pages]]` | no | yes, at most 20 |
| `[budget]` | yes | no |
| `[export]` | no | no |
| `[google_ads]` | no | no |
| `[catalog]` | no | no |
| `[research]` | no | no |
| `[brand]` | no | no |
| `[design]` | no | no |
| `[campaigns]` | no | no |
| `[app]` or `[[app]]` | no | one per store |
| `[focus]` | no | no |
| `[strategy]` | no | no |

## [business]

| Key | Type | Required | Default | Rule |
|---|---|---|---|---|
| `name` | string | yes | | 1 to 80 characters. |
| `url` | string | yes | | Absolute `http` or `https` URL. |
| `language` | string | yes | | BCP 47 tag such as `pt-BR`. The primary subtag has 2 or 3 letters. Every other subtag has 2 to 8 letters or digits. The CSV `Language` column gets the primary subtag. |
| `locations` | string array | yes | | 1 to 10 entries. Each is 1 to 80 characters, has no `;` and does not repeat another entry (compared lowercase). Written to the location cell joined with `;`. |
| `goal` | string | yes | | 1 to 200 characters. |
| `description` | string | yes | | 20 to 4000 characters. |
| `conversion_tracking` | bool | no | `false` | None. |
| `brand_terms` | string array | no | `[lowercase(name)]` | Each entry 1 to 80 characters. An empty list is accepted and replaces the default. |
| `competitors` | string array | no | `[]` | Each entry 1 to 80 characters. |
| `avoid` | string array | no | `[]` | Each entry 1 to 80 characters. |
| `restricted` | string array | no | `[]` | Any of `alcohol`, `gambling`, `healthcare`, `financial_services`, `political`, `sexual_content`. Any other value is a parse error. `mads init` always writes the key, `[]` included. |

For what `restricted` changes in a run, see [Restricted categories](../explanation/restricted-categories.md).

### [[business.pages]]

Repeat the table for each page. Each page is part of the [allowed URLs](#allowed-urls).

| Key | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters. |
| `url` | string | yes | Absolute `http` or `https` URL. |

More than 20 pages fails with `business.pages: at most 20 pages`.

## [budget]

| Key | Type | Required | Default | Rule |
|---|---|---|---|---|
| `daily` | number | yes | | Money. The campaign budgets of a plan must add up to it (`E06`). |
| `currency` | string | yes | | 3 uppercase ASCII letters. Shown in `report.md` and given to the agents. It is not written to the CSV files. |
| `max_cpc` | number | no | none | Money. A CPC above it fails with `E11`. |
| `target_cpa` | number | no | none | Money. The most you pay for one conversion. `mads optimize` compares each campaign with it. |
| `target_cpi` | number | no | none | Money. The most you pay for one app install. `mads optimize` judges App campaigns with it. An App campaign is not judged by `target_cpa`. |
| `target_cpa_in_app` | number | no | none | Money. The most you pay for one in-app action. With `mads optimize --app-events` it judges App campaigns in place of `target_cpi`. It is also written as `Target CPA` for an app that optimizes for `in_app_actions`. |

See [Optimize a live account](../how-to/optimize-a-live-account.md) for how the targets are used.

## [export]

The whole table is optional.

| Key | Type | Default | Rule and effect |
|---|---|---|---|
| `status` | `"Paused"` or `"Enabled"` | `"Paused"` | Exact case. Written to `Campaign status` for campaigns that are not in the account yet. In `mads optimize`, a campaign the live account has and the plan keeps is exported `Enabled` whatever this says. |
| `url_suffix` | string | `utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}` | `{mads_campaign}` becomes the campaign slug. Any other `{...}` is a Google ValueTrack parameter and stays as written. |
| `eu_political_ads` | bool | `false` | Written as `Yes` or `No`. |
| `decimal_comma` | bool | `true` when the primary language subtag is `pt`, `es`, `fr`, `de` or `it`, otherwise `false` | Prints CPCs and budgets with a comma in the `bulk/` CSV files and in `report.md`. `editor/account.csv` and the automation package always write money with a dot. |

## [google_ads]

The whole table is optional. The automation package reads it. See [Automation package](automation-package.md).

| Key | Type | Default | Rule and effect |
|---|---|---|---|
| `customer_id` | string | empty | 10 digits, or `123-456-7890`. Empty leaves the Customer ID setting blank and raises `W11`. `000-000-0000` and `0000000000` are rejected. |
| `tracking_template` | string | empty | 1 to 2048 characters when set. `{mads_campaign}` becomes the campaign slug. Empty means `{lpurl}?` followed by `export.url_suffix`. |
| `devices` | string | empty | 1 to 80 characters when set. Written as given, for example `Mobile;Desktop;Tablet`. |
| `labels` | string array | `[]` | Each entry 1 to 80 characters, trimmed, with no `;`. Joined with `;` into the Label column. |
| `app_id` | string | empty | A Google Play package such as `com.example.app`, or a numeric App Store id. Without it the package has no `Structure/S8. App Extension.xlsx`. |

## [catalog]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | yes, when the table exists | Path to `catalog.csv`. A missing file is an error. |

Without `[catalog]` the catalog is empty. The columns of the file are in [catalog.csv](catalog-csv.md).

## [research]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | yes, when the table exists | Markdown research notes. At most 20000 characters. |

A longer file fails with the key `research.file`. A missing file is an error. `mads init` writes `research.md` and this table. The plan agent gets the text from `get_business` as `research`. Without `[research]` the plan runs without notes.

## [brand]

| Key | Type | Required | Description |
|---|---|---|---|
| `logo` | path | yes, when the table exists | The logo for image campaigns. |

The file must meet all of these.

| Check | Value |
|---|---|
| Format | PNG or JPEG |
| Size | At most 150 KB (153600 bytes) |
| Smallest side | 144 pixels |
| Shape | Square, with a 1 percent tolerance on the ratio |

A failure uses the key `brand.logo` and names the file path, for example `brand.logo: <path>: 400x200 is not square`. A missing file is an error. `mads init` writes `brand/logo.png` and this table when it finds a logo on the site. See [Add image campaigns](../how-to/add-image-campaigns.md).

## [design]

| Key | Type | Required | Description |
|---|---|---|---|
| `file` | path | yes, when the table exists | `DESIGN.md`, the brand identity of image campaigns. At most 8000 characters. |

A longer file fails with the key `design.file`. The image campaign agent gets the whole file in `get_brief`. The hex colors listed under its `## Colors` heading go into every prompt sent to the image model. See [Image campaigns](../explanation/image-campaigns.md).

## [campaigns]

| Key | Type | Required | Description |
|---|---|---|---|
| `formats` | string array | yes, when the table exists | Formats the account must have, at least one campaign each. Values: `search`, `performance_max`, `demand_gen`, `app_installs`. |

Without `[campaigns]` the plan agent picks the formats. A plan that leaves one out fails with `E21`.

Checked when the file loads, with the key `campaigns.formats`:

| Check | Message |
|---|---|
| Empty list | `list at least one format, or remove [campaigns]` |
| A format twice | `<format> is listed twice`, with the format label, for example `Demand Gen is listed twice` |
| `performance_max`, `demand_gen` or `app_installs` without `[brand]` | `<format> needs [brand] logo`, with the label `Performance Max`, `Demand Gen` or `App - Installs` |
| `performance_max` with `conversion_tracking = false` | `Performance Max needs business.conversion_tracking = true` |

An unknown format name is a parse error. `mads generate` also exits `2` when an image format or `app_installs` is listed and the run has no image model.

```toml
[campaigns]
formats = ["search", "demand_gen"]
```

## [app] and [[app]]

`[app]` is one app. `[[app]]` is a list, at most one per store. Each App campaign promotes one of them.

| Key | Type | Required | Default | Rule |
|---|---|---|---|---|
| `store` | string | yes | | `google_play` or `app_store`. |
| `id` | string | yes | | Google Play: a package name with at least one `.` and only letters, digits, `.` and `_`. App Store: digits only. |
| `optimize_for` | string | no | `installs` | `installs` or `in_app_actions`. Any other value is a parse error. |
| `conversion_action` | string | with `in_app_actions` | empty | Name of the in-app conversion action in Google Ads, trimmed. Empty with `in_app_actions` fails with the key `app.conversion_action`. A value with `installs` fails with the same key. |

Rules checked when the file loads:

- A second app on the same store fails with the key `app.store`.
- A bad `id` fails with the key `app.id`.
- An `app_installs` campaign without an app for its store fails with `E24`.
- An app that `mads init` finds on the site replaces a hand-written one of the same store. A store the site does not link keeps yours.

```toml
[[app]]
store = "google_play"
id = "com.vinellu.app"

[[app]]
store = "app_store"
id = "6751919708"
optimize_for = "in_app_actions"
conversion_action = "Sign up"
```

With `optimize_for = "in_app_actions"` the Editor file gets `In-app actions` in `Campaign optimization`, a `Conversion action` column and, when `budget.target_cpa_in_app` is set, `Target CPA`. Only `Installs` was checked against a real Editor export. The in-app names are unverified. See [Editor file](editor-file.md). The automation package does not write the goal.

## [focus]

With `[focus]` the account advertises one offer, not the whole business.

| Key | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters. |
| `urls` | string array | yes | 1 to 10 absolute `http` or `https` URLs. |
| `terms` | array of string arrays | no | At most 5 groups. Each group has 1 to 10 terms of 1 to 80 characters. |

What it enforces in a plan:

- Every final URL must be one of `urls` (`E22`). Sitelinks may go elsewhere.
- Every keyword must contain one term of each group (`E23`).
- Every URL in `urls` is also in the [allowed URLs](#allowed-urls).

`mads init --focus` writes the table.

```toml
[focus]
name = "Bus Belo Horizonte to Sao Paulo"
urls = [
  "https://www.buser.com.br/onibus/belo-horizonte-mg/sao-paulo-sp",
  "https://www.buser.com.br/onibus/sao-paulo-sp/belo-horizonte-mg",
]
terms = [["bh", "belo horizonte"], ["sp", "sao paulo"]]
```

With these terms `clickbus bh sp` passes `E23` and a bare `clickbus` fails.

## [strategy]

`mads init --strategy` writes this table and the `STRATEGY.md` it points to. See [Strategies](../explanation/strategies.md) and [Pick a strategy](../how-to/pick-a-strategy.md).

| Key | Type | Required | Rule |
|---|---|---|---|
| `name` | string | yes | 1 to 80 characters. |
| `intent` | string | yes | `purchase`, `consideration` or `discovery`. |
| `products` | string array | yes | 1 to 50 absolute `http` or `https` URLs, in priority order. |
| `min_share` | integer | yes | Percent of the non-brand budget the product campaigns get, 0 to 100. A plan below it fails with `E33`. `mads optimize` turns `E33` into warning `W14`. |
| `file` | path | no | Text for the agents. At most 8000 characters. |

Checked when the file loads:

| Check | Key |
|---|---|
| `products` has 1 to 50 absolute URLs | `strategy.products` |
| With `[focus]`, every product is one of its `urls` | `strategy.products` |
| `min_share` is 0 to 100 | `strategy.min_share` |
| `intent = "purchase"` with a non-empty `business.restricted` | `strategy.intent` |
| `file` has at most 8000 characters | `strategy.file` |
| Every product has a catalog item with the same page | `strategy.products` |

Products compare by page. A trailing `/` and a leading `www.` make no difference. The query string does (`?id=1` and `?id=2` are different products). The catalog check runs when mads reads the catalog, so a `[strategy]` without `[catalog]` fails.

```toml
[strategy]
name = "need"
intent = "purchase"
products = ["https://vinellu.com/w/alamos-malbec"]
min_share = 80
file = "STRATEGY.md"
```

## Allowed URLs

A final URL or sitelink URL is accepted only when it is in the allowed set. Anything else fails with `E07`.

The set is:

- `business.url`
- every `business.pages[].url`
- every catalog `url`
- every `focus.urls` entry

URLs compare after normalization. The scheme and host are lowercased, the default port and the fragment are removed. The path and the query stay as written, so `/a` and `/a/` differ.

## What mads init keeps with --force

`mads init --force` rewrites `business.toml` and keeps what you wrote by hand in these places.

| Kept | Where |
|---|---|
| `max_cpc`, `target_cpa`, `target_cpi`, `target_cpa_in_app` | `[budget]` |
| The whole table | `[export]`, `[campaigns]`, `[google_ads]` |
| Apps | `[app]` or `[[app]]` |

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
restricted = ["alcohol"]

[[business.pages]]
name = "Baixe o app"
url = "https://vinellu.com/app"

[budget]
daily = 50
currency = "BRL"
max_cpc = 3.0

[export]
status = "Paused"
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

## Error messages

Real output for a typo in a key. The list ends with `restricted`.

```text
error: TOML parse error at line 8, column 1
  |
8 | languge = "en"
  | ^^^^^^^
unknown field `languge`, expected one of `name`, `url`, `language`, `locations`, `goal`, `description`, `conversion_tracking`, `brand_terms`, `competitors`, `avoid`, `pages`, `restricted`
```

Value errors have the form `<key>: <message>`. Exit code `2`.

```text
business.name: must be 1 to 80 chars, got 0
business.url: must be an absolute http(s) URL
business.language: must be a BCP 47 tag such as pt-BR
business.locations: must have 1 to 10 locations
business.locations[1]: must not contain `;`
business.locations[1]: repeats another location
business.goal: must be 1 to 200 chars, got <n>
business.description: must be 20 to 4000 chars, got <n>
business.pages: at most 20 pages
budget.daily: must be greater than 0 with at most 2 decimals
budget.currency: must be 3 uppercase letters
budget.max_cpc: must be greater than 0 with at most 2 decimals
google_ads.customer_id: use 10 digits or 123-456-7890
google_ads.customer_id: 000-000-0000 is a placeholder: leave customer_id empty or set the real account id
google_ads.app_id: Google Play takes a package name such as com.example.app, the App Store a numeric id
app.store: one app per store: a second app of the same store cannot have its own campaign
app.id: Google Play takes a package name such as com.example.app, the App Store a numeric id
```

The `E` and `W` codes are in [Validation rules](validation-rules.md).

## Next

- [Write the business file](../how-to/write-the-business-file.md)
- [catalog.csv](catalog-csv.md)
- [Validation rules](validation-rules.md)
