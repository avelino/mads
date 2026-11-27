# Write the business file

Write or fix `business.toml` so the agents produce ads you can use.

The agents read this file through the `get_business` and `get_brief` tools. They write every headline from it and are told never to invent facts, so a thin file gives thin ads. The steps cover the keys that change the ads. Types, defaults and limits of every key are in the [business.toml reference](../reference/business-toml.md).

## Before you start

- A Google Ads account decision: its currency and whether it records conversions. See [Prepare Google Ads](prepare-google-ads.md).
- Optional: a draft. `mads init --from-url` writes a `business.toml` from your site, and you fix it with this page open. See [Draft the files from your site](draft-from-your-site.md).
- Optional: a catalog for named products, routes or places. See [Write a catalog](write-a-catalog.md).

## 1. Start from the minimal file

Only `[business]` and `[budget]` are required. Every other key has a default.

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

An unknown key fails the load, so a typo cannot silently do nothing.

## 2. Write the description as claims you can defend

The agents copy numbers, prices and offers from `description` and from your catalog notes. Nothing else counts as a fact for them.

- Put facts in. "Mais de 170 mil rótulos" can reach a headline. "O maior app de vinhos" cannot, unless you wrote it.
- State the offer: free, trial, delivery, price range.
- Say who the customer is and what they search for.
- Leave out claims you cannot defend. A tool rejects text with an avoided term, but nothing checks a claim for truth.

The limit is 20 to 4000 characters.

## 3. Say what a conversion is in `goal`

Keep `goal` to one short line. It tells the agents which intent words fit. "cadastros no app" points them to words like `baixar` and `grátis`. "venda de vinhos" points them to `comprar` and `preço`.

## 4. Set the language and the locations

`language` is a BCP 47 tag such as `pt-BR` or `en-US`. The agents write all ad text in it.

`locations` takes 1 to 10 entries, the places where your buyers are. Every campaign targets all of them. mads checks the length, repeats and the `;` character. It does not check a name against Google's list, so a wrong place name surfaces when you upload.

## 5. List every spelling of your brand

`brand_terms` defaults to the lowercase business name. mads adds each term as a phrase negative to every non-image campaign that is not the brand campaign, so people who already know you do not eat the budget of the generic campaigns. A term that would block one of a campaign's own keywords is skipped, and the run notes it.

```toml
brand_terms = ["vinellu", "vinelu"]
```

Add the misspellings people type. A word of a brand term in all caps does not raise `W02`.

## 6. Name competitors only if you want that campaign

`competitors` has these effects.

- It lets the plan agent create a `competitor` campaign. With an empty list it does not.
- It adds each name as a phrase negative to every other non-image campaign.
- It raises `W01` when a name appears in ad text.

A competitor campaign carries trademark risk. The plan prompt tells the agent to keep it small. Leave the list empty if you do not want one.

## 7. Block words with `avoid`

`avoid` is a hard rule. A headline, description, sitelink, callout or snippet value that contains an entry fails with `E05`. The match ignores case and extra whitespace.

```toml
avoid = ["melhor do mundo", "garantido"]
```

## 8. List the pages ads may point to

Every final URL must come from the allowed set, or it fails with `E07`. This stops an agent from inventing a URL that returns 404. The set is `business.url`, every page you list, every catalog URL and every `[focus]` URL. See [allowed URLs](../reference/business-toml.md#allowed-urls).

```toml
[[business.pages]]
name = "Baixe o app"
url = "https://vinellu.com/app"
```

Add at most 20. The campaign prompt asks the agent for at least 4 sitelinks to distinct allowed pages. The validator accepts 2 to 8 and warns with `W05` below 4. A small site that lists no pages gets fewer sitelinks, so list your key pages.

## 9. Set `conversion_tracking` to match the account

It is `false` by default. Set it to `true` only when your Google Ads account records conversions. Search campaigns bid with `manual_cpc` and ignore it. The plan agent plans Performance Max only with tracking on, and `maximize_conversions` without it fails with `E10`.

## 10. Set the budget

```toml
[budget]
daily = 50
currency = "BRL"
max_cpc = 3.0
```

- `daily` is the account budget. The campaign budgets of a plan must add up to it (`E06`).
- `currency` is for the report. It is not written to any CSV, because Google Ads uses your account currency.
- `max_cpc` is optional. A CPC above it fails with `E11`.
- `target_cpa`, `target_cpi` and `target_cpa_in_app` are optional. `mads optimize` holds campaigns to them. See [Optimize a live account](optimize-a-live-account.md).

## 11. Add the optional tables only when you need them

These have their own pages.

| You want | Table | Read |
|---|---|---|
| Image campaigns, logo, formats | `[brand]`, `[campaigns]` | [Add image campaigns](add-image-campaigns.md) |
| A catalog | `[catalog]` | [Write a catalog](write-a-catalog.md) |
| Alcohol, gambling and other restricted subjects | `restricted` in `[business]` | [Handle restricted categories](handle-restricted-categories.md) |
| Status, URL suffix, number format | `[export]` | [business.toml reference](../reference/business-toml.md#export) |
| The ads automation package | `[google_ads]` | [Ads automation package](../reference/automation-package.md) |

## Check it worked

`mads generate` loads and validates the file before it creates a run directory and before any model call. A bad file prints the key and exits with code `2`.

Example run (replay provider, no model was called). The file had `languge = "pt-BR"` on line 2 of `[business]`:

```text
error: TOML parse error at line 2, column 1
  |
2 | languge = "pt-BR"
  | ^^^^^^^
unknown field `languge`, expected one of `name`, `url`, `language`, `locations`, `goal`, `description`, `conversion_tracking`, `brand_terms`, `competitors`, `avoid`, `pages`, `restricted`
```

A value error names the key and the limit. Same setup, with `description = "x"`:

```text
error: business.description: must be 20 to 4000 chars, got 1
```

A file that loads goes straight on to the agents, and that step calls your model and spends your tokens. The plan log shows the agent calling `get_business` to read it.

Example run (replay provider, no model was called, URL check skipped with `--skip-url-check`):

```text
...
10:21:17 [plan] > get_business
10:21:17 [plan] < ok get_business
```

If a limit message is unclear, find the key in the [business.toml reference](../reference/business-toml.md) or the code in [Validation rules](../reference/validation-rules.md).

## Next

- [Write a catalog](write-a-catalog.md) for products, routes or places that deserve their own ad group.
- [Choose a provider](choose-a-provider.md) before the first real run.
