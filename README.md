# mads

Describe your business in a TOML file. mads asks an LLM to plan the account and build the campaigns, checks every limit Google enforces, and writes the CSV files you upload in Google Ads.

LLM agents make the marketing decisions. Deterministic Rust code enforces the rules and writes the files. An invalid account never reaches a CSV.

## What it does

- Studies your business with `mads init --from-url`: reads the site, searches the web for demand, ranks campaign ideas by expected return and drafts `business.toml`, `catalog.csv` and `research.md`. Or you write the files yourself.
- Reads `business.toml` and an optional `catalog.csv` (the products, labels or pages people search for by name).
- Runs one plan mission and one mission per campaign. Campaigns run in parallel.
- Picks a format per campaign: Search, Demand Gen, Performance Max or App installs. Image campaigns get generated pictures (Gemini or OpenAI), your real logo and real product photos as references. Or you choose the formats in `business.toml`. See [Image campaigns](docs/guides/image-campaigns.md).
- Validates 24 error rules and 9 warning rules: text lengths, `!` in headlines, URLs outside your site, negatives that block your own keywords, budgets that do not add up, picture counts and sizes.
- Adds brand and competitor terms as negatives to the campaigns they do not belong to.
- Detects Google Ads restricted categories (alcohol, gambling, healthcare, financial services, political, sexual content) during `init`, keeps sales language out of keywords and ads, and writes the exception request into the report. See [Restricted categories](docs/guides/restricted-categories.md).
- Learns from a live account with `mads optimize`: reads the search terms, keywords and campaigns reports you export from Google Ads and rebuilds the account from the numbers, with an Editor file that pauses what it drops. See [Optimize a live account](docs/guides/optimize.md).
- Exports the Google Ads bulk upload CSVs, a Google Ads Editor CSV with the pictures for image campaigns, and a `report.md` with budgets, bids, warnings and token usage.
- Runs unattended in CI with GitHub annotations and a step summary.

## Quick start

Install from source. The repository pins its Rust toolchain, so `rustup` picks the right one.

```bash
git clone https://github.com/avelino/mads
cd mads
cargo install --path crates/mads-cli
mads providers
```

Or use the container. No Rust needed. See [Docker](docs/guides/docker.md).

```bash
docker run --rm ghcr.io/avelino/mads:latest --version
```

Let an agent read your site and draft the two input files. The budget is a flag because a site does not say how much you want to spend.

```bash
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider claude-cli
```

Read `research.md` to see what the agent found. Then review `business.toml` and `catalog.csv`. Fix the goal, the competitors and anything the site got wrong. See [Init from a URL](docs/guides/init-from-url.md).

Then generate the campaigns.

```bash
mads generate business.toml --provider claude-cli
```

`claude-cli` uses your installed `claude` and its login. With an API key, use an API provider.

```bash
export ANTHROPIC_API_KEY=...
mads generate business.toml --provider anthropic --model <model-id>
```

You can also write `business.toml` by hand. The smallest file looks like this.

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
max_cpc = 3.0
```

Read `out/<run-id>/report.md`, review the files in `out/<run-id>/google-ads/`, then upload them in Google Ads under Tools, Bulk actions, Uploads. Upload in numeric order. Campaigns arrive paused.

The [tutorial](docs/tutorial.md) walks through a full run with a catalog.

## What you get

One run directory per `mads generate`.

```text
out/20261001-122850-b51417/
  google-ads/
    1-campaign.csv
    2-ad-groups.csv
    3-keywords.csv
    4-negative-keywords.csv
    5-responsive-search-ads.csv
  report.md
  workspace.json
  events.ndjson
  run.json
  input/
  transcripts/
```

A slice of `3-keywords.csv`. Keywords are name variants crossed with intent modifiers in phrase match, then the variants again in exact match.

```csv
Row Type,Action,Keyword status,Campaign,Ad group,Keyword,Type
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu app baixar,Phrase match
Keyword,Add,Enabled,Vinellu - Marca,marca-vinellu,vinellu,Exact match
```

A slice of `report.md`.

```markdown
| Campaign | Intent | Daily budget | Bidding | Ad groups | Keywords |
|---|---|---|---|---|---|
| Vinellu - Marca | brand | 10,00 BRL | Manual CPC | 1 | 8 |
| Vinellu - Rotulos | catalog | 40,00 BRL | Manual CPC | 4 | 63 |
```

The CSVs follow the format of a real Google Ads export: CRLF line endings, UTF-8 without BOM, comma decimals for `pt`, `es`, `fr`, `de` and `it`, and parents referenced by name. See [the bulk upload format](docs/howto/google-ads-bulk-upload-format.md).

## How it works

- **Input.** `business.toml` and `catalog.csv` are validated first. Unknown keys fail fast with exit code 2.
- **Plan mission.** An agent sets the brand kit (shared headlines and descriptions) and splits the budget into 1 to 5 campaigns by intent: brand, catalog, generic, competitor.
- **Campaign missions.** One agent per campaign builds ad groups. It sends a keyword spec (variants and modifiers) and mads expands it. Every tool call is validated before it changes state.
- **Post-processing.** mads adds cross negatives, validates the whole account, checks every final URL and exports the CSVs.
- **Review.** You read `report.md`. Budgets and CPCs are estimates. mads has no auction data.

Details in [How campaigns are built](docs/guides/how-campaigns-are-built.md).

## Providers

| Provider | Kind | Credentials |
|---|---|---|
| `anthropic` | API | `ANTHROPIC_API_KEY` |
| `openai` | API | `OPENAI_API_KEY` |
| `gemini` | API | `GEMINI_API_KEY` |
| `openrouter` | API | `OPENROUTER_API_KEY` |
| `groq` | API | `GROQ_API_KEY` |
| `deepseek` | API | `DEEPSEEK_API_KEY` |
| `xai` | API | `XAI_API_KEY` |
| `ollama` | API, local | none |
| `openai-compat` | API, custom endpoint | `MADS_API_KEY` (optional) and `--base-url` |
| `claude-cli`, `codex-cli`, `gemini-cli` | agent CLI | the CLI's own login. `--model` is optional. See [Agent CLIs](docs/guides/agent-clis.md) |

`--model` is required for every API provider. Run `mads providers` to see which ones are ready. More in [Providers](docs/guides/providers.md).

## GitHub Actions

```yaml
name: ads
on:
  workflow_dispatch:

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Generate campaigns
        run: |
          docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" \
            -v "$GITHUB_STEP_SUMMARY:$GITHUB_STEP_SUMMARY" \
            -e GITHUB_ACTIONS -e GITHUB_STEP_SUMMARY -e ANTHROPIC_API_KEY -e MADS_MODEL \
            ghcr.io/avelino/mads:latest \
            generate ads/business.toml --provider anthropic --out out
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
          MADS_MODEL: ${{ vars.MADS_MODEL }}
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: mads-run
          path: out/
```

To use `claude-cli` on a runner, install `claude` and set `CLAUDE_CODE_OAUTH_TOKEN` (create it with `claude setup-token`) or `ANTHROPIC_API_KEY`. See [GitHub Actions](docs/guides/github-actions.md#with-claude-cli).

On GitHub Actions `--format auto` picks `github`. Validation issues become annotations and `report.md` lands in the step summary. See [GitHub Actions](docs/guides/github-actions.md).

## Limitations

- **Search, Performance Max and Demand Gen.** No video, Shopping, Display-only, Dynamic Search Ads or broad match. Image campaigns need an image model key and a logo, see [Image campaigns](docs/guides/image-campaigns.md).
- **Image campaigns go through Google Ads Editor, and pictures are attached by hand.** `editor/account.csv` holds the whole account in Editor's own format (Search and App rows checked against a real export), but Editor does not import the link between an ad and its images. Search, App and Demand Gen imported into Editor 2.13.3 with no error. Performance Max rows have not gone through a real import yet.
- **One location per account.** `business.locations` takes exactly one entry.
- **Search bids with `manual_cpc` only.** `maximize_clicks` and `maximize_conversions` on Search return `UNSUPPORTED` until the Google bulk templates for them are verified. Performance Max uses `maximize_conversions` and Demand Gen `maximize_clicks` or `maximize_conversions`.
- **Campaign negatives are expanded.** The campaign-level negative list is exported as ad group negatives in every ad group, not as campaign-level rows.
- **No asset CSVs yet.** Sitelinks, callouts and structured snippets are collected and validated, but files 6 to 8 are not written until their templates are verified.
- **Bids and budget shares are estimates.** mads has no Keyword Planner or auction data.
- **Latin scripts.** Length checks count Unicode characters. Google counts some CJK characters as 2.
- **`codex-cli` and `gemini-cli` are untested against the real CLIs.** Their stream parsers follow the documented formats and run against recorded fixtures. `claude-cli` ran for real.
- **`init` reads HTML only.** No JavaScript runs, so a site that renders in the browser gives the agent little to read.
- **No upload.** mads writes files. You upload them yourself.

## Documentation

Start at [docs/](docs/README.md). The table of contents is in [docs/SUMMARY.md](docs/SUMMARY.md).

## Sponsors

These companies support mads financially. Thank you.

<a href="https://vinellu.com"><img src="docs/assets/sponsors/vinellu.svg" alt="Vinellu" height="48"></a>&nbsp;&nbsp;&nbsp;
<a href="https://www.buser.com.br"><img src="docs/assets/sponsors/buser.svg" alt="Buser" height="32"></a>

Want to support mads too? Open an issue. See [Sponsors](docs/sponsors.md).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). The gate before any change is done.

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## License

MIT. See [LICENSE](LICENSE).
