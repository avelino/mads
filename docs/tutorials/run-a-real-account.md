# Run a real account

Go from a Google Ads account with billing to two optimize cycles on a live account, then repeat the loop on your own.

The example business is Vinellu, a social wine app. Its goal is app sign-ups and it has a catalog of wine labels. Your names, numbers and warnings will differ. The structure is the same. An unknown term is in the [Glossary](../reference/glossary.md).

The loop has six moves.

```text
init -> generate -> import -> wait -> optimize -> import again
```

mads never uploads anything. You import every file yourself, and every campaign arrives paused. [Safe on a live account](../explanation/safe-on-a-live-account.md) explains why that holds.

## About the samples

Every sample below is real output of the `mads` binary. It comes from an offline run with the hidden `replay` provider, which plays back a script. No model was called. Each sample says so.

Two samples had to differ from your run.

- The `init` sample read a copy of a site served on `http://127.0.0.1:8765`, so its URLs point there.
- The `generate` and `optimize` samples used `--skip-url-check`, so the URLs of the example business were not requested. Leave that flag out on a real run. Without it mads requests every final URL and a URL that does not answer blocks the export.

Three steps call a model with your provider: 2, 4 and 10. They spend your tokens. `--max-tokens` caps a run, and [Keep token cost down](../how-to/keep-cost-down.md) covers the limits.

## 0. Before you start

You need a Google Ads account that can take money and measure sign-ups, and Google Ads Editor on your computer.

1. Open the account and check billing. Google says that if you skip the payment prompt, "Your ad won't be able to show until you've done this." To add a method, open Settings from the Billing menu, then select Add a payment method. See [Add a new payment method in Google Ads](https://support.google.com/google-ads/answer/2375375).
2. Note the account currency and time zone. Google sets both when the account is created and does not let you change them. See [About your language, number format, time zone, and currency settings](https://support.google.com/google-ads/answer/9842104). The currency you give mads must be the one of the account.
3. Set up a conversion action for the result you want, here a sign-up. Then open Goals, then Summary, set the Conversions drop-down to All conversions and read the Tracking status column. Recording means the page records conversions. Unverified means it does not yet, so hover over the status and select Troubleshoot. See [Use Tag Assistant to troubleshoot your unverified or inactive conversion actions](https://support.google.com/google-ads/answer/10989978?hl=en).
4. Install [Google Ads Editor](https://support.google.com/google-ads/answer/2484521) and sign in to the account.

Tracking is not an extra. `mads optimize` judges campaigns by conversions, and it refuses a higher budget or bid for a campaign that clicks without any. [Prepare your Google Ads account](../how-to/prepare-google-ads.md) has the details.

You should see a conversion action in Goals, then Summary, with the Tracking status Recording. If it is not recording yet, you can still go on, but leave `conversion_tracking = false` and do not enable a campaign until it records. This step has no mads output.

## 1. Install and pick a provider

Install mads with the path in [Install mads](../how-to/install.md). With Docker, define an alias first, so the commands below work as written. [Run in Docker](../how-to/run-in-docker.md) explains each flag.

```bash
alias mads='docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -e ANTHROPIC_API_KEY ghcr.io/avelino/mads:edge'
```

Forward the key of your provider with `-e`. Keep `business.toml`, the reports folder and any `--app-events` file inside the folder you mount, because the container sees nothing else. With a source install, run `mads` from outside the clone, in your own project folder. Then choose how it reaches a model in [Choose a provider](../how-to/choose-a-provider.md). An API provider needs a key variable and `--model`. An agent CLI such as `claude-cli` needs the CLI on your `PATH`.

Check the provider.

```bash
mads providers
```

You should see the row of your provider end in `ready`. A key that is not set shows as `missing <VARIABLE>`. The full table and every status are in [Providers](../reference/providers.md).

If this fails, see [Troubleshoot](../how-to/troubleshoot.md).

## 2. Draft the input

Make a folder and let an agent read your site. This step calls your provider.

```bash
mkdir vinellu-ads
cd vinellu-ads
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider <provider> --model <model-id>
```

`--daily-budget` is the whole account per day, in the account currency. Pick it from what you can lose in a day, not from a bid guess. `init` reads HTML only. A site that builds its pages in the browser gives the agent little to read.

Example run (replay provider, no model was called). The site was a local copy on `127.0.0.1:8765`.

```text
10:21:17 [run] init started (replay, default model)
10:21:17 [init] started (attempt 1)
10:21:17 [init] tokens 46000 in, 6900 out
10:21:17 [init] > fetch_page
10:21:17 [init] < ok fetch_page http://127.0.0.1:8765/ (10 links)
10:21:17 [init] > fetch_sitemap
10:21:17 [init] < ok fetch_sitemap 17 of 17 URLs
10:21:17 [init] > search_site
10:21:17 [init] < ok search_site: 3 of 5 names found
10:21:17 [init] > write_business Vinellu
10:21:17 [init] < ok business Vinellu
10:21:17 [init] > add_catalog_items
10:21:17 [init] < ok catalog: +12 (0 skipped), 12 total
10:21:17 [init] > write_research
10:21:17 [init] < ok research: 3 opportunities
10:21:17 [init] > finish
10:21:17 [init] < ok init finished
10:21:17 [init] finished
10:21:17 [run] logo: none of 0 candidates is a PNG or JPEG of 144 px or more: add [brand] logo for image campaigns
10:21:17 [out] ./business.toml
10:21:17 [out] ./catalog.csv
10:21:17 [out] ./research.md
10:21:17 [run] done (exit 0): 46000 in, 6900 out tokens, cost n/a, 1 missions finished
```

It also printed these lines on stderr.

```text
Read what the agent found in ./research.md
Restricted categories: Alcohol. Google may refuse some keywords: the report explains how to ask for an exception
No logo found on the site: add [brand] logo to business.toml for image campaigns
Review ./business.toml and the catalog, then run: mads generate ./business.toml --provider replay
```

You should see `business.toml`, `catalog.csv` and `research.md` in the folder, and exit code `0`. The `Restricted categories` line is the agent's decision for a wine business. See [Restricted categories](../explanation/restricted-categories.md).

If this fails, see [Draft the files from your site](../how-to/draft-from-your-site.md).

## 3. Review the input

The draft is a start. You own what the ads say, so read `business.toml` and `catalog.csv` before any campaign exists. The list below is the pre-flight.

| Check | Where | What to do |
|---|---|---|
| Conversion tracking | `business.conversion_tracking` | Set `true` only if step 0 shows a working conversion action. |
| Highest bid | `budget.max_cpc` | Add it, in the account currency, at what one click is worth to you. A bid above it fails validation (`E11`). |
| Tracking suffix | `export.url_suffix` | Check that the UTM parameters are the ones your analytics expects. It becomes the Final URL suffix. |
| Status | `export.status` | Leave it `Paused`. It is the default. |
| Brand and rivals | `brand_terms`, `competitors` | They become negatives in the campaigns they do not belong to. |
| Words you never want | `avoid` | Ad text with one of them fails validation (`E05`). |
| Claims | `description` | The agents build ads from it. Remove what you cannot prove. |
| Restricted content | `restricted` | Keep what `init` set. It changes the prompts and the report. |
| Catalog | `catalog.csv` | Check names, URLs and `third_party`. |

Example run (replay provider, no model was called). This is the `business.toml` that the `generate` sample read. The values `conversion_tracking`, `max_cpc` and `[export]` are the ones a person adds after `init`.

```toml
[business]
name = "Vinellu"
url = "https://vinellu.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "cadastros no app"
description = """
App social de vinhos. A foto do rótulo mostra nota, safra,
harmonização e reviews de quem já bebeu. Mais de 170 mil rótulos.
Grátis no Android e no iOS.
"""
conversion_tracking = true
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

[catalog]
file = "catalog.csv"
```

The sample also lists three more `business.pages`. The checks in [Review a run](../how-to/review-a-run.md) explain each row. Every key is in [business.toml](../reference/business-toml.md) and every column in [catalog.csv](../reference/catalog-csv.md).

You should see a file where `max_cpc` is set, `export.status` is `Paused` and `conversion_tracking` matches your account. A key mads does not know stops the next step with exit code `2` before any model runs.

If this fails, see [Write the business file](../how-to/write-the-business-file.md).

## 4. Generate

Build the account. This step calls your provider.

```bash
mads generate business.toml --provider <provider> --model <model-id>
```

Example run (replay provider, no model was called, `--skip-url-check`). This is an excerpt of the log. Lines between the parts were cut.

```text
10:21:17 [run] 20261127-102117-a7fb01 started (replay, default model)
10:21:17 [plan] started (attempt 1)
10:21:17 [plan] tokens 34000 in, 5200 out
10:21:17 [plan] > get_business
10:21:17 [plan] < ok get_business
10:21:17 [plan] > query_catalog
10:21:17 [plan] < ok query_catalog: 12 of 12 items
10:21:17 [plan] > set_brand_kit
10:21:17 [plan] < ok brand kit: 10 headlines, 3 descriptions
10:21:17 [plan] > set_account_plan
10:21:17 [plan] < ok plan: 3 campaigns, 10 ad groups
10:21:17 [plan] > finish
10:21:17 [plan] < ok plan finished
10:21:17 [plan] finished
```

```text
10:21:17 [campaign:vinellu-rotulos] < ok rotulo-alamos: 11 kw, 4 neg, rsa 15/4
10:21:17 [campaign:vinellu-rotulos] > upsert_ad_group rotulo-luigi-bosca
```

```text
10:21:17 [run] cross-negatives: brand and competitor terms
10:21:17 [run] validate: checking limits and policies
10:21:18 [validate] 0 errors, 40 warnings
10:21:18 [validate] W05 campaigns[0].assets: recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet
10:21:18 [validate] W04 campaigns[0].ad_groups[0].rsa: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
10:21:18 [run] url-check: skipped
10:21:18 [run] export: writing files
10:21:18 [run] done (exit 0): 135000 in, 21800 out tokens, cost n/a, 4 missions finished
```

The run directory is `out/<run-id>/`. Here it was `out/20261127-102117-a7fb01/`.

You should see `[validate] 0 errors`, a `done (exit 0)` line and this layout in the run directory.

```text
google-ads/
input/
transcripts/
events.ndjson
report.md
run.json
workspace.json
```

One validation error and mads writes no CSV. It exits with `3`. If this fails, see [Exit codes and errors](../reference/exit-codes-and-errors.md) and [Resume or re-export a run](../how-to/resume-a-run.md).

A `GEMINI_API_KEY` or `OPENAI_API_KEY` in your shell turns on paid pictures through `--image-provider auto` when a run builds image campaigns. Pass `--image-provider none` if you do not want that.

## 5. Read the report

Open `out/<run-id>/report.md`. Read it before you open anything in Google Ads. [report.md](../reference/report-md.md) lists every section.

Example run (replay provider, no model was called, `--skip-url-check`). These are slices of the report.

```markdown
> Import only `google-ads/editor/account.csv` into Google Ads Editor. Do not import the web bulk CSVs (`google-ads/bulk/`) into Editor.

# mads report: Vinellu
20261127-102117-a7fb01 | 2026-11-27 | provider replay | model default

> Ready to import. Review the files in `google-ads/` before uploading.

## Summary

| Campaign | Format | Intent | Daily budget | Bidding | Groups | Keywords |
|---|---|---|---|---|---|---|
| Vinellu - Marca | Search | brand | 8,00 BRL | Manual CPC | 1 | 9 |
| Vinellu - Rotulos | Search | catalog | 30,00 BRL | Manual CPC | 6 | 71 |
| Vinellu - App de vinhos | Search | generic | 12,00 BRL | Manual CPC | 3 | 24 |
```

```markdown
**Vinellu - Marca**: Protects the brand name at a low CPC. Searches for Vinellu are still rare, so it gets the smallest share.

- marca: CPC 0,80 (Brand clicks are cheap. 0,80 is enough while nobody else bids on the name.)
```

```markdown
**W04 (5)**

- `campaigns[0].ad_groups[0].rsa`: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
- `campaigns[1].ad_groups[4].rsa`: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)

**W05 (2)**

- `campaigns[0].assets`: recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet
```

```markdown
## URL check

skipped
```

What to read, in order.

- The status line. `Ready to import` means exit `0`. `Not exported` means exit `3`.
- The summary. The budgets add up to `budget.daily`, or the run would have stopped with `E06`.
- The bids. They are estimates. mads has no auction data, so compare them with the Keyword Planner.
- The validation warnings. They never block the export.
  - `W01` is a third-party name in ad text. Here it is the wine labels, on purpose. Google's trademark rules can still limit those ads in your market.
  - `W04` is an ad with fewer than 15 headlines or 4 descriptions. It still serves.
  - `W05` is a campaign with fewer than 4 sitelinks, 4 callouts or no structured snippet. mads does not export those assets, so step 7 adds them.
- The URL check. On a real run it lists every URL with its status, and one that does not answer 2xx is `E15`.
- Restricted categories. The report explains how to ask Google for an exception when it refuses an item.

You should see the status `Ready to import` and a campaign table that matches the budget you set. The meaning of every code is in [Validation rules](../reference/validation-rules.md). If a campaign or a claim is wrong, fix the input and run step 4 again. The checklist for this review is [Review a run](../how-to/review-a-run.md).

## 6. Import

Use Google Ads Editor and import `google-ads/editor/account.csv`. Do not import the files in `google-ads/bulk/` as well. They are for the web upload, and importing both creates every Search campaign twice. Every later optimize run goes through Editor, so start there. [Upload the bulk CSVs](../how-to/upload-bulk-csvs.md) covers the web upload.

1. Open Editor and select Get recent changes in the toolbar, so Editor has the account as it is. See [Review recent account changes](https://support.google.com/google-ads/editor/answer/30582).
2. Select Account, then Import. Choose From file ... and pick `account.csv`. See [Import a CSV file](https://support.google.com/google-ads/editor/answer/30564).
3. Review the changes. To accept them all, select Keep proposed changes.
4. Select Post in the toolbar, review the summary and select Post to continue. See [Post changes to Google Ads](https://support.google.com/google-ads/editor/answer/30583).

The campaigns arrive with the status of `export.status`, here `Paused`. Nothing serves yet.

You should see the three Vinellu campaigns in Editor with the Paused status, and the post finishing without errors. Google can flag keywords or ads of a restricted category. Ask for an exemption in Editor before you post, because an item that posted and was later disapproved cannot get one. The report says how, and [Handle restricted categories](../how-to/handle-restricted-categories.md) has the steps, including what to do when an item is already posted.

If this fails, see [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md).

## 7. Go live

Turn off the auto-apply recommendations you did not choose first, so Google does not change the account behind mads. See step 4 of [Go live](../how-to/go-live.md). Then turn on one campaign. In Google Ads, open Campaigns within the Campaigns menu, select the status icon next to the campaign and select Enable. See [Enable, pause, or remove a campaign](https://support.google.com/google-ads/answer/2404259).

Add what mads does not export once the first campaign is approved, as step 3 of [Go live](../how-to/go-live.md) does. mads validates sitelinks, callouts and structured snippets but writes none of them, so add sitelinks (links under the ad), callouts (short lines of text) and structured snippets (a header with a list of values) in Google Ads. [Limitations](../reference/limitations.md) lists the rest. Then check these in the account.

- Budget, location, language and bidding match what the report says.
- Every ad has the status you expect. Policy flags show up here.
- The Final URL suffix carries your tracking parameters.

[Go live](../how-to/go-live.md) has the full checklist. Enable the other campaigns one by one when the first one serves.

You should see the campaign with the status Enabled and impressions in the next hours. If an ad group shows no ads, see [Troubleshoot](../how-to/troubleshoot.md).

## 8. Wait

Let the account run. mads cannot judge a campaign on a few days of clicks, so it draws these lines.

| Line | Value | Source |
|---|---|---|
| Thin | under 14 days or under 100 clicks, and under 30 conversions | `THIN_DAYS`, `THIN_CLICKS`, `JUDGE_CONVERSIONS` in `perf/model.rs` |
| Judged early | 30 conversions | `JUDGE_CONVERSIONS` |
| Stalled | enabled and has no ads, or 7 days without one impression | `STALLED_DAYS` |

A thin campaign only gets structural fixes in the next optimize. These lines are mads's own, not Google rules. mads writes Manual CPC bids only, so Google's Smart Bidding "Learning" status, which Google says never lasts more than 7 days, does not apply to its Search campaigns. See [About Smart Bidding learning](https://support.google.com/google-ads/answer/13020501).

A small budget can take months to reach 100 clicks per campaign. Wait for it. An early optimize on thin data changes structure only.

Do not edit budgets or bid strategies by hand without a reason. mads cannot tell your edit from a change Google applied through recommendations. It reads both as drift. A live budget more than 20 percent away from what the run exported is flagged, and so is a changed bid strategy, a pause, a removal and a dropped campaign enabled again. [How optimize decides](../explanation/how-optimize-decides.md) explains what it does with each.

After 2 days, `report.md` asks you to check keyword bid estimates. Its After launch section lists this and the reports to export.

You should see at least 14 days and 100 clicks on the campaigns you want judged, or 30 conversions.

## 9. Export the reports

Export the Google Ads reports as CSV into one folder, all for the same dates. For cycle 1 that is the whole life of the account, from the day it started serving. Step 12 gives the range for the next cycles. mads tells a report from its columns, not from its file name, so the names do not matter. It reads eight kinds.

Example run (replay provider, no model was called). This is the `Performance data` list of the cycle 1 report. The sample dates come from a fixture and do not line up with the run id of step 4. Use your own.

```markdown
## Performance data

Reports from 2026-10-13 to 2026-11-02 (21 days).

- `campanhas-por-dia.csv`: CampaignsByDay, 84 rows
- `campanhas.csv`: Campaigns, 4 rows
- `conversoes-por-acao.csv`: ConversionActions, 6 rows
- `grupos-de-anuncios.csv`: AdGroups, 10 rows
- `palavras-chave.csv`: Keywords, 103 rows
- `recursos.csv`: Assets, 119 rows
- `redes.csv`: Networks, 6 rows
- `termos-de-pesquisa.csv`: SearchTerms, 25 rows
```

The `After launch` list in `report.md` names seven exports. Export all eight kinds from [Export Google Ads reports](../how-to/export-google-ads-reports.md), which has the steps in the Google Ads interface. Do not export the ads report. mads reads no ad-level numbers and skips it. The columns of each kind are in [Google Ads reports](../reference/google-ads-reports.md).

Keep the campaign and ad group names. They are the key between a report row and the run, and renaming one breaks the link.

You should see a folder with the CSV files. The next step prints how many mads read.

## 10. Optimize

Pass the run you imported and the folder. This step calls your provider.

```bash
mads optimize out/<run-id> --reports <folder> --provider <provider> --model <model-id>
```

`<run-id>` is the `generate` run from step 4. mads never changes it. It copies the input saved in that run, so edits you made to `business.toml` after step 4 do not reach the new run. Pass `--daily-budget`, `--target-cpa` or another flag for what must change. [Optimize a live account](../how-to/optimize-a-live-account.md) covers each flag, and the targets, `--app-events` and `--pause-outside-run` in particular.

Before the missions start, mads prints what it found in the reports.

Example run (replay provider, no model was called, `--skip-url-check`, `--pause-outside-run`). These are the lines printed on stderr.

```text
reports: 8 read (2026-10-13 to 2026-11-02, 21 days)
not in this run: Vinellu - Search antiga
too little data, structure only: Vinellu - Rotulos, Vinellu - App de vinhos
stalled, no ads or no impression for 7 days, no more budget: Vinellu - Marca
budget unspent every day: 41.23
enabled outside this run and spending without conversions while the account converts, money the plan does not manage: Vinellu - Search antiga (47.21% of the account cost, 241 clicks)
paused by the import (--pause-outside-run): Vinellu - Search antiga
conversion actions of Vinellu - Rotulos: Cadastro concluído (6), Clique em baixar o app (1)
conversion actions of Vinellu - App de vinhos: Cadastro concluído (2), Clique em baixar o app (1)
networks of Vinellu - Rotulos: search 92% of cost, 100% of conversions, CTR 2.1%; search partners 8% of cost, 0% of conversions, CTR 1.33%
networks of Vinellu - App de vinhos: search 92% of cost, 100% of conversions, CTR 5.16%; search partners 8% of cost, 0% of conversions, CTR 3%
drift since the last run (changed outside mads (by hand or by auto-applied recommendations)):
  Vinellu - App de vinhos: daily budget is 67% higher than the last run exported
  Vinellu - App de vinhos: bids with `Maximizar cliques`, the last run exported Manual CPC
  the daily budgets of the enabled campaigns add up to 136% of the daily budget of this run
```

Then the missions run like in step 4 and the run ends with this line.

```text
10:21:18 [run] done (exit 0): 220000 in, 26400 out tokens, cost n/a, 4 missions finished
```

A new run directory holds the result. Its id is on the `[run] <id> started` line, and it is the newest folder in `out/`. Open its `report.md`. Two sections are new.

Example run (replay provider, no model was called, `--skip-url-check`, `--pause-outside-run`). Slices of the cycle 1 report.

```markdown
## Drift since the last run

The live account differs from what the last run exported. These were changed outside mads (by hand or by auto-applied recommendations): the reports cannot tell which. The budgets in this run start from the live ones.

- Vinellu - App de vinhos: daily budget is 67% higher than the last run exported
- Vinellu - App de vinhos: bids with `Maximizar cliques`, the last run exported Manual CPC
- the daily budgets of the enabled campaigns add up to 136% of the daily budget of this run
```

```markdown
## Changes

Compared with the account that ran. Each campaign rationale and CPC reason is in Budget and bids.

**Vinellu - Marca**
- daily budget 8,00 to 5,00
- marca: CPC 0,80 to 1,20
- marca: keywords added `vinelu` (phrase), `vinelu app` (phrase), `vinelu download` (phrase), `vinelu` (exact)

**Vinellu - Rotulos**
- daily budget 30,00 to 32,00
- rotulo-alamos: CPC 1,50 to 2,20
- rotulo-alamos: negatives added `preço` (phrase), `comprar` (phrase)
```

You should see exit code `0`, a `Changes` section that lists each edit and a `Drift` section when the account moved outside mads. The replay script is not a model decision: it also raises a budget and a bid on a thin campaign, which a real agent is told not to do for lack of results. If the command exits `1` before a run directory exists, the message names the reports or the base run. See [Exit codes and errors](../reference/exit-codes-and-errors.md).

## 11. Import the new version

Import the new run's `google-ads/editor/account.csv` into Editor, the same way as step 6. Editor never deletes on import, so everything the new run dropped arrives with the status `Paused`.

The report for this run says what the import does.

```markdown
## How to import

This run replaces an account that is already live. Import it with Google Ads Editor only: the files in `google-ads/bulk/` would add every Search campaign a second time.
```

The rest of that paragraph, cut here, says that campaigns the plan keeps arrive enabled, new ones follow `export.status` and what the run dropped arrives paused.

After the import, do these by hand.

- A new responsive search ad comes in next to the old one. Pause the old one when the new one is approved.
- A negative keyword the new run dropped stays in Google Ads. Remove it yourself.
- A bid strategy you changed in Google Ads stays as it is. The import leaves it empty, so the ad group CPCs only count under Manual CPC.

The exact rows are in [Google Ads Editor file](../reference/editor-file.md).

You should see, in Editor, the paused rows of what the run dropped and the changed budgets and bids as proposed changes. Keep them, post them, and the account matches the new run.

If this fails, see [Import into Google Ads Editor](../how-to/import-into-google-ads-editor.md).

## 12. The next cycle

Repeat steps 8 to 11. Two choices differ from cycle 1.

**Pass the run you imported last.** The base of cycle 2 is the optimize run from step 10, not the `generate` run. Drift reads the live data that only an optimize run stores. A campaign cycle 1 dropped, which someone enabled again, is flagged only when the base has that data.

**Export reports from the import date on.** The reports are read as one date range, and days running count to its end. A range that starts at the import keeps the numbers of the old structure out.

```bash
mads optimize out/<optimize-run-id> --reports <new-folder> --provider <provider> --model <model-id>
```

Flags are not remembered between cycles. The budget, the three targets and `export.status` come from the base run unless you pass them. `--pause-outside-run` and `--app-events` are not carried over, so pass them again when you want them.

Example run (replay provider, no model was called, `--skip-url-check`). Cycle 2 on the cycle 1 run. These are the lines printed on stderr.

```text
reports: 8 read (2026-11-03 to 2026-11-23, 21 days)
not in this run: Vinellu - Search antiga
too little data, structure only: Vinellu - Marca
budget unspent every day: 1.95
conversion actions of Vinellu - Marca: Cadastro concluído (1)
conversion actions of Vinellu - Rotulos: Cadastro concluído (29), Clique em baixar o app (7)
conversion actions of Vinellu - App de vinhos: Cadastro concluído (6), Clique em baixar o app (2)
networks of Vinellu - Marca: search 92% of cost, 100% of conversions, CTR 15.15%; search partners 8% of cost, 0% of conversions, CTR 0%
networks of Vinellu - Rotulos: search 92% of cost, 94.4% of conversions, CTR 3.93%; search partners 8% of cost, 5.6% of conversions, CTR 2.38%
networks of Vinellu - App de vinhos: search 92% of cost, 100% of conversions, CTR 4.7%; search partners 8% of cost, 0% of conversions, CTR 2.87%
drift since the last run (changed outside mads (by hand or by auto-applied recommendations)):
  Vinellu - Marca: paused, the last run exported it enabled
  Vinellu - App de vinhos: bids with `Maximizar cliques`, the last run exported Manual CPC
```

```text
10:21:18 [run] done (exit 0): 235000 in, 27200 out tokens, cost n/a, 4 missions finished
```

The drift lines are the check on yourself. Here someone paused the Marca campaign by hand after cycle 1, and the new run says so.

You should see exit code `0`, the new date range and the drift since cycle 1. Import the result as in step 11. You can now run the loop without this page. The commands are in [CLI](../reference/cli.md).

## Next

- [Optimize a live account](../how-to/optimize-a-live-account.md) for the flags, targets and app events.
- [Run in GitHub Actions](../how-to/run-in-github-actions.md) to run `generate` on a schedule.
- [Safe on a live account](../explanation/safe-on-a-live-account.md) for every guard between your file and the account.
