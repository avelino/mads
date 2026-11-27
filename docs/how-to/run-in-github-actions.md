# Run in GitHub Actions

Run `mads generate` from a GitHub Actions workflow, see validation issues as annotations, read the report in the job summary and download the run as an artifact.

This workflow calls a model. Every run spends your provider tokens, so start it by hand with `workflow_dispatch` before you put it on a schedule.

## Before you start

- A repository that holds your input files, for example `ads/business.toml` and `ads/catalog.csv`. See [Write the business file](write-the-business-file.md) and [Write a catalog](write-a-catalog.md).
- A provider and its key. See [Choose a provider](choose-a-provider.md) and the key variables in [Environment variables](../reference/environment-variables.md#provider-keys).
- A way to run mads on the runner. This page uses the container image. [Run in Docker](run-in-docker.md) explains the image and its tags.

## What mads does on Actions

When `GITHUB_ACTIONS` is `true`, `--format auto` picks the `github` format. You can force it with `--format github` or `MADS_FORMAT=github`.

The `github` format is the `plain` format plus two things.

- One annotation for every validation issue. Errors print as `::error title=<code>::<path>: <message>` and warnings as `::warning title=<code>::<path>: <message>`.
- When the run finishes, mads appends `report.md` to the file named by `GITHUB_STEP_SUMMARY`.

Progress lines go to stdout. Logs from `tracing` go to stderr and stay quiet unless you pass `-v`.

## Run with an API provider

1. Create a repository variable `MADS_MODEL` with the model id. Go to Settings, Secrets and variables, Actions, Variables. An API provider needs a model, and without one mads exits `2` with `--model (or MADS_MODEL) is required for <provider>`. To use another model for the plan mission, add a `MADS_PLAN_MODEL` line to the env block of the generate step. Do not pass it empty. An empty `MADS_PLAN_MODEL` is read as a model named with an empty string, and the plan mission fails. Without it the plan uses `MADS_MODEL` too.

2. Create a repository secret `ANTHROPIC_API_KEY`, or the key your provider reads.

3. Add `.github/workflows/ads.yml`.

   ```yaml
   name: ads

   on:
     workflow_dispatch:
       inputs:
         business:
           description: Path to business.toml
           default: ads/business.toml

   permissions:
     contents: read

   jobs:
     generate:
       runs-on: ubuntu-latest
       timeout-minutes: 60
       steps:
         - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1

         - name: Install mads
           run: |
             docker pull ghcr.io/avelino/mads:edge
             sudo tee /usr/local/bin/mads >/dev/null <<'SH'
             #!/bin/sh
             exec docker run --rm \
               --user "$(id -u):$(id -g)" \
               -v "$PWD:/work" \
               -v "$GITHUB_STEP_SUMMARY:$GITHUB_STEP_SUMMARY" \
               -e GITHUB_ACTIONS -e GITHUB_STEP_SUMMARY -e MADS_FORMAT \
               -e MADS_PROVIDER -e MADS_MODEL -e MADS_PLAN_MODEL -e MADS_BASE_URL -e MADS_API_KEY \
               -e MADS_IMAGE_PROVIDER -e MADS_IMAGE_MODEL \
               -e ANTHROPIC_API_KEY -e OPENAI_API_KEY -e GEMINI_API_KEY -e OPENROUTER_API_KEY \
               -e GROQ_API_KEY -e DEEPSEEK_API_KEY -e XAI_API_KEY \
               ghcr.io/avelino/mads:edge "$@"
             SH
             sudo chmod +x /usr/local/bin/mads

         - name: Check providers
           run: mads providers

         - name: Generate campaigns
           run: >
             mads generate "$BUSINESS"
             --provider anthropic
             --out out
             --max-tokens 2000000
           env:
             BUSINESS: ${{ inputs.business }}
             ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
             MADS_MODEL: ${{ vars.MADS_MODEL }}

         - name: Upload run
           if: always()
           uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
           with:
             name: mads-run
             path: out/
             if-no-files-found: ignore
   ```

4. Start it from the Actions tab with Run workflow.

What the file does.

- The `mads` wrapper script forwards `GITHUB_ACTIONS` and `GITHUB_STEP_SUMMARY` into the container. `GITHUB_ACTIONS` selects the `github` format (without it mads falls back to `plain`), and `GITHUB_STEP_SUMMARY` is where the report is appended (without it the job summary stays empty). It also runs the container as your runner user, so `out/` is not owned by root.
- `mads providers` only prints which providers are ready. It exits `0` whatever the statuses say, so it never fails the job.
- The image tag is `edge`. The `latest` and version tags exist only after a `v*` git tag is pushed. Pin a tag or a digest (`ghcr.io/avelino/mads@sha256:...`) for reproducible runs. [Run in Docker](run-in-docker.md) lists what is published.
- `--max-tokens 2000000` caps spend. Without the flag the cap is 4000000 tokens. See [Keep cost down](keep-cost-down.md).
- `if: always()` uploads the run even when the job fails. A failed run still has `report.md`, `workspace.json` and `events.ndjson`, which is what you need to debug it.
- The two `uses:` lines pin the same action commits as the mads repository's own CI.

## Run with claude-cli

Use an agent CLI to bill a Claude subscription instead of API usage. The runner needs `claude` and a token. See [Agent CLIs](../explanation/agent-clis.md) for what mads does with the CLI.

The published image does not contain `claude`, so this workflow installs mads on the runner. This workflow has not been run on a hosted runner. Check the first run before you rely on it.

1. Create a token on your machine.

   ```bash
   claude setup-token
   ```

   The command prints a one-year token and does not save it. It needs a Pro, Max, Ultra, Team or Enterprise plan. See [Claude Code authentication](https://code.claude.com/docs/en/authentication#generate-a-long-lived-token).

2. Store the token as the repository secret `CLAUDE_CODE_OAUTH_TOKEN`.

3. Add `.github/workflows/ads-claude.yml`.

   ```yaml
   name: ads-claude

   on:
     workflow_dispatch:

   permissions:
     contents: read

   jobs:
     generate:
       runs-on: ubuntu-latest
       timeout-minutes: 60
       steps:
         - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1

         - name: Install mads
           run: cargo install --git https://github.com/avelino/mads mads-cli --locked

         - name: Install claude
           run: |
             curl -fsSL https://claude.ai/install.sh | bash
             echo "$HOME/.local/bin" >> "$GITHUB_PATH"

         - name: Generate campaigns
           run: mads generate ads/business.toml --provider claude-cli --out out
           env:
             CLAUDE_CODE_OAUTH_TOKEN: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}

         - name: Upload run
           if: always()
           uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
           with:
             name: mads-run
             path: out/
             if-no-files-found: ignore
   ```

   `cargo install` from a clone-free git URL needs Rust 1.88 or newer on the runner. The native installer puts `claude` in `~/.local/bin`, per [Claude Code setup](https://code.claude.com/docs/en/setup#install-claude-code).

What to know.

- mads needs `claude` 2.1.259 or newer. It checks before the first mission and exits `2` with the required version when the CLI is older.
- With `ANTHROPIC_API_KEY` set instead of the OAuth token, mads runs claude in bare mode. Bare mode does not read `CLAUDE_CODE_OAUTH_TOKEN`.
- Without `--model`, claude picks its own default model.
- The config files with the MCP token live in a temporary directory on the runner and are deleted after each mission. They are not in `out/`.
- `out/<run>/transcripts/<mission>.cli.jsonl` has the raw stream of each mission. Read it when a mission fails.

## Keep secrets and cost in check

- Store keys as repository or environment secrets and read them through `secrets`. mads does not write keys to the run directory.
- An `OPENAI_API_KEY` or `GEMINI_API_KEY` in the job also gives the run an image model, because `--image-provider` defaults to `auto`. With a logo, the plan agent then plans an image campaign by default. Set `MADS_IMAGE_PROVIDER: none` to keep the run on Search, or cap the pictures with `--max-images`. See [Keep cost down](keep-cost-down.md).
- Pull requests from forks do not receive secrets, except `GITHUB_TOKEN`. Run mads from `workflow_dispatch` or from a trusted branch. See [GitHub's page on secrets](https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions).
- The artifact holds the whole `out/` directory, with your business description, your catalog and the generated ads. Do not upload it from a public repository if those are private.

## Run on a schedule

Add a `schedule` trigger next to `workflow_dispatch` when your input changes rarely and you want fresh drafts.

```yaml
on:
  workflow_dispatch:
  schedule:
    - cron: "0 9 * * 1"
```

Every run starts from a fresh checkout, so the result lives only in the artifact. mads never uploads anything to Google Ads. Someone has to download and review each run.

## Check it worked

1. Open the run page. The job has one annotation for each validation issue, if there were any.
2. Open the job summary. It holds the content of `report.md`.
3. Download the `mads-run` artifact. It holds one `<run>/` folder with `report.md`, `workspace.json`, `events.ndjson` and `google-ads/` with the CSVs, as in [Output files](../reference/output-files.md).

A step fails on any non-zero exit code. Exit `1` is a runtime failure, `2` is bad input or usage and `3` is an invalid account with no CSV written. Each message and its fix is in [Exit codes and errors](../reference/exit-codes-and-errors.md).

## Next

- [Review a run](review-a-run.md)
- [Import into Google Ads Editor](import-into-google-ads-editor.md)
- [Keep cost down](keep-cost-down.md)
