# Tutorial

This tutorial takes you from an empty folder to Vinellu campaigns uploaded in Google Ads, paused and ready to review.

Vinellu is a social wine app. People photograph a label and see rating, vintage, food pairing and reviews. The goal is app sign-ups. The catalog is a list of wine labels people search for by name.

You need mads installed and a provider. See [Getting started](getting-started.md). The examples show a run with `anthropic`. Any provider works, including `claude-cli` (see [Agent CLIs](guides/agent-clis.md)).

The output in this tutorial is an example. Agents make different choices each run, so your campaign names, keyword counts and warnings will differ. The structure is the same.

## 1. Create the folder

```bash
mkdir vinellu-ads
cd vinellu-ads
```

## 2. Create the input files

You have two ways. Let an agent draft the files from your site, or write them by hand. The hand-written path below gives the same result and shows what every key does.

### Draft them from the site

```bash
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider anthropic --model <model-id>
```

init writes `business.toml` and `catalog.csv` into the current folder. Open both and fix what the agent got wrong. In particular check `goal`, `competitors`, the page list and the catalog notes. Then add `max_cpc = 3.0` under `[budget]`, and `avoid = ["melhor do mundo"]` under `[business]`. See [Init from a URL](guides/init-from-url.md). Go to step 3 when you are done.

### Write them by hand

#### business.toml

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

[catalog]
file = "catalog.csv"
```

What each choice does.

- `brand_terms` becomes a negative keyword in every campaign that is not the brand campaign. Ads for "alamos malbec review" should not bid on people who typed "vinellu".
- `competitors` becomes a negative in every campaign that is not the competitor campaign. It also raises warning `W01` when a competitor name shows up in ad text.
- `avoid` is a hard rule. Any ad text that contains "melhor do mundo" fails validation (`E05`).
- `business.pages` adds a URL to the allowed set. Sitelinks and landing pages can only point at allowed URLs. Pick pages that exist, because mads requests them later.
- `budget.max_cpc` is a ceiling. A CPC above 3.00 fails validation (`E11`).
- `catalog.file` is relative to the folder that holds `business.toml`.

#### catalog.csv

```csv
name,url,category,aliases,third_party,notes
Alamos Malbec,https://vinellu.com/w/Wv3T9ngB8S/malbec,malbec argentino,alamos,true,
Luigi Bosca Malbec,https://vinellu.com/w/uMm7sBLgDV/malbec,malbec argentino,luigi bosca,true,
DV Catena,https://vinellu.com/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend argentino,d.v. catena|dv catena cabernet malbec,true,
Angélica Zapata,https://vinellu.com/w/KYddWdfzzg/angelica-zapata,cabernet sauvignon,angelica zapata,true,
```

Each row is a wine label. The agents can give one ad group to each.

- `url` is the landing page for that label. It joins the allowed URL set.
- `aliases` are other ways people write the name. They separate with `|`. The agents use them as keyword variants.
- `third_party` is `true` because these labels are other companies' trademarks. mads then warns (`W01`) when a label name appears in ad text. It is a warning, not an error. Reviews of a product are a legitimate use, but Google's trademark policy can still limit the ad.

Every row gets an id from its name. `Alamos Malbec` becomes `alamos-malbec` and `Angélica Zapata` becomes `angelica-zapata`. The plan mission refers to labels by id. See [The catalog](guides/catalog.md).

## 3. Run generate

```bash
mads generate business.toml --provider anthropic --model <model-id>
```

mads checks the files first. A mistake exits with code 2 and names the key.

Then the missions run. This is an excerpt from a run with `--format plain`.

```text
12:28:50 [run] 20261001-122850-b51417 started (anthropic, <model-id>)
12:28:50 [plan] started (attempt 1)
12:28:50 [plan] > get_business
12:28:50 [plan] < ok get_business
12:28:50 [plan] > query_catalog
12:28:50 [plan] < ok query_catalog: 4 of 4 items
12:28:50 [plan] > set_brand_kit
12:28:50 [plan] < ok brand kit: 10 headlines, 3 descriptions
12:28:50 [plan] > set_account_plan
12:28:50 [plan] < ok plan: 2 campaigns, 5 ad groups
12:28:50 [plan] > finish
12:28:50 [plan] < ok plan finished
12:28:50 [plan] finished
12:28:50 [campaign:vinellu-rotulos] started (attempt 1)
12:28:50 [campaign:vinellu-marca] started (attempt 1)
12:28:50 [campaign:vinellu-rotulos] > upsert_ad_group rotulo-alamos
12:28:50 [campaign:vinellu-rotulos] < ok rotulo-alamos: 14 kw, 0 neg, rsa 15/4
12:28:50 [campaign:vinellu-marca] > set_campaign_negatives
12:28:50 [campaign:vinellu-marca] < ok 8 campaign negatives
12:28:50 [campaign:vinellu-marca] > set_assets
12:28:50 [campaign:vinellu-marca] < ok assets: 4 sitelinks, 5 callouts, 1 snippets
12:28:50 [campaign:vinellu-marca] > validate
12:28:50 [campaign:vinellu-marca] < ok validate: 0 errors, 4 warnings
12:28:50 [campaign:vinellu-marca] > finish
12:28:50 [campaign:vinellu-marca] < ok campaign finished
12:28:50 [campaign:vinellu-marca] finished
12:28:50 [run] cross-negatives: brand and competitor terms
12:28:50 [run] validate: checking limits and policies
12:28:50 [validate] 0 errors, 24 warnings
12:28:50 [run] url-check: 5 URLs
12:28:51 [url] 200 https://vinellu.com/app
12:28:52 [url] 200 https://vinellu.com/w/uMm7sBLgDV/malbec
12:28:52 [run] export: writing CSV files
12:28:52 [out] out/20261001-122850-b51417/google-ads/1-campaign.csv
12:28:52 [out] out/20261001-122850-b51417/report.md
```

Read it like this.

- `>` is a tool call. `<` is its result. `error` instead of `ok` means the tool refused the call and the agent will retry.
- `rotulo-alamos: 14 kw, 0 neg, rsa 15/4` means 14 keywords, 0 ad group negatives, and a merged ad with 15 headlines and 4 descriptions.
- `[campaign:...]` missions run at the same time, so lines interleave.
- The URL check shows one line per distinct final and sitelink URL.

When it ends you see a line like this.

```text
12:28:52 [run] done (exit 0): 104000 in, 18000 out tokens, cost n/a, 3 missions finished
```

`cost n/a` is normal. API providers report tokens, not dollars.

The run directory is `out/<run-id>/`.

```bash
ls out/*/
```

```text
events.ndjson
google-ads
input
report.md
run.json
transcripts
workspace.json
```

## 4. Read report.md

Open `out/<run-id>/report.md`. It starts with the status.

```markdown
> Ready to import. Review the files in `google-ads/` before uploading.
```

Then the summary.

```markdown
| Campaign | Intent | Daily budget | Bidding | Ad groups | Keywords |
|---|---|---|---|---|---|
| Vinellu - Marca | brand | 10,00 BRL | Manual CPC | 1 | 8 |
| Vinellu - Rotulos | catalog | 40,00 BRL | Manual CPC | 4 | 63 |
```

Check the split. Brand gets 10 of 50 (20 percent), the catalog campaign gets 40. The budgets sum to `budget.daily`, or the run would have stopped with `E06`.

Next, budget and bids.

```markdown
**Vinellu - Rotulos**: Maior intenção: quem busca um rótulo pelo nome. 80% da verba.

- rotulo-alamos: CPC 1,50 (Intenção alta, CPC estimado sem dados de leilão.)
```

These are estimates. Each ad group has a rationale. Compare the CPCs with the Keyword Planner in your account before you spend.

Then validation.

```markdown
**W01 (23)**

- `campaigns[1].ad_groups[0].rsa.headlines[0]`: third-party term 'alamos malbec' in ad text (trademark policy risk)
- and 18 more

**W04 (1)**

- `campaigns[0].ad_groups[0].rsa`: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
```

Warnings do not stop the export.

- `W01` is expected here. The ads name the labels on purpose. Read the Google Ads trademark policy for your market and decide.
- `W04` means an ad has fewer than 15 headlines. It still works. The ad has more room for variety if you add headlines.

The URL check lists each URL with its status. The usage table lists tokens per mission. Every section is explained in [Review and import](guides/review-and-import.md).

## 5. Review the CSVs

Open the files in `out/<run-id>/google-ads/`. They are plain text. Look at keywords first.

```csv
Row Type,Action,Keyword status,Campaign,Ad group,Keyword,Type
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app baixar,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app grátis,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu baixar,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu grátis,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Exact match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app,Exact match
```

The agent sent two variants (`vinellu`, `vinellu app`) and two modifiers (`baixar`, `grátis`). mads built 6 phrase keywords and 2 exact keywords.

Check negatives next. The catalog campaign carries your brand and your competitor as negatives.

```csv
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,receita,Phrase match
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,vinagre,Phrase match
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,vinellu,Phrase match
Negative keyword,Add,Enabled,Ad group,Vinellu - Rotulos,rotulo-alamos,vivino,Phrase match
```

`vinellu` and `vivino` are the cross negatives. They sit in every ad group of the campaign, because campaign-level negatives are exported expanded into each ad group for now.

Last, the ads. Open `5-responsive-search-ads.csv`. Headlines 1 to 5 are specific to the ad group. Headlines 6 to 15 are the same brand kit in every ad. Empty cells at the end mean the ad has fewer than 15 headlines.

Read every headline and description out loud. Check these things.

- Claims. Does each one come from your business description?
- Tone. Does it sound like your brand?
- Third-party names. Are you comfortable with them?
- Final URLs. Does each ad point at the right page?

If something is wrong, you can edit the CSV directly, or fix the input and run `mads generate` again.

## 6. Import into Google Ads

1. Open Google Ads and go to Tools, Bulk actions, Uploads.
2. Upload `1-campaign.csv` and preview the changes.
3. Fix anything Google flags, then apply.
4. Repeat for `2-ad-groups.csv`, `3-keywords.csv`, `4-negative-keywords.csv` and `5-responsive-search-ads.csv`, in that order.

Rows reference their parents by name, so campaigns go first and ads go last.

The campaigns arrive with status `Paused`. The ad groups, keywords and ads are `Enabled`, but nothing serves while the campaign is paused.

## 7. Check before you enable

- Open each campaign and confirm budget, location (`Brazil`), language (`pt`) and bidding (`Manual CPC`).
- Open the keyword list of one ad group. Confirm the phrase and exact keywords match what you reviewed.
- Open the negatives. Confirm `vinellu` and `vivino` are there in the non-brand campaign.
- Compare each CPC with the Keyword Planner. Raise or lower it. mads had no auction data.
- Check the Final URL suffix. It carries your UTM parameters with `{adgroupid}` and `{keyword}` filled in by Google.
- Check for policy flags in the Ads tab. Third-party trademark warnings show up here.
- Enable one campaign. Watch search terms for a few days and add negatives you find.

Sitelinks, callouts and structured snippets are not in the files yet. mads validates them but does not export them. Add them in the Google Ads interface. See [Limitations](../README.md#limitations).

## Next

* [How campaigns are built](guides/how-campaigns-are-built.md) explains why the output looks this way.
* [Resume and export](guides/resume-and-export.md) covers a run that failed halfway.
* [Cost and limits](guides/cost-and-limits.md) covers token budgets.
