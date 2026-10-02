# Init from a URL

This page shows how to let an agent study your business and draft `business.toml`, `catalog.csv` and `research.md`, so you start `mads generate` from a reviewed draft instead of an empty file.

## What it does

`mads init` runs one agent mission. The agent knows nothing about your business when it starts. It learns how the business works, finds what people search for, ranks the opportunities by expected return and writes three files.

| Tool | What it does |
|---|---|
| `fetch_page` | Returns the title, description, visible text, links and `og:image` of one page. |
| `fetch_sitemap` | Lists sitemap URLs, filtered and paginated. |
| `search_site` | Finds the pages of a name in the sitemap. |
| `write_business` | Saves the business profile. |
| `write_research` | Saves what the agent learned and the opportunities it found. |
| `add_catalog_items` | Adds the entities people search for by name. |
| `write_design` | Saves how the brand looks and sounds. Optional. |
| `finish` | Ends the mission. Needs `write_business` and `write_research`. |

The agent reads the home page and the pages that explain the business, then maps the kinds of pages in the sitemap. Next it looks for demand: the names people type, the categories and problems they search, the business name and its competitors. With an agent CLI it also searches the web for rankings, bestseller lists and comparisons. See [Web search](#web-search).

It ranks what it finds by expected return, not by fame. A good opportunity has enough searches, a clear intent to act and few advertisers. A famous name that every big player bids on can cost more than it brings. The agent takes names from the market, not from the site. What a site features or ranks highest shows what it has, not what people search. So the agent builds the candidate names from bestseller and "most popular" lists for the country, then checks them all with `search_site`, up to 50 per call. The research says how many it checked and how many have a page, such as `34 of 40 names checked`. A name with demand and no page goes into the open questions.

The prompt teaches a method, not a business. It holds no rule about any market, so the same run works for a shop, an app or a course.

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

## Web search

With `claude-cli`, `codex-cli` or `gemini-cli`, the init agent can use the CLI's own web search. mads opens only that, and only for init. `generate` missions keep every built-in tool off. See [Agent CLIs](agent-clis.md#web-search-in-init).

A model with search can still skip it and write from memory. In one real run claude was offered search, made no search at all and finished in 10 turns. So when web search is on, `write_research` refuses research that cites fewer than 3 pages outside your site. The agent has to search to finish.

API providers do not search the web yet. The agent is told so and learns from the site alone. It lists what it could not check in the open questions.

`--no-web-search` keeps an agent CLI off the web too. Use it when you want the cheaper run.

Web search costs tokens. Each search adds its results to the agent's context, so a run with research costs more than one without. The numbers the agent writes about demand and competition are its estimates. They are not keyword planner data.

`--plan-model` also applies to init. When set, init uses it instead of `--model`.

This is real output from a run with `claude-cli` against https://vinellu.com. Agent text and timings differ on every run. The run is from before research existed, so it has no `search_site` or `write_research` lines and wrote no `research.md`.

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

`--out-dir` sets the folder. The default is the current directory. init writes at most five files, `business.toml`, `catalog.csv`, `research.md`, `brand/logo.png` and `DESIGN.md`. It creates no run directory and no `events.ndjson`. It keeps the agent's transcript in `.mads/transcripts/` inside the same folder, even when the mission fails. A failed run prints that path. Each run replaces the transcript of the one before.

The catalog file is written only when the agent added items. Without items, `business.toml` has no `[catalog]` table. `research.md` is always written, because `finish` needs `write_research`. `business.toml` points at it with a `[research]` table.

## One offer, not the whole business

Pass the page of one offer and `--focus` to advertise only it, such as one route, one product line or one city.

```bash
mads init --from-url https://www.buser.com.br/onibus/belo-horizonte-mg/sao-paulo-sp \
  --focus --daily-budget 10000 --currency BRL --provider claude-cli
```

The agent still studies the business for context, but it writes a `[focus]` table with the offer's name and pages (the start page and close variants such as the return direction), keeps the catalog to those pages, and ranks only ways people search for that offer. Without `--focus`, the start page is only where the agent begins: it drafts the whole business.

For several offers, run init once per offer in its own folder. Each one gets its own budget, run and report.

## The logo

Image campaigns need the brand logo, and mads never draws one. While the agent fetches pages, mads collects logo candidates from the markup: `apple-touch-icon` first, then `og:logo`, then the largest `<link rel="icon">`, then `<img>` tags with `logo` in `src`, `alt`, `class` or `id`. SVG, ICO and GIF files are skipped.

After the mission, init downloads the candidates in that order and keeps the first PNG or JPEG of 144 px or more. It pads it to a square with transparency, shrinks it until the PNG fits in 150 KB, saves `brand/logo.png` and adds `[brand] logo` to `business.toml`. Logos often live on a CDN, so this download may go to another public host. Private hosts are refused like any other request.

Without a usable candidate init prints that no logo was found, and the plan can only use Search. Put your logo in `brand/logo.png` and add the table yourself. Check the downloaded logo too: a site icon can be a cropped mark, not the full logo.

The agent also sees the `og:image` of each page and can set it as the `image` of a catalog item when it is a photo of the item. Image ads then show the real product. An `image` the run did not see on a fetched page fails with `E07`.

## The design

init writes `DESIGN.md` and a `[design]` table in `business.toml`. The `## Colors` section comes from code: the dominant colors of the saved logo (white, black, gray and transparent pixels are skipped, close shades merge, at most 3) and the `<meta name="theme-color">` of every fetched page. The agent can add `Style`, `Imagery`, `Voice` and `Avoid` with `write_design`. It is optional, and finish does not need it. The file is not written when there is neither a color nor a design draft.

Read it and fix what is off. Image campaigns follow it. See [Image campaigns](image-campaigns.md#brand-identity).

## Overwriting

init refuses to overwrite any of these files.

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

## Read the research

`research.md` is where the agent shows its work. Read it first. It is written in the language of the business. It has three parts.

- **Summary.** How the agent understood the business and how its customers search. If this is wrong, the rest is too.
- **Opportunities.** Campaign ideas, best expected return first. Each one has the intent, example searches, the agent's estimate of demand and competition, the evidence and the sources it used. An opportunity built on names also says how many of them the site has.
- **Open questions.** What the agent could not confirm. Typical ones are how much a customer is worth to you, or a demand with no page on your site.

```markdown
### 1. Labels by name

- Intent: catalog
- Demand: high. Competition: low.
- Searches: `alamos malbec`
- Evidence: Bestsellers get searched by name.
```

The file is yours to edit. `generate` reads it through the `[research]` table and hands it to the plan agent, which uses it to split the budget. Delete an opportunity you do not want and it will not shape the plan. Remove the `[research]` table and the plan runs without notes.

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
- **Catalog.** Delete items you do not sell or do not want ads for. Check `third_party`, `aliases` and `notes`. The `notes` of each item are facts an ad can use. Clear an `image` that is a banner and not a photo of the item.
- **brand/logo.png.** Open it. Replace it with your real logo when it is wrong.
- **DESIGN.md.** Check the colors and the style. Add the colors of your brand guide when the site does not show them.

Then run `mads generate` on the reviewed files. See [Getting started](../getting-started.md).

```bash
mads generate business.toml --provider claude-cli
```

## Limits and safety

The agent reads your site, and the text it reads goes to the model. mads limits what it can fetch.

- Only the start host and its `www.` or apex sibling. `https://vinellu.com` and `https://www.vinellu.com` count as one site.
- `robots.txt` is respected.
- IP-literal hosts and hosts that resolve to loopback, private or link-local addresses are refused. This keeps crawled text and requests away from internal services and cloud metadata endpoints. For local development you can lift it with `MADS_ALLOW_PRIVATE_HOSTS=1`. Do not set it in CI.
- At most 30 pages per run. A 31st `fetch_page` returns a `LIMIT` error. `search_site` reads the sitemap mads already fetched, so it does not count.
- A page returns at most 8000 characters of visible text and 200 links.
- The sitemap is read in pages of at most 200 URLs. Indexes are followed 2 levels deep, with at most 50 child sitemaps and 500 000 URLs in total. `.xml.gz` works.
- A sitemap file can be up to 50 MB, the limit of the sitemap protocol. A file that cannot be read (too large, an HTTP error) is listed in `skipped` in the results of `fetch_sitemap` and `search_site`, with the reason. The agent is told that a name missing from a partial sitemap may still exist.
- The catalog is capped by `--catalog-limit` (default 50). Adding more fails with `E13`.
- Page and catalog URLs the agent writes must have been fetched, listed in the sitemap or returned by `search_site` in that run. Otherwise the tool returns an `E07` error and the agent fixes it. A name the agent found on the web becomes a catalog item only through a page of your site.
- What the agent reads on the web goes to the model as well. mads does not fetch those pages itself. The CLI's own search tool does.
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
