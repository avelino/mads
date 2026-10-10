# Validation rules

This page lists every validation code mads emits, what it checks and where.

Errors block the export. Warnings go to the report, to progress output and, on GitHub Actions, to annotations. Each issue has a `code`, a `path` into the account, for example `campaigns[0].ad_groups[3].rsa.headlines[2]`, and a `message`.

## Where rules run

- **During a mission.** Every tool checks its input before it changes state. A failing call returns the errors to the agent and changes nothing.
- **At the end.** mads validates the whole account again. Any error means exit code `3` and no CSV.
- **URL check.** `E15` runs only in the end step, and only without `--skip-url-check`.

## How text is measured

- Length is the number of Unicode scalar values after NFC normalization, on the trimmed text. Google counts some CJK characters as 2. mads targets Latin scripts and ignores that.
- Comparisons (duplicates, avoid terms, negatives) use `normalize`. NFC, lowercase, trim and collapse inner whitespace to one space.

## Errors

| Code | Rule | Notes |
|---|---|---|
| `E01` | Text longer than its limit. | Headline 30, description 90, path 15, sitelink text 25, sitelink description 35, callout 25, snippet value 25, campaign name 255, ad group name 255, keyword 80. Message `<n> chars, limit is <limit>`. |
| `E02` | Required text is empty after trim. | Message `text is empty`. Same fields as `E01`. |
| `E03` | Duplicate text inside one list. | Brand kit headlines and descriptions, specific RSA headlines and descriptions, sitelink texts, callouts, snippet values. Compared after `normalize`. Message `duplicate of [<index>]`. |
| `E04` | `!` in a headline. | Brand kit and specific headlines. Message `headlines cannot contain '!'`. |
| `E05` | Ad or asset text contains a `business.avoid` term. | Substring match after `normalize`. Checks headlines, descriptions, sitelink text and descriptions, callouts and snippet values. Not keywords or paths. Message `text contains avoided term '<term>'`. |
| `E06` | Budgets. | Campaign budgets do not sum exactly to `budget.daily`, a campaign budget is below 1.00, or a budget is not greater than 0 with at most 2 decimals. |
| `E07` | URL outside the allowed set. | Planned final URLs, ad group final URLs and sitelink URLs. The set is `business.url`, `business.pages` and the catalog. Message `URL not in business.url, business.pages or catalog: <url>`. |
| `E08` | Bad keyword syntax. | More than 10 words, or any of ``! @ % ^ * ( ) = { } ; ~ ` < > ? \ \| , [ ] "``. Applies to keywords and negatives. |
| `E09` | A negative blocks a keyword in its scope. | A phrase negative blocks a keyword when its words appear contiguously in the keyword's words. An exact negative blocks a keyword with equal text. Ad group negatives apply to their ad group. Campaign negatives apply to every ad group in the campaign. Message `negative '<n>' blocks keyword '<k>'`. |
| `E10` | `maximize_conversions` while `business.conversion_tracking` is `false`. | Reached by Performance Max and Demand Gen campaigns. Search takes only `manual_cpc`. App campaigns are exempt: the store counts the installs. |
| `E11` | CPC problems. | A CPC of zero, an ad group CPC that is not greater than 0 with at most 2 decimals, or a CPC above `budget.max_cpc` when set. |
| `E12` | Structure. | Duplicate campaign name in the account. Duplicate ad group name in a campaign. An ad group not in the plan. A planned ad group missing at finish. An ad group without keywords. A missing brand kit or missing campaign assets at finish. An unknown catalog id in `entity_ids`. A campaign with no planned ad groups. A name with no letters or digits, or two names that give the same slug. |
| `E13` | Count out of range. | See the table below. |
| `E14` | `path2` without `path1`, or a sitelink with exactly one description. | |
| `E15` | URL check failed. | A final or sitelink URL did not answer 2xx after redirects. Message `URL check failed: HTTP <status>` or `URL check failed: unreachable`. The path is the URL. |
| `E16` | Image briefs of an asset group. | Performance Max needs at least 1 landscape and 1 square picture and takes no vertical one. Demand Gen needs at least 1 landscape or square picture. An image `id` must be a slug and unique in the group. A prompt has 20 to 1500 characters. At finish, every asset group needs briefs. |
| `E17` | The campaign does not fit its `kind`. | Search takes `manual_cpc`, Performance Max `maximize_conversions`, Demand Gen `maximize_clicks` or `maximize_conversions`. A Search campaign with asset groups, or an image campaign with ad groups, negatives or assets. Message `<strategy> does not fit a <kind> campaign` or `a <kind> campaign has no <part>`. |
| `E18` | Image campaign without a logo. | Message `image campaigns need a logo: set [brand] logo in business.toml`. |
| `E19` | `reference` names an unknown catalog item or one without `image`. | Message `unknown catalog id '<id>'` or `catalog item '<id>' has no image to use as reference`. |
| `E22` | A landing page outside `[focus]`. | Planned group, ad group and asset group final URLs must be one of `focus.urls`. Sitelinks may go elsewhere. In init, a catalog item outside the focus, or `--focus` without a focus that holds the start URL. Message `final URL is not a [focus] page: <url>`. |
| `E23` | A keyword or search theme without the focus words. | With `focus.terms`, every keyword and Performance Max search theme needs one term of each group. Compared without accents or punctuation, so `sao paulo` matches `São Paulo`. Message `'<text>' is not about the focus: add a word like <terms>`. In init, `--focus` without `focus.terms`. |
| `E24` | App campaign without the app. | Message `app campaigns need the app: set [app] store and id in business.toml`. An App campaign needs no logo and its ads link to the store page, so `E07`, `E18` and `E22` do not apply to it. |
| `E21` | A format from `[campaigns] formats` has no campaign. | Message `business.toml asks for a <kind> campaign and the plan has none`. Path `campaigns`. |
| `E20` | A brief has no picture at the end. | The image step could not make it: a model error, `mads export` without the file, or a brief over `--max-images` that its asset group cannot do without. A brief over the cap that the group can do without is removed instead, with a note in the report. Message `image '<id>' has no file`. Agents never see it: their tools skip it, because pictures are made after the missions. |
| `E25` | A group that had impressions is missing from the plan of a campaign that continues. | Only in `mads optimize`. Editor matches by name, so a renamed group comes in as a new one, and Google measures [Quality Score per keyword from its past impressions](https://support.google.com/google-ads/answer/6167118?hl=en). A line `Drop <group>: <reason>` in the campaign `rationale` lets it go. Message `'<group>' had impressions in Google Ads and is not in the plan: keep its name, or add a line `Drop <group>: <reason>` to the rationale (a renamed group comes in as a new one, its keywords without Quality Score data)`. Path `campaigns[<i>].ad_groups`. |
| `E27` | A stalled or untracked campaign gets more daily budget than it ran with. | Only in `mads optimize`. Stalled: enabled with no ads, or 7 days without one impression. Untracked: 100 clicks or more and no conversion while another campaign converts, capped at its average daily cost rounded up when that is lower than its budget. Cutting is allowed. Message `'<campaign>' is <stalled or untracked>: <why>: keep its daily budget at <budget> or lower, and give the rest to a campaign that converts`. Path `campaigns[<i>].daily_budget`. |
| `E28` | An ad group of an untracked campaign bids higher than it ran. | Only in `mads optimize`, checked by `upsert_ad_group`. A higher bid buys more clicks that convert into nothing measurable. Message `this campaign is untracked: its clicks bring no conversion anyone can see, so a higher bid buys more of them. Keep default_cpc at <cpc> or lower`. Path `default_cpc`. |
| `E26` | Keyword spec `variants` look like different searches. | Checked by `upsert_ad_group`, not stored. Two variants belong together when, without accents or punctuation, they share a word of 3 or more letters, one holds the other (4 or more letters), they start with the same 4 letters, or one is the initials of the other. A synonym goes in `extra`. Message `variants look like <n> different searches: keep the spellings of one search in variants, put a synonym in extra, and give another search its own ad group`. |

### E13 limits

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
| Performance Max headlines (30 chars, no `!`) | 3 to 15 |
| Performance Max long headlines (90 chars) | 1 to 5 |
| Performance Max descriptions (90 chars, one of them 60 or fewer) | 2 to 5 |
| Performance Max search themes (80 chars) | 0 to 25 |
| Demand Gen headlines (40 chars) | 1 to 5 |
| Demand Gen descriptions (90 chars) | 1 to 5 |
| Demand Gen long headlines and search themes | none |
| Pictures per asset group | 0 to 20 |

The asset group `business_name` has at most 25 characters (`E01`).

## Warnings

| Code | Rule | Notes |
|---|---|---|
| `W01` | Ad or asset text contains a third-party term. | The terms are every `business.competitors` entry and the name and aliases of every catalog item with `third_party = true`. The match is on whole words, in order, after `normalize`. Trademark policy risk. Message `third-party term '<term>' in ad text (trademark policy risk)`. |
| `W02` | A word of 4 or more letters fully in uppercase that is not part of a brand term. | Words split on any non-letter. Message `word in all caps`. |
| `W03` | The same keyword (text and match type) in two ad groups of the same campaign. | Message `keyword '<k>' is also in ad_groups[<index>]`. |
| `W04` | Merged RSA with fewer than 15 headlines or fewer than 4 descriptions. | Message `merged ad has <h> headlines and <d> descriptions (15 and 4 recommended)`. |
| `W05` | Fewer than 4 sitelinks, fewer than 4 callouts or no structured snippet. | Message `recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet`. |
| `W06` | Fewer pictures than Google recommends for ad strength. | Performance Max 4 landscape, 4 square and 2 portrait. Demand Gen 1 landscape, 1 square and 1 portrait. Message `recommended for ad strength: <missing>`. |
| `W07` | A prompt asks for text in the picture, or shows an object that carries writing. | The words `text`, `logo`, `caption`, `headline`, `words`, `lettering`, `typography`, `slogan` or `label that reads`: Google adds the ad text itself. Or `menu`, `wine list`, `sign`, `signboard`, `billboard`, `poster`, `book`, `newspaper`, `magazine`, `screen`, `monitor`, `packaging`, `price tag`, `ticket`: the model fills them with invented text, so show them from the side, closed or out of focus. |
| `W09` | A one-word phrase keyword outside a brand campaign. | It matches any search with that word. mads no longer builds one from `variants`, so it comes from `extra`. Message `'<keyword>' as phrase matches any search with that word: make it exact or add words`. |
| `W10` | An ad group that ran is rebuilt without some of its keywords, and the keywords report is missing. | Only in `mads optimize`. The tool puts the keywords back: without numbers there is no reason to drop them, and one run dropped the keyword with 43% of the Search cost. Message `kept <n> keywords that ran, no keywords report to judge them: <examples>`. Path `keywords`. |
| `W11` | `--layout drive-folders` and `google_ads.customer_id` is empty or `000-000-0000`. | Does not block the export. The Customer ID column is left blank. mads never writes `000-000-0000`. Bulk export does not emit this warning. Path `google_ads.customer_id`. Message `Customer ID is empty or the 000-000-0000 placeholder, so the drive files leave that column blank. Set google_ads.customer_id to the real account id before import.` |
| `W12` | `--layout drive-folders` and a campaign that is not Search. | Does not block the export. The drive files hold Search only, so the campaign is in none of them, B1 and B2 included. Path `campaigns[<index>]`. Message `<type> campaign '<name>' is not in the drive files, which hold Search only. Export it with --layout bulk.` |

## Tool-only codes

These come from the tool layer, not the rule set. The agent sees them in tool results.

| Code | Meaning |
|---|---|
| `ARGS` | The arguments did not parse. Unknown field, missing field or wrong type. The message is the parser error. |
| `UNKNOWN_TOOL` | The tool does not exist or is not in this mission's toolset. |
| `LIMIT` | The mission used its tool call budget (`--max-turns` times 4). |
| `UNSUPPORTED` | A Search campaign with a bid strategy other than `manual_cpc`, or an image campaign in a run without an image model or a logo. |
| `NO_IMAGE_REASON` | Image campaigns are available, the plan has none, and no campaign `rationale` contains `No image campaign: <reason>`. Not asked when `[campaigns] formats` is set. |
| `NO_APP_REASON` | `[app]` and an image model are there, the plan has no `app_installs` campaign, and no `rationale` contains `No app campaign: <reason>`. Not asked when `[campaigns] formats` is set. |
| `LIVE_DETAIL` | In `mads optimize`, `upsert_ad_group` or `upsert_asset_group` for a group that ran, before this mission called `get_ad_group_performance` with it. |
| `NOT_FOUND` | The campaign of this mission no longer exists in the plan. |
| `PERSIST` | The workspace could not be saved to disk. |
| `HOST`, `FETCH` | Init only. The URL is outside the site, or the request failed. |
| `INPUT` | Init only. The draft does not parse like `business.toml` or `catalog.csv`. |
| `EXPORT` | The final export failed. Seen in the end step, for example when the brand kit or the logo is missing. |

## Cross negatives

The end step adds brand and competitor terms as campaign negatives before validation. Image campaigns are skipped. A term that would trigger `E09` in a campaign is skipped and reported as a note in the Validation section of `report.md`.
