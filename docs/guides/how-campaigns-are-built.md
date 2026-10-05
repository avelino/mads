# How campaigns are built

This page explains what the agents decide, what mads computes, and what the validator enforces, so you can predict and review the output.

## The flow

```text
business.toml + catalog.csv
        |
   plan mission          brand kit + 1 to 5 campaigns + planned ad groups
        |
   campaign missions     one per campaign, in parallel
        |
   post-processing       cross negatives, validation, URL check, export
```

Each mission has a system prompt, a short user prompt and a fixed set of tools. The agent works only through tools. Every tool validates its arguments before it writes state. A failed call changes nothing and returns the errors, and the agent fixes the call and tries again.

## Intents

The plan mission splits the account by search intent. There are four.

| Intent | Who searches | How it is used |
|---|---|---|
| `brand` | People typing your business name | Cheap and protective. The prompt asks for 10 to 20 percent of the budget. Only this campaign may get under 10 percent. |
| `catalog` | People typing a specific entity name plus an evaluation or purchase word | One ad group per entity or per tight group. Usually the highest intent. |
| `generic` | People searching the category or the problem without a name | Broader terms. |
| `competitor` | People typing a competitor's name | Only when `business.competitors` is not empty. Carries trademark risk, so it stays small. |

The intent follows the words people type. A search for the kind of product or the need, such as "app to scan a label", is `generic` even when it competes with a known brand. The plan agent is told never to put searches with a competitor name and searches without one in the same campaign, so the trademark risk of one does not shrink the budget of the other.

The agent skips an intent that does not fit. A business with no catalog has no `catalog` campaign. An account has 1 to 5 campaigns.

## Budget

Campaign budgets are in currency units with at most 2 decimals. They must sum exactly to `budget.daily` (`E06`). Each campaign needs at least 1.00. mads computes with integer cents, so there is no rounding drift. The agent explains each share in the campaign `rationale`, and it shows in `report.md`.

## Ad groups

Ad groups are tightly themed. One entity, or a few entities that share the same searches. The plan mission lists them (name, theme, entity ids, final URL). The campaign mission must build every planned ad group and nothing else (`E12`).

The final URL resolves at plan time.

- A non-empty `final_url` must be in the allowed set (`E07`).
- An empty one resolves to the entity's URL when the ad group has exactly one entity.
- Otherwise it resolves to `business.url`.

## Keywords

The agent does not write the keyword list. It sends a spec.

| Field | Limit | Meaning |
|---|---|---|
| `variants` | 1 to 6 | Ways people write one name. Different things go in different ad groups (`W08`). |
| `modifiers` | 0 to 10 | Intent words appended to each variant. |
| `exact_heads` | default `true` | Also add each variant as an exact match keyword. |
| `extra` | 0 to 20 | Explicit keywords for cases the spec cannot express. |

mads expands it.

1. Normalize every variant and modifier: lowercase, trim, collapse spaces. Drop empties and duplicates.
2. Phrase keywords are every variant plus every `variant modifier` pair. Sort them by byte order and remove duplicates. Outside brand campaigns, a one-word variant gets no phrase keyword when `exact_heads` is true. Alone in phrase match it catches any search with that word: in a live account, `"malbec"` took 43% of the Search spend on people looking for a bottle to buy.
3. If `exact_heads` is true, add each variant again as an exact match keyword, in the order the agent gave them.
4. Add `extra` keywords that are not already present with the same text and match type.

Example, in a brand campaign. Variants `vinellu` and `vinellu app` with modifiers `baixar` and `grátis` give 6 phrase keywords and 2 exact keywords.

```csv
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app baixar,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app grátis,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu baixar,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu grátis,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Exact match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app,Exact match
```

Only phrase and exact match exist. Broad match is out of scope. An ad group holds 1 to 50 keywords (`E13`).

Keyword text has at most 80 characters and 10 words. It cannot contain any of ``! @ % ^ * ( ) = { } ; ~ ` < > ? \ | , [ ] "`` (`E08`).

## Negatives

Negatives stop wasted clicks. There are three sources.

- **Ad group negatives.** The agent passes them in `upsert_ad_group`.
- **Campaign negatives.** The agent sets them once per campaign with `set_campaign_negatives`. The prompt asks for the shared ones there and never repeated per ad group. Typical themes are job seekers, wholesale and B2B, free or pirated when the offer is paid, recipes and DIY, and other meanings of the names.
- **Cross negatives.** mads adds them after all missions end.

### Cross negatives

Brand and competitor names should not trigger the wrong campaign. After the missions, mads does this.

- For every campaign whose intent is not `brand`, add each `brand_terms` entry as a campaign-level phrase negative.
- For every campaign whose intent is not `competitor`, add each `competitors` entry the same way.
- Skip a term that would block one of the campaign's own keywords, and record a note under Notes in the Validation section of `report.md`.
- Skip a term already present.

```text
skipped negative 'vinellu' in campaign 'generic': it would block keyword 'vinellu app'
```

Running the step twice gives the same account.

### The rule that protects your keywords

A negative must never block a keyword in its scope (`E09`). A phrase negative blocks a keyword when its words appear contiguously in the keyword's words. An exact negative blocks a keyword with equal text. Campaign negatives apply to every ad group in the campaign. Ad group negatives apply to their own ad group.

The tools check this on every write, so an agent cannot add a negative that kills a keyword it built.

### Export shape

Campaign-level negatives are exported expanded into every ad group (Level `Ad group`), campaign negatives first, then the ad group's own, without duplicates. Campaign-level rows need a Google bulk template that is not verified yet. See [the bulk upload format](../howto/google-ads-bulk-upload-format.md).

## Ads

A responsive search ad has up to 15 headlines and 4 descriptions. mads builds each ad from two parts.

- **Brand kit.** Set once by the plan mission: 8 to 12 headlines and 2 to 3 descriptions. They cover the value proposition, social proof, a call to action and the brand. Every ad shares them.
- **Specific texts.** Set per ad group: 3 to 7 headlines and 1 to 2 descriptions. They use the entity name in 2 or 3 variants with different angles, such as a question, a benefit or proof.

The final ad is computed at export, never stored.

```text
headlines    = unique(specific headlines + brand kit headlines).take(15)
descriptions = unique(specific descriptions + brand kit descriptions).take(4)
```

Uniqueness uses normalized text. If a specific text equals a brand kit text, the specific one stays and the brand kit copy drops. Duplicates inside the specific list or inside the brand kit are an error (`E03`), because they point to an agent mistake.

In the reference export, headlines 6 to 15 and descriptions 2 to 4 are the same in every ad, in the same order. That is the brand kit at work.

Limits: headline 30 characters, description 90, path 15. No `!` in headlines (`E04`). `path2` needs `path1` (`E14`). Descriptions end with a call to action, and that part is a prompt rule, not a validator rule.

A merged ad with fewer than 15 headlines or fewer than 4 descriptions raises `W04`. It still exports.

## Assets

Each campaign sets sitelinks, callouts and structured snippets with `set_assets`.

| Asset | Count | Text limit |
|---|---|---|
| Sitelinks | 2 to 8 (4 or more recommended) | text 25, descriptions 35 each, both or none |
| Callouts | 2 to 10 (4 or more recommended) | 25 |
| Structured snippets | 0 to 2, each with 3 to 10 values | 25 per value |

Sitelink URLs must be in the allowed set. A campaign mission cannot finish until assets are set. Fewer than 4 sitelinks, fewer than 4 callouts or no snippet raises `W05`.

Assets are collected and validated but not exported yet. Files 6 to 8 are not written until their Google templates are verified.

## Focus

With `[focus]` in `business.toml` the account advertises one offer. The plan keeps every campaign on it, brand and competitor campaigns combine their names with the focus words, and every landing page must be a focus URL (`E22`). A planned group without entity lands on the first focus URL.

## Formats

Each campaign has a `kind`: `search` (the default), `performance_max` or `demand_gen`. The plan agent picks it per campaign. Search catches demand that already exists. Demand Gen creates demand in YouTube, Discover and Gmail feeds. Performance Max needs conversion tracking. The image formats are open only when the run has an image model and `business.toml` has a logo. An image campaign gets a plan only when at least 20 percent of the budget is left after the best Search campaigns. `[campaigns] formats` in `business.toml` makes formats mandatory. [Image campaigns](image-campaigns.md) covers what they build and how they export.

## Bids

Search campaigns take only `manual_cpc`. The plan tool refuses `maximize_clicks` and `maximize_conversions` on Search.

```json
{"code": "UNSUPPORTED", "path": "campaigns[0].bid_strategy.type", "message": "search campaigns take only manual_cpc until the Google Ads bulk templates are verified"}
```

Performance Max takes `maximize_conversions`. Demand Gen takes `maximize_clicks` or `maximize_conversions`. Any other pair is `E17`.

Each ad group gets a `default_cpc` and a `cpc_rationale`. These are estimates. mads has no auction data, no Keyword Planner volumes and no history. A CPC under the first-page bid gets no impressions, so the prompt asks for what searches of that kind cost, below `budget.max_cpc` when set. Brand and competitor groups of a little-known brand get a CPC close to the generic ones: in a live account, brand groups at 0.60 BRL showed zero times while generic groups at 1.00 to 1.20 BRL served. `E11` rejects a zero CPC and any CPC above `max_cpc`.

The report labels budget shares and CPCs as estimates. Check them before you enable anything.

## What the validator enforces and why

The full table is in [Validation rules](../reference/validation-rules.md). The reasons fall into five groups.

- **Google's limits.** Text lengths, counts and keyword syntax (`E01`, `E02`, `E08`, `E13`, `E14`). Google rejects these at upload.
- **Your instructions.** `business.avoid` terms (`E05`), the budget sum and the `max_cpc` ceiling (`E06`, `E11`), `!` in headlines (`E04`).
- **Hallucination guards.** URLs outside your site (`E07`) and URLs that do not answer 2xx (`E15`). A made-up URL returns 404 and gets the ad disapproved.
- **Self-inflicted damage.** Negatives that block your keywords (`E09`), duplicate names (`E12`), duplicate texts (`E03`).
- **Policy and quality warnings.** Third-party terms (`W01`), all-caps words (`W02`), the same keyword in two ad groups (`W03`), short ads (`W04`), thin assets (`W05`).

Errors block the run. A tool call with errors returns them to the agent and changes nothing. At the end, any error left in the account stops the export with exit code `3`. Warnings go to the report and, in CI, to annotations.
