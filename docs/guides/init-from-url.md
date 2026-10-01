# Init from a URL

This page shows how to let an agent read your website and draft `business.toml` and `catalog.csv`, so you start `mads generate` from a reviewed draft instead of an empty file.

## What it does

`mads init` runs one agent mission. The agent reads your site through five tools and writes two files.

| Tool | What it does |
|---|---|
| `fetch_page` | Returns the title, description, visible text and links of one page. |
| `fetch_sitemap` | Lists sitemap URLs, filtered and paginated. |
| `write_business` | Saves the business profile. |
| `add_catalog_items` | Adds the entities people search for by name. |
| `finish` | Ends the mission. Needs `write_business`. |

The agent looks at the home page, follows the pages that explain the business, searches the sitemap for the pages of catalog items and then writes the draft. It is told to prefer specific entities (a wine label) over broad groups (a grape), and to use only facts from the site.

Every draft passes the same parsers `generate` uses, so a file that init writes always loads.

## Run it

You need a provider. See [Providers](providers.md) and [Agent CLIs](agent-clis.md). The budget and the currency are flags, because a site does not say how much you want to spend.

```bash
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider claude-cli
```

With an API provider.

```bash
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider anthropic --model <model-id>
```

`--plan-model` also applies to init. When set, init uses it instead of `--model`.

This is real output from a run with `claude-cli` against https://vinellu.com. Agent text and timings differ on every run.

```text
12:56:13 [run] init started (claude-cli, default model)
12:56:13 [init] started (attempt 1)
12:56:15 [init] > fetch_page
12:56:15 [init] < ok fetch_page https://vinellu.com/ (7 links)
12:56:15 [init] > fetch_sitemap
12:56:30 [init] < ok fetch_sitemap 60 of 8657 URLs
12:56:32 [init] > fetch_page
12:56:32 [init] < ok fetch_page https://vinellu.com/vinhos/melhores (21 links)
12:56:32 [init] > fetch_sitemap
12:56:32 [init] < ok fetch_sitemap 40 of 2877 URLs
12:56:34 [init] > fetch_page
12:56:34 [init] < ok fetch_page https://vinellu.com/vinhos/melhores/Malbec (61 links)
12:56:39 [init] > write_business Vinellu
12:56:39 [init] < ok business Vinellu
12:56:49 [init] > add_catalog_items
12:56:49 [init] < ok catalog: +20 (0 skipped), 20 total
12:56:50 [init] > finish
12:56:50 [init] < ok init finished
12:56:54 [out] ./business.toml
12:56:54 [out] ./catalog.csv
12:56:54 [run] done (exit 0): 68765 in, 3842 out tokens, cost $0.1099, 1 missions finished
Review ./business.toml and the catalog, then run: mads generate ./business.toml --provider claude-cli
```

(The output is trimmed. The real run printed more tool calls and agent text.)

That run read 4 pages, listed an 8657-URL sitemap in pages and wrote 20 catalog items for about 69k input tokens and 11 US cents. The last line goes to stderr and tells you the next step.

## Where the files go

`--out-dir` sets the folder. The default is the current directory. init writes exactly two files, `business.toml` and `catalog.csv`. It creates no run directory and no `events.ndjson`.

The catalog file is written only when the agent added items. Without items, `business.toml` has no `[catalog]` table.

init refuses to overwrite.

```text
error: ./business.toml, ./catalog.csv already exist: use --force to overwrite
```

Exit code `2`. Pass `--force` to replace them. With `--force` and no catalog items, mads also deletes an old `catalog.csv` in that folder, because it would not match the new `business.toml`.

## Read the draft

This is the `business.toml` from that run, shortened to the first page.

```toml
[business]
name = "Vinellu"
url = "https://vinellu.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "app installs"
description = "Vinellu é a rede social para quem ama vinho: o usuário avalia rótulos, segue amigos e descobre o próximo vinho pelas recomendações de pessoas reais. O app tem scanner de rótulos e cartas de restaurante por IA com Deal Score, adega inteligente que indica o apogeu de cada safra e comparação de preços em lojas parceiras. O catálogo reúne mais de 170 mil rótulos."
competitors = ["Vivino", "Delectable", "CellarTracker"]

[[business.pages]]
name = "Baixe o App"
url = "https://vinellu.com/app"

[budget]
daily = 50.0
currency = "BRL"

[catalog]
file = "catalog.csv"
```

And the first rows of `catalog.csv`.

```csv
name,url,category,aliases,third_party,notes
Château Haut-Brion Pessac-Léognan,https://vinellu.com/w/izL2cP5VSE/pessac-leognan,Tintos,Chateau Haut-Brion,true,"98 pts da crítica; Pessac-Léognan, França"
La Muse Verité,https://vinellu.com/w/hnTFJ7Rxpm/la-muse,Sonoma County,Verité La Muse,true,"100 pts; Sonoma County, EUA"
```

## What to fix by hand

Treat the draft as a first pass. Read every line. These are the usual fixes.

- **goal.** The agent can write it in another language than your ads. In the run above it wrote `app installs` for a site in Portuguese. Rewrite it the way you want it. See [The business file](business-file.md#goal).
- **description.** Every number and claim in it becomes something an ad can say. Delete what you cannot back up.
- **competitors.** Each name becomes a negative keyword in the other campaigns and a trademark warning in ad text. Remove names you do not want and add the ones that matter.
- **avoid.** The prompt does not ask the agent to fill it, so it is usually empty. Add terms that must never appear in an ad.
- **brand_terms.** Left out, it defaults to the lowercase business name. Add the other spellings people use.
- **max_cpc.** init does not write it. Add `max_cpc` under `[budget]` to cap every CPC.
- **[export].** init does not write it. Add it if you need `status`, `url_suffix` or `decimal_comma`. See the [business.toml reference](../reference/business-toml.md).
- **pages.** Keep the pages that make good sitelinks and delete the rest. They all exist, because the agent may only use URLs it fetched or saw in the sitemap.
- **Catalog.** Delete items you do not sell or do not want ads for. Check `third_party`, `aliases` and `notes`. The `notes` of each item are facts an ad can use.

Then run `mads generate` on the reviewed files. See [Getting started](../getting-started.md).

```bash
mads generate business.toml --provider claude-cli
```

## Limits and safety

The agent reads your site, and the text it reads goes to the model. mads limits what it can fetch.

- Only the start host and its `www.` or apex sibling. `https://vinellu.com` and `https://www.vinellu.com` count as one site.
- `robots.txt` is respected.
- IP-literal hosts and hosts that resolve to loopback, private or link-local addresses are refused. This keeps crawled text and requests away from internal services and cloud metadata endpoints. For local development you can lift it with `MADS_ALLOW_PRIVATE_HOSTS=1`. Do not set it in CI.
- At most 30 pages per run. A 31st `fetch_page` returns a `LIMIT` error.
- A page returns at most 8000 characters of visible text and 200 links.
- The sitemap is read in pages of at most 200 URLs. Indexes are followed 2 levels deep, with at most 50 child sitemaps. `.xml.gz` works.
- The catalog is capped by `--catalog-limit` (default 50). Adding more fails with `E13`.
- Page and catalog URLs the agent writes must have been fetched or listed in the sitemap in that run. Otherwise the tool returns an `E07` error and the agent fixes it.
- No JavaScript. The agent sees the HTML a plain request returns. A site that renders its content in the browser gives the agent little to read.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The agent finished and mads wrote the files. |
| `1` | The mission failed after its retries, or the token budget ran out. No files are written. |
| `2` | Bad flags or input. A URL that is not `http` or `https`, a budget that is not greater than 0 with at most 2 decimals, a currency that is not 3 uppercase letters, a missing provider, or existing files without `--force`. |

```text
error: --from-url must be an absolute http(s) URL, got 'vinellu.com'
error: --daily-budget must be greater than 0 with at most 2 decimals
error: --currency must be 3 uppercase letters, such as BRL
```

`--max-turns`, `--mission-timeout`, `--max-tokens` and `--mission-retries` work as in `generate`. See [Cost and limits](cost-and-limits.md). The tools are listed in [MCP tools](../reference/mcp-tools.md#init-mission).
