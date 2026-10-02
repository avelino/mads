# mads documentation

mads turns a business description into Google Ads bulk upload CSVs. Read this page to find the doc you need.

You write `business.toml` and, optionally, `catalog.csv`. LLM agents plan the account and build the campaigns. Rust code validates every limit and writes the files. You review `report.md`, upload the CSVs and enable the campaigns yourself.

## Start here

* **[Why mads](why-mads.md)** covers the problem and what mads does and does not do.
* **[Getting started](getting-started.md)** installs mads and runs it end to end.
* **[Tutorial](tutorial.md)** goes from an empty folder to uploaded campaigns, using a wine app as the example.

## Guides

* **[The business file](guides/business-file.md)** explains how to describe your business so the agents write good ads.
* **[The catalog](guides/catalog.md)** explains how to list the things people search for by name.
* **[Providers](guides/providers.md)** covers the API providers, models and keys.
* **[Agent CLIs](guides/agent-clis.md)** covers `claude-cli`, `codex-cli` and `gemini-cli`, which use a coding agent you have installed.
* **[Init from a URL](guides/init-from-url.md)** covers `mads init --from-url`, which studies your business and drafts the input files and research notes.
* **[How campaigns are built](guides/how-campaigns-are-built.md)** explains intents, ad groups, keyword specs, negatives, ads and what the validator enforces.
* **[Image campaigns](guides/image-campaigns.md)** explains Performance Max and Demand Gen: formats, logo, generated pictures and the Google Ads Editor import.
* **[Restricted categories](guides/restricted-categories.md)** explains how mads handles alcohol, gambling, healthcare and other subjects Google Ads restricts, and how to ask Google for an exception.
* **[Review and import](guides/review-and-import.md)** explains how to read `report.md` and upload the files.
* **[Resume and export](guides/resume-and-export.md)** explains how to recover a failed run and re-export without a model.
* **[Cost and limits](guides/cost-and-limits.md)** covers turns, timeouts, token budgets and retries.
* **[GitHub Actions](guides/github-actions.md)** has a complete workflow.

## How-to

* **[Troubleshooting](howto/troubleshooting.md)** lists every exit code and real error messages.
* **[Run with Ollama](howto/ollama-local.md)**
* **[Use an OpenAI-compatible server](howto/openai-compatible.md)**
* **[Google Ads bulk upload format](howto/google-ads-bulk-upload-format.md)** describes the five CSV files of Search campaigns.

## Reference

* **[CLI reference](reference/cli.md)**
* **[business.toml reference](reference/business-toml.md)**
* **[catalog.csv reference](reference/catalog-csv.md)**
* **[Validation rules](reference/validation-rules.md)**
* **[Output files](reference/output-files.md)**
* **[Environment variables](reference/environment-variables.md)**
* **[Architecture](reference/architecture.md)** also shows how to embed `mads-core` as a library.
* **[MCP tools](reference/mcp-tools.md)** lists the tools the agents call.

## Sponsors

* **[Sponsors](sponsors.md)** lists the companies that fund mads: Vinellu and Buser.
