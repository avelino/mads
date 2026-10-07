# The business file

This page shows how to write `business.toml` so the agents produce ads you can use. For the exact type and limit of every key, see the [business.toml reference](../reference/business-toml.md).

Starting from a site? `mads init --from-url` drafts this file for you. See [Init from a URL](init-from-url.md). Review it with this page open.

## Why it matters

The agents see this file through the `get_business` and `get_brief` tools. They write every headline, pick every keyword modifier and split the budget from what is in it. They are told never to invent facts. Numbers, prices and claims must come from your `description` or your catalog notes. A thin description gives thin ads.

## Minimal file

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

[budget]
daily = 50
currency = "BRL"
```

Everything else has a default.

## The keys that change the ads

### description

Write what you want claimed. The agents copy numbers and offers from here.

- Put facts in. "Mais de 170 mil rótulos" can appear in a headline. "O maior app de vinhos" cannot, unless you wrote it.
- State the offer. Free, trial, delivery, price range.
- Say who the customer is and what they search for.
- Leave out claims you cannot defend. A tool rejects text with an avoided term, but nothing checks a claim for truth.

The limit is 20 to 4000 characters.

### goal

One short line. It tells the agents what a conversion is and which intent words fit. "cadastros no app" pushes them toward words like `baixar` and `grátis`. A goal of "venda de vinhos" pushes them toward `comprar` and `preço`. Limit 1 to 200 characters.

### language

A BCP 47 tag such as `pt-BR` or `en-US`. Agents write all ad text in it. The CSV `Language` column gets the primary subtag, so `pt-BR` becomes `pt`.

### locations

Exactly one entry in the current version. Use the location name Google Ads shows, such as `Brazil`. mads does not check the name against Google's list. A wrong name fails at upload.

### brand_terms

Your brand names. mads adds each one as a phrase negative to every campaign that is not the brand campaign. If you leave the key out, it defaults to the lowercase business name.

Add every spelling people use.

```toml
brand_terms = ["vinellu", "vinelu"]
```

Warning `W02` (a word with 4 or more letters in all caps) skips brand words, so `VINELLU` in a headline does not trigger it.

### competitors

Names of competitors. They do three things.

- They enable a `competitor` campaign. The plan prompt only allows one when this list is not empty.
- They become phrase negatives in every campaign that is not the competitor campaign.
- They raise `W01` when one appears in ad text.

Competitor campaigns carry trademark risk. The plan prompt tells the agent to keep them small.

### avoid

Terms that must never appear in ad text. This is a hard rule. Any headline, description, sitelink, callout or snippet value that contains one fails with `E05`. The match is a case-insensitive substring match after whitespace is collapsed.

```toml
avoid = ["melhor do mundo", "garantido"]
```

### pages

Pages that sitelinks and landing pages may point at. Each entry has a `name` and a `url`. At most 20.

```toml
[[business.pages]]
name = "Baixe o app"
url = "https://vinellu.com/app"
```

Every final URL must belong to the allowed set. The allowed set is `business.url`, every page and every catalog URL. Anything else fails with `E07`. This stops the agent from inventing a URL that returns 404 and gets the ad disapproved.

Agents need at least 4 sitelinks to distinct allowed pages. With a small site, list your key pages here.

### conversion_tracking

Set `true` only when your Google Ads account tracks conversions. It is `false` by default. Search campaigns bid with `manual_cpc`, so they ignore it. Performance Max needs it: the plan agent uses Performance Max only with tracking, and `maximize_conversions` without it fails with `E10`.

## Logo and formats

Image campaigns need `[brand] logo`, a square PNG or JPEG. `mads init` downloads one from your site. `[campaigns] formats` makes formats mandatory instead of leaving the choice to the agent.

```toml
[brand]
logo = "brand/logo.png"

[campaigns]
formats = ["search", "demand_gen"]
```

See [Image campaigns](image-campaigns.md) and the [business.toml reference](../reference/business-toml.md#campaigns).

## Budget

```toml
[budget]
daily = 50
currency = "BRL"
max_cpc = 3.0
```

- `daily` is the account budget. The campaign budgets must add up to it exactly, at most 2 decimals. A campaign needs at least 1.00.
- `currency` is three uppercase letters. It is for the report only. mads does not write it to the CSV, because Google Ads uses your account currency.
- `max_cpc` is optional. When set, any CPC above it fails with `E11`.

## Export

```toml
[export]
status = "Paused"
url_suffix = "utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}"
eu_political_ads = false
decimal_comma = true
```

- `status` is `Paused` or `Enabled` (exact case) and applies to campaigns. Keep `Paused`.
- `url_suffix` is written to every campaign. `{mads_campaign}` becomes the campaign slug, for example `vinellu-marca`. Any other `{...}` is a Google ValueTrack parameter and stays as written.
- `eu_political_ads` writes `Yes` or `No`.
- `decimal_comma` controls how CPCs and budgets print (`1,50` or `1.50`). It defaults to `true` for the languages `pt`, `es`, `fr`, `de` and `it`, and `false` otherwise. Set it to match the number format of the Google Ads account you upload to. The drive-folder files always use a dot.

## Google Ads account details

```toml
[google_ads]
customer_id = "123-456-7890"
devices = "Mobile;Desktop;Tablet"
labels = ["estrutural"]
app_id = "com.vinellu.app"
tracking_template = "{lpurl}?utm_campaign={mads_campaign}&utm_term={keyword}"
```

Leave this table out unless you export with `--layout drive-folders`. An empty `customer_id` is a warning, not an error. Do not paste `000-000-0000`. See [Drive folder layout](../howto/google-ads-editor-drive-folders.md).

## Catalog

```toml
[catalog]
file = "catalog.csv"
```

The path is relative to the folder that holds `business.toml`. See [The catalog](catalog.md).

## Mistakes mads catches

Unknown keys fail, so a typo does not silently do nothing.

```text
error: TOML parse error at line 4, column 1
  |
4 | languge = "en"
  | ^^^^^^^
unknown field `languge`, expected one of `name`, `url`, `language`, `locations`, `goal`, `description`, `conversion_tracking`, `brand_terms`, `competitors`, `avoid`, `pages`
```

Value errors name the key.

```text
error: business.locations: exactly 1 location is supported in v1
error: budget.currency: must be 3 uppercase letters
error: business.url: must be an absolute http(s) URL
error: business.description: must be 20 to 4000 chars, got 5
```

All of these exit with code 2.
