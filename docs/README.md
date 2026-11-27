# mads documentation

Find the page you need to run a real Google Ads account with mads.

mads is an open source tool for managing Google Ads accounts in production. You describe the business in `business.toml`. LLM agents plan the account and write the campaigns. Rust code enforces every Google limit and writes the files. You import them, let the campaigns run, and feed the reports back with `mads optimize`.

## Pick a door

- **Try it.** [Your first run](tutorials/first-run.md) takes 15 minutes and needs no Google Ads account.
- **Run a real account.** [Run a real account](tutorials/run-a-real-account.md) goes from billing to the second optimize cycle.
- **Look something up.** [CLI](reference/cli.md), [business.toml](reference/business-toml.md) and [Exit codes and errors](reference/exit-codes-and-errors.md) hold the exact values.
- **Understand it.** [Why mads](explanation/why-mads.md) and [Safe on a live account](explanation/safe-on-a-live-account.md) explain what it does and what it guards.

The docs follow four kinds of page. Tutorials are lessons with a result. How-to pages are steps for one job. Reference pages are tables of exact values. Explanation pages say why.

## All pages

### Tutorials

- [Your first run](tutorials/first-run.md)
- [Run a real account](tutorials/run-a-real-account.md)

### Set up

- [Install mads](how-to/install.md)
- [Prepare your Google Ads account](how-to/prepare-google-ads.md)
- [Choose a provider](how-to/choose-a-provider.md)
- [Run with Ollama](how-to/run-with-ollama.md)
- [Use an OpenAI-compatible server](how-to/use-an-openai-compatible-server.md)

### Describe your business

- [Draft the files from your site](how-to/draft-from-your-site.md)
- [Write the business file](how-to/write-the-business-file.md)
- [Write a catalog](how-to/write-a-catalog.md)
- [Pick a campaign strategy](how-to/pick-a-strategy.md)
- [Add image and app campaigns](how-to/add-image-campaigns.md)
- [Handle restricted categories](how-to/handle-restricted-categories.md)

### Launch

- [Review a run](how-to/review-a-run.md)
- [Import into Google Ads Editor](how-to/import-into-google-ads-editor.md)
- [Upload the bulk CSVs](how-to/upload-bulk-csvs.md)
- [Go live](how-to/go-live.md)

### Improve

- [Export Google Ads reports](how-to/export-google-ads-reports.md)
- [Optimize a live account](how-to/optimize-a-live-account.md)

### Automate

- [Run in Docker](how-to/run-in-docker.md)
- [Run in GitHub Actions](how-to/run-in-github-actions.md)
- [Use the ads automation package](how-to/use-the-automation-package.md)

### Fix

- [Resume or re-export a run](how-to/resume-a-run.md)
- [Keep token cost down](how-to/keep-cost-down.md)
- [Troubleshoot](how-to/troubleshoot.md)

### Reference

- [CLI](reference/cli.md)
- [business.toml](reference/business-toml.md)
- [catalog.csv](reference/catalog-csv.md)
- [Validation rules](reference/validation-rules.md)
- [Exit codes and errors](reference/exit-codes-and-errors.md)
- [Output files](reference/output-files.md)
- [report.md](reference/report-md.md)
- [Google Ads Editor file](reference/editor-file.md)
- [Bulk upload format](reference/bulk-upload-format.md)
- [Ads automation package](reference/automation-package.md)
- [Google Ads reports](reference/google-ads-reports.md)
- [Providers](reference/providers.md)
- [Run limits](reference/limits.md)
- [Environment variables](reference/environment-variables.md)
- [MCP tools](reference/mcp-tools.md)
- [Limitations](reference/limitations.md)
- [Glossary](reference/glossary.md)

### Explanation

- [Why mads](explanation/why-mads.md)
- [Safe on a live account](explanation/safe-on-a-live-account.md)
- [How a run works](explanation/how-a-run-works.md)
- [How optimize decides](explanation/how-optimize-decides.md)
- [Campaign strategies](explanation/strategies.md)
- [Image and app campaigns](explanation/image-campaigns.md)
- [Restricted categories](explanation/restricted-categories.md)
- [Agent CLIs](explanation/agent-clis.md)
- [Limits and retries](explanation/limits-and-retries.md)
- [Architecture](explanation/architecture.md)

### Project

- [Sponsors](sponsors.md)

## Next

- [Your first run](tutorials/first-run.md)
- [Install mads](how-to/install.md)
