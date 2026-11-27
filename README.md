# mads

Run your Google Ads account from a text file. Open source, built for real accounts that spend real money.

You describe the business in `business.toml`. LLM agents plan the account and write the campaigns. Rust code enforces every limit Google sets, so an invalid account never becomes a file. After the campaigns run, mads reads your Google Ads reports and rebuilds the account from the numbers.

## Why you can point it at a live account

- **mads uploads nothing.** It writes files. You import them, so a human sees every change before it reaches Google Ads.
- **Campaigns arrive paused.** Nothing serves until you enable it.
- **One validation error and no file is written.** The run exits with code 3 and says why.
- **Every final URL is requested before export.** A landing page that does not answer is an error.
- **`optimize` pauses what it drops.** Nothing is deleted, and campaigns mads did not create are left alone.

The model decides. Code checks every decision. More in [Safe on a live account](docs/explanation/safe-on-a-live-account.md).

## The loop

```text
init -> generate -> import -> wait -> optimize -> import -> repeat
```

`init` drafts the input from your site. `generate` builds the account. You import it into Google Ads Editor and let it run. `optimize` reads the reports you export and writes the next version, with pause rows for what it dropped.

## Quick start

The image needs Docker and an API key. The repository has no release tag yet, so use `edge`, the last build of `main`. Once a release exists, pin its version to repeat a run exactly. These commands call a model and spend your tokens.

```bash
alias mads='docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -e ANTHROPIC_API_KEY ghcr.io/avelino/mads:edge'
mads init --from-url https://your-site.example --daily-budget 50 --currency BRL --provider anthropic --model <model-id>
# read research.md, then fix business.toml and catalog.csv
mads generate business.toml --provider anthropic --model <model-id>
```

Open `out/<run-id>/report.md`, then import `google-ads/editor/account.csv` into Google Ads Editor. No Docker? Build from source, or use an agent CLI such as `claude-cli` instead of an API key. See [Install mads](docs/how-to/install.md) and [Your first run](docs/tutorials/first-run.md).

## What you get

One run directory per `generate`: the CSVs for Google Ads Editor and for bulk upload, a `report.md` with budgets, bids, warnings and token usage, and the full transcript of every agent. See [Output files](docs/reference/output-files.md).

## Why "mads"

It stands for Manager Ads. Short and lowercase, like the binary.

## Docs

- [Your first run](docs/tutorials/first-run.md), 15 minutes, no Google Ads account needed.
- [Run a real account](docs/tutorials/run-a-real-account.md), from billing to the second optimize cycle.
- [Reference](docs/reference/cli.md) for flags, files, rules and exit codes.
- [Explanation](docs/explanation/why-mads.md) for how and why it works this way.

The full table of contents is in [docs/SUMMARY.md](docs/SUMMARY.md).

## Limits to know

- mads guarantees the format and the rules, not conversions.
- You import the files yourself. There is no upload.
- Search campaigns bid with manual CPC only, and bids and budget shares are estimates because mads has no auction data.

The complete list is in [Limitations](docs/reference/limitations.md).

## Sponsors

These companies support mads financially. Thank you.

<a href="https://vinellu.com"><img src="docs/assets/sponsors/vinellu.svg" alt="Vinellu" height="48"></a>&nbsp;&nbsp;&nbsp;
<a href="https://www.buser.com.br"><img src="docs/assets/sponsors/buser.svg" alt="Buser" height="32"></a>

Want to support mads too? See [Sponsors](docs/sponsors.md).

## Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you change anything. MIT license, see [LICENSE](LICENSE).
