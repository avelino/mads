# Getting started

This page installs mads and runs it once, end to end, so you finish with five CSV files and a report.

## Install

mads installs from source for now. You need `rustup`. The repository pins its toolchain, so the first build installs the right Rust version.

```bash
git clone https://github.com/avelino/mads
cd mads
cargo install --path crates/mads-cli
```

Check the install.

```bash
mads --version
```

```text
mads 0.1.0
```

## Pick a provider

mads needs an LLM. List the providers and see which ones are ready.

```bash
mads providers
```

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
```

`ollama` and `openai-compat` show `ready` because they need no key. That does not mean a server is running. A CLI provider shows `ready` when its executable is in `PATH`.

Export the key of the provider you use, or log in to the CLI of a CLI provider. See [Agent CLIs](guides/agent-clis.md).

```bash
export ANTHROPIC_API_KEY=...
```

Every API provider needs `--model`. CLI providers do not. mads has no default model. Pass the model id your provider documents, or set `MADS_MODEL`. See [Providers](guides/providers.md).

## Draft your input from your site

Let an agent read your website and write `business.toml` and `catalog.csv`. The daily budget and the currency are flags, because a site does not say how much you want to spend.

```bash
mads init --from-url https://vinellu.com --daily-budget 50 --currency BRL --provider claude-cli
```

init writes `business.toml`, `catalog.csv` and `research.md` into the current folder, or into `--out-dir`. It refuses to overwrite existing files unless you pass `--force`. Read `research.md` first, then fix what is wrong in the other two before you go on. See [Init from a URL](guides/init-from-url.md).

Skip this step if you prefer to write the file yourself.

## Or describe your business by hand

Create a file named `business.toml`.

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

Each key in this file means something to the agents.

- `url` is the landing page for brand ads and the base of the allowed URL set.
- `language` is the language of every ad text. The CSV `Language` column gets the primary subtag (`pt`).
- `locations` takes exactly one entry. Use the name Google Ads uses.
- `goal` tells the agents what a conversion is.
- `description` is everything the agents are allowed to claim. They do not invent numbers or prices.
- `budget.daily` is split across campaigns and must add up to the cent.
- `budget.max_cpc` caps every CPC the agents choose.

The full list is in the [business.toml reference](reference/business-toml.md).

## Run it

```bash
mads generate business.toml --provider anthropic --model <model-id>
```

On a terminal you see one spinner per running mission. In a pipe or a log you see plain lines. Pass `--format plain` to get lines on a terminal too.

```text
12:28:50 [run] 20261001-122850-b51417 started (anthropic, <model-id>)
12:28:50 [plan] started (attempt 1)
12:28:50 [plan] > get_business
12:28:50 [plan] < ok get_business
12:28:50 [plan] finished
12:28:50 [run] cross-negatives: brand and competitor terms
12:28:50 [run] validate: checking limits and policies
12:28:52 [run] export: writing CSV files
12:28:52 [out] out/20261001-122850-b51417/google-ads/1-campaign.csv
12:28:52 [out] out/20261001-122850-b51417/report.md
```

This is an excerpt. A real run prints every tool call, one `tokens` line per turn and a line per campaign mission. Times, run id and counts differ.

What happens, in order.

1. mads validates `business.toml`. A bad file exits with code 2 before any model call.
2. The **plan** mission sets the brand kit and the campaign split.
3. One **campaign** mission per campaign builds ad groups, negatives and assets. Up to 4 run at once.
4. mads adds cross negatives, validates the whole account and requests every final URL.
5. mads writes the CSVs and `report.md` into `out/<run-id>/`.

The command exits `0` when the files are written. See [Troubleshooting](howto/troubleshooting.md) for the other exit codes.

## Read the result

```bash
ls out/*/google-ads
```

```text
1-campaign.csv
2-ad-groups.csv
3-keywords.csv
4-negative-keywords.csv
5-responsive-search-ads.csv
```

Open `out/<run-id>/report.md` first. It lists the budget split, every CPC with its rationale, validation warnings, the URL check and token usage. [Review and import](guides/review-and-import.md) explains each section.

## Upload

In Google Ads open Tools, Bulk actions, Uploads. Upload the five files in numeric order, preview, then apply. Campaigns arrive paused. Review them before you enable anything.

## Next

* Follow the [tutorial](tutorial.md) for a run with a catalog.
* Learn what goes in [the business file](guides/business-file.md).
