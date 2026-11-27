# Draft the files from your site

Let an agent read your website and draft `business.toml`, `catalog.csv` and `research.md`, so you start from a reviewed draft and not from an empty file.

## Before you start

- A provider that works. See [Choose a provider](choose-a-provider.md). With an agent CLI the agent also searches the web. See [Agent CLIs](../explanation/agent-clis.md#web-search-in-init).
- A site that serves its content as HTML. The fetcher runs no JavaScript, so a site that renders in the browser gives the agent little to read. See [Limitations](../reference/limitations.md).
- The daily budget and the currency of your Google Ads account. The site does not say how much you want to spend, so both are flags. See [Prepare your Google Ads account](prepare-google-ads.md).
- An empty folder, or a folder you are fine overwriting with `--force`.

`mads init` calls a model. It spends your tokens, and web search adds to them. `--no-web-search` keeps an agent CLI off the web.

## Steps

### 1. Run init

```bash
mads init --from-url https://example.com --daily-budget 50 --currency BRL --provider claude-cli
```

With an API provider, add `--model <model-id>` and drop `--provider claude-cli` for the one you use. The flags are in the [CLI reference](../reference/cli.md#mads-init).

The agent reads the home page, maps the sitemap, looks for what people search and writes the files. Files go to the current folder. `--out-dir` changes that.

Example run (replay provider, no model was called). It read a local copy of a site at `http://127.0.0.1:8765` with `MADS_ALLOW_PRIVATE_HOSTS=1`, so the URLs are local. It ran with `--no-web-search`. Progress goes to stdout.

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

The last lines go to stderr and tell you what to check.

```text
Read what the agent found in ./research.md
Restricted categories: Alcohol. Google may refuse some keywords: the report explains how to ask for an exception
No logo found on the site: add [brand] logo to business.toml for image campaigns
Review ./business.toml and the catalog, then run: mads generate ./business.toml --provider replay
```

### 2. Optional: advertise one offer, not the business

Pass the page of one offer and `--focus` to advertise only it, such as one route, one product line or one city.

```bash
mads init --from-url https://example.com/routes/a-to-b --focus --daily-budget 50 --currency BRL --provider claude-cli
```

The agent still studies the business for context. It writes a `[focus]` table and keeps the catalog and the opportunities inside it. Without `--focus`, the URL is only where the agent starts. The table is in [business.toml](../reference/business-toml.md#focus).

For several offers, run init once per offer, each in its own `--out-dir`.

### 3. Optional: push some products

Pass `--strategy` to say which products get most of the budget and which searches they buy. With a strategy, init also writes `STRATEGY.md`. `--product <URL>` names the pages and `--top <N>` lets the agent pick. See [Pick a campaign strategy](pick-a-strategy.md).

```bash
mads init --from-url https://example.com --strategy need --daily-budget 50 --currency BRL --provider claude-cli
```

### 4. Read the research first

`research.md` is where the agent shows its work. If its summary of your business is wrong, the rest is wrong too. It has a summary, opportunities with the best expected return first, and open questions.

Demand and competition in it are the agent's estimates. They are not keyword planner data.

This slice of the example run shows the shape of one opportunity and the open questions. Lines are left out where marked.

```markdown
## Opportunities

Best expected return first. Demand and competition are the agent's estimates from the site and the web, not keyword planner data.

### 1. Labels by name

- Intent: catalog
- Demand: high. Competition: low.
- Searches: `alamos malbec`, `casillero del diablo review`, `luigi bosca safra`
- Evidence: Each label has a page on the site, and the best sellers in Brazil are searched by name with words like review and safra.
...
## Open questions

- Is a conversion action recording app sign-ups in Google Ads?
- Which labels sell best this season?
```

Delete an opportunity you do not want. The plan agent reads this file through the `[research]` table, so a deleted opportunity no longer shapes the budget split. Remove the table and the plan runs without notes.

With web search on, `write_research` refuses research that cites fewer than 3 pages outside your site. The agent has to search to finish. API providers do not search the web. The agent learns from the site alone and lists what it could not check in the open questions.

### 5. Read business.toml

This is the file from the example run. The shape is what your run writes. The values are yours.

```toml
[business]
name = "Vinellu"
url = "http://127.0.0.1:8765"
language = "pt-BR"
locations = ["Brazil"]
goal = "cadastros no app"
description = "App social de vinhos. A foto do rótulo mostra nota, safra, harmonização e reviews de quem já bebeu. Mais de 170 mil rótulos. Grátis no Android e no iOS."
brand_terms = ["vinellu"]
competitors = ["Vivino"]
avoid = ["melhor do mundo"]
restricted = ["alcohol"]

[[business.pages]]
name = "Baixe o app"
url = "http://127.0.0.1:8765/app"

[[business.pages]]
name = "Como funciona"
url = "http://127.0.0.1:8765/sobre"

[[business.pages]]
name = "Blog"
url = "http://127.0.0.1:8765/blog"

[[business.pages]]
name = "Ajuda"
url = "http://127.0.0.1:8765/ajuda"

[budget]
daily = 50.0
currency = "BRL"

[catalog]
file = "catalog.csv"

[research]
file = "research.md"
```

And the first rows of `catalog.csv` from the same run.

```csv
name,url,category,aliases,third_party,notes
Alamos Malbec,http://127.0.0.1:8765/w/Wv3T9ngB8S/malbec,malbec,alamos,true,
Luigi Bosca Malbec,http://127.0.0.1:8765/w/uMm7sBLgDV/malbec,malbec,luigi bosca,true,
DV Catena Cabernet Malbec,http://127.0.0.1:8765/w/CjHn8hVT3r/d-v-catena-cabernet-sauvignon-malbec,blend,dv catena|d.v. catena,true,
```

### 6. Fix what is off

Treat the draft as a first pass. Read every line.

- `goal`: the agent may write it in another language than your ads. Rewrite it the way you want it.
- `description`: the agents write ads from it. Delete every number and claim you cannot back up.
- `competitors`: each name becomes a phrase negative in the other campaigns and a trademark warning in ad text. Remove names you do not want.
- `brand_terms`: left out, it is the lowercase business name. Add the other spellings people use.
- `avoid`: add terms that must never appear in an ad.
- `[app]`: init reads the whole page, JSON-LD included (`downloadUrl`, `sameAs`), and keeps the first app of each store, Google Play first. One app is written as `[app]`, two as `[[app]]`, and each store gets its own App campaign.
- `restricted`: init decides this for every business. `[]` means it checked and none applies. Add a category it missed and remove a wrong one. See [Handle restricted categories](handle-restricted-categories.md).
- `pages`: keep the pages that make good sitelinks and delete the rest.
- Catalog: delete items you do not sell or do not want ads for. Check `third_party`, `aliases` and `notes`. See [Write a catalog](write-a-catalog.md).
- `max_cpc`: init does not write it. Add it under `[budget]` to cap every CPC.
- `[export]`, `[campaigns]` and `[google_ads]`: init does not write them. Add them when you need them. See [business.toml](../reference/business-toml.md).
- `brand/logo.png`: if init found a logo, open it and check it is the full logo. init takes candidates from the markup in this order: `apple-touch-icon`, `og:logo`, the largest `<link rel="icon">`, then `<img>` tags with `logo` in `src`, `alt`, `class` or `id`. SVG, ICO and GIF are skipped. It keeps the first PNG or JPEG of 144 px or more, pads it to a square and shrinks it to fit 150 KB. The download may go to another public host, such as a CDN. A site icon can be a cropped mark, which is why you check it. If it found none, as in the example, add `[brand] logo` yourself before you ask for image campaigns. See [Add image and app campaigns](add-image-campaigns.md).
- `DESIGN.md`: init writes it when it finds brand colors or a design. Image campaigns follow it. Check the colors and the style.

Every key and its limits are in the [business.toml reference](../reference/business-toml.md). The full pre-flight list is in [Write the business file](write-the-business-file.md).

## Check it worked

- The command exited `0` and printed `[out]` lines for `business.toml` and `research.md`. `catalog.csv` is written only when the agent added items.
- The `Restricted categories:` line matches your business.
- `business.toml` has a `[research]` table, and `[catalog]` when `catalog.csv` exists.
- Every URL in `pages` and in the catalog opens on your site. The agent may only use URLs it fetched or saw in the sitemap.

## If it fails

- Exit `2` with `<files> already exist: use --force to overwrite`. init also checks `brand/logo.png`, `DESIGN.md` and `STRATEGY.md`. Pass `--force` to replace them. It keeps what you wrote by hand in `business.toml`. See [What mads init keeps with --force](../reference/business-toml.md). With `--force` and no catalog items, it deletes an old `catalog.csv` in that folder.
- Exit `1`. The mission did not finish after its retries, or the token budget ran out. No files are written. init prints `See what the agent did in <path>`. That is `.mads/transcripts/` in the output folder. See [Exit codes and errors](../reference/exit-codes-and-errors.md#init-tool-codes).
- A thin draft from a site that renders in the browser. init reads HTML only. See [Limitations](../reference/limitations.md).

## Next

- [Write the business file](write-the-business-file.md)
- [Pick a campaign strategy](pick-a-strategy.md)
- [Review a run](review-a-run.md)
