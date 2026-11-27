# Your first run

Install mads, point it at a website and read the Google Ads report it writes, in about 15 minutes and without a Google Ads account.

You end with a run folder that holds the account as CSV files and a `report.md` you have read. Nothing is uploaded anywhere. The tutorial stops before any import.

You need:

- A terminal with `git` and `rustup`.
- A website of your own to read. Any small business site works.
- One LLM provider you can call. See [Choose a provider](../how-to/choose-a-provider.md) if you have none.

Two steps call a model for real: step 3 (`mads init`) and step 5 (`mads generate`). They spend tokens from your provider account. Every other step is free.

The samples on this page come from the hidden `replay` provider, which plays back a script. **No model was called to make them.** They use a demo site called Vinellu. Your names, numbers, ids and times will differ.

## 1. Install mads

Clone the repository and install the binary. The repository pins its Rust toolchain, so the first build downloads the right version.

```bash
git clone https://github.com/avelino/mads
cd mads
cargo install --path crates/mads-cli
```

Check it.

```bash
mads --version
```

You should see:

```text
mads 0.1.0
```

If this fails, see [Install mads](../how-to/install.md).

## 2. Pick a provider

List the providers and find one that shows `ready`.

```bash
mads providers
```

You should see one row per provider, then a second table for image providers. The statuses depend on your machine.

Example output (real command, run on a machine with the `claude` CLI installed):

```text
PROVIDER         KIND  STATUS
anthropic        api   missing ANTHROPIC_API_KEY
openai           api   missing OPENAI_API_KEY
gemini           api   missing GEMINI_API_KEY
openrouter       api   missing OPENROUTER_API_KEY
groq             api   missing GROQ_API_KEY
deepseek         api   missing DEEPSEEK_API_KEY
xai              api   missing XAI_API_KEY
ollama           api   ready
openai-compat    api   ready
claude-cli       cli   ready
codex-cli        cli   `codex` not found in PATH
gemini-cli       cli   `gemini` not found in PATH

IMAGE PROVIDER         STATUS
gemini                 missing GEMINI_API_KEY
openai                 missing OPENAI_API_KEY
none                   ready
```

Export the key of an API provider, or log in to the CLI of a CLI provider. Then tell mads which one to use for the rest of the session. An API provider also needs a model id, because mads has no default model. A CLI provider does not.

```bash
export MADS_PROVIDER=anthropic
export MADS_MODEL=<model-id>
```

Run `mads providers` again. Your provider must show `ready`. The providers, keys and model rules are in [Providers](../reference/providers.md). `ollama` and `openai-compat` show `ready` without a check that a server is listening.

## 3. Draft your input from your site

This step calls a model for real.

Make an empty folder and let an agent read your site. The daily budget and the currency are flags because a site does not say how much you want to spend. Use your own currency code. Nothing is spent on Google Ads.

```bash
mkdir my-first-run
cd my-first-run
mads init --from-url https://your-site.example --daily-budget 50 --currency BRL
```

You should see the agent read your pages and write three files. On a terminal you get one spinner per mission. Add `--format plain` to get lines like these.

Example run (replay provider, no model was called). The site was a local copy served at `http://127.0.0.1:8765`, so init ran with `MADS_ALLOW_PRIVATE_HOSTS=1`. Your run reads your own `https` site.

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

The command also prints these lines on stderr.

```text
Read what the agent found in ./research.md
Restricted categories: Alcohol. Google may refuse some keywords: the report explains how to ask for an exception
No logo found on the site: add [brand] logo to business.toml for image campaigns
Review ./business.toml and the catalog, then run: mads generate ./business.toml --provider replay
```

If this fails, see [Draft the files from your site](../how-to/draft-from-your-site.md).

## 4. Review the draft

Read `research.md` first. It lists what the agent found and what it is unsure about.

Example `research.md` (from the same replay run, cut after the first opportunity):

```markdown
# Research: Vinellu

Vinellu is a free social wine app. People photograph a label and get its score, vintages, food pairing and reviews from friends. The site lists more than 170 thousand labels, each with its own page. Customers are Brazilian wine drinkers who search a label by name before they buy or open it.

## Opportunities

Best expected return first. Demand and competition are the agent's estimates from the site and the web, not keyword planner data.

### 1. Labels by name

- Intent: catalog
- Demand: high. Competition: low.
- Searches: `alamos malbec`, `casillero del diablo review`, `luigi bosca safra`
- Evidence: Each label has a page on the site, and the best sellers in Brazil are searched by name with words like review and safra.
- Found on the site: 5 of 5 names checked.
- Sources: http://127.0.0.1:8765/sitemap.xml
```

Then open `business.toml`. The agents may claim only what `description` says, so fix anything wrong in it. Add a CPC cap under `[budget]`. Every bid the agents choose stays at or below it.

```toml
[budget]
daily = 50.0
currency = "BRL"
max_cpc = 3.0
```

You should see `business.toml`, `catalog.csv` and `research.md` in the folder, and a `budget` table with `max_cpc`. Every key is in the [business.toml reference](../reference/business-toml.md).

## 5. Generate the account

This step calls a model for real.

Pass `--image-provider none` so a `GEMINI_API_KEY` or `OPENAI_API_KEY` in your shell cannot turn on paid image generation. Without it, `--image-provider auto` picks whichever key it finds.

```bash
mads generate business.toml --image-provider none
```

mads checks the file, runs the plan mission, runs one mission per campaign, then adds cross negatives, validates the account, requests every final URL and writes the files. The command exits `0` when the files are written.

You should see one line per tool call. Add `--format plain` to get lines on a terminal.

Example run (replay provider, no model was called). It ran with `--skip-url-check`, so the `url-check` line says `skipped`. Yours requests your URLs. Its `business.toml` also set `conversion_tracking = true` and `[export] status = "Paused"`. The listing is an excerpt, and the `...` lines mark what was left out. The `[out]` lines list the seven files in the folders shown below, and their long absolute paths are left out.

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
10:21:17 [campaign:vinellu-marca] started (attempt 1)
10:21:17 [campaign:vinellu-rotulos] started (attempt 1)
10:21:17 [campaign:vinellu-app-de-vinhos] started (attempt 1)
...
10:21:17 [campaign:vinellu-rotulos] < ok campaign finished
10:21:17 [campaign:vinellu-rotulos] finished
10:21:17 [run] cross-negatives: brand and competitor terms
10:21:17 [run] validate: checking limits and policies
10:21:18 [validate] 0 errors, 40 warnings
10:21:18 [validate] W05 campaigns[0].assets: recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet
10:21:18 [validate] W04 campaigns[0].ad_groups[0].rsa: merged ad has 14 headlines and 4 descriptions (15 and 4 recommended)
...
10:21:18 [validate] and 20 more warnings
10:21:18 [run] url-check: skipped
10:21:18 [run] export: writing files
...
10:21:18 [run] done (exit 0): 135000 in, 21800 out tokens, cost n/a, 4 missions finished
```

Warnings do not stop the run. Errors do, with exit code `3` and no CSV. If this fails, see [Exit codes and errors](../reference/exit-codes-and-errors.md).

## 6. Find the run folder

mads writes one folder per run under `out/`, named with the UTC time and a random suffix.

```bash
ls out
```

Open the newest folder. These are the files of the example run. A real run holds the same ones.

```text
events.ndjson
google-ads/bulk/1-campaign.csv
google-ads/bulk/2-ad-groups.csv
google-ads/bulk/3-keywords.csv
google-ads/bulk/4-negative-keywords.csv
google-ads/bulk/5-responsive-search-ads.csv
google-ads/editor/account.csv
input/
report.md
run.json
transcripts/
workspace.json
```

You should see these files. Every file is explained in [Output files](../reference/output-files.md). You do not open the CSVs in this tutorial.

## 7. Read report.md

Open `report.md` in the run folder. It is the page to read before anything goes near a Google Ads account.

Example report (replay provider, no model was called), from the top. The status line says the run exported.

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

Read these four parts in order.

1. **Summary.** Each campaign, its share of the daily budget and its keyword count. The shares add up to your `budget.daily`.
2. **Budget and bids.** Every ad group CPC with the reason the agent gave. mads has no auction data, so these are estimates.
3. **Validation.** Warnings group by rule code with a count. The example has `W01`, `W04` and `W05`.
4. **URL check.** A table of every final URL and its HTTP status. The example shows `skipped` because of `--skip-url-check`.

Example slice of Budget and bids and Validation:

```markdown
**Vinellu - Marca**: Protects the brand name at a low CPC. Searches for Vinellu are still rare, so it gets the smallest share.

- marca: CPC 0,80 (Brand clicks are cheap. 0,80 is enough while nobody else bids on the name.)

## Validation

### Warnings

**W01 (33)**

- `campaigns[1].assets.snippets[0].values[0]`: third-party term 'alamos' in ad text (trademark policy risk)
- `campaigns[1].assets.snippets[0].values[1]`: third-party term 'luigi bosca' in ad text (trademark policy risk)
```

Money in the report follows your `budget.currency`. The rule codes are in [Validation rules](../reference/validation-rules.md). The remaining sections are in [report.md](../reference/report-md.md).

You should see a status line that starts with `Ready to import`, a summary row for each campaign and a Validation section with zero errors. You have a run that mads would let you import. The tutorial stops here, because importing needs a Google Ads account.

## Next

- Run the same loop on a real account: [Run a real account](run-a-real-account.md).
- Go through the checks before an import: [Review a run](../how-to/review-a-run.md).
- Look up every section of the report: [report.md](../reference/report-md.md).
