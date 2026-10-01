# GitHub Actions

This page gives you a complete workflow that runs mads in CI, shows validation issues as annotations, posts the report to the job summary and uploads the run as an artifact.

## What mads does on Actions

When `GITHUB_ACTIONS` is `true`, `--format auto` picks the `github` format. You can also force it with `--format github` or `MADS_FORMAT=github`.

The `github` format is the `plain` format plus two things.

- An annotation for every validation issue. Errors print as `::error title=<code>::<path>: <message>` and warnings as `::warning title=<code>::<path>: <message>`.
- When the run finishes, mads appends `report.md` to the file in `$GITHUB_STEP_SUMMARY`.

```text
::error title=E05::campaigns[0].assets.callouts[4]: text contains avoided term 'reviews reais'
```

Progress lines go to stdout. Logs from `tracing` go to stderr and stay quiet unless you pass `-v`.

## The workflow

Put your input files in the repository, for example `ads/business.toml` and `ads/catalog.csv`.

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
      - uses: actions/checkout@v4

      - name: Install mads
        run: cargo install --git https://github.com/avelino/mads mads-cli --locked

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
          MADS_PLAN_MODEL: ${{ vars.MADS_PLAN_MODEL }}

      - name: Upload run
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: mads-run
          path: out/
          if-no-files-found: ignore
```

Notes on the file.

- `cargo install --git ... mads-cli` builds from source on the runner. Cache `~/.cargo` and the target directory with `actions/cache` if the build time matters to you.
- `--model` comes from `MADS_MODEL`. Set it as a repository variable under Settings, Secrets and variables, Actions, Variables. `MADS_PLAN_MODEL` is optional. Leave it unset to use `MADS_MODEL` for the plan too.
- `--max-tokens 2000000` caps spend. A looping model stops there. See [Cost and limits](cost-and-limits.md).
- `if: always()` uploads the run even when the job fails. A failed run has `report.md`, `workspace.json` and `events.ndjson`, which is what you need to debug it.
- The artifact has the whole `out/` directory. It contains your business description, the catalog and the generated ads. Do not upload it from a public repository if those are private.

## With claude-cli

Use an agent CLI when you want to bill a Claude subscription instead of API usage. The runner needs `claude` installed and a token.

Create the token once on your machine.

```bash
claude setup-token
```

Store it as the secret `CLAUDE_CODE_OAUTH_TOKEN`. Then use this workflow. It installs `claude` with npm (Node is on the runner image) and runs mads with `--provider claude-cli`.

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
      - uses: actions/checkout@v4

      - name: Install mads
        run: cargo install --git https://github.com/avelino/mads mads-cli --locked

      - name: Install claude
        run: npm install -g @anthropic-ai/claude-code

      - name: Generate campaigns
        run: mads generate ads/business.toml --provider claude-cli --out out
        env:
          CLAUDE_CODE_OAUTH_TOKEN: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}

      - name: Upload run
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: mads-run
          path: out/
          if-no-files-found: ignore
```

Notes.

- mads needs `claude` 2.1.259 or newer. It checks before the first mission and exits `2` with the required version when the CLI is older.
- With `ANTHROPIC_API_KEY` set instead of the OAuth token, mads runs claude in bare mode.
- Without `--model`, claude picks its own default model.
- The config files with the MCP token live in a temporary directory on the runner and are deleted after each mission. They are not in `out/`.
- `out/transcripts/<mission>.cli.jsonl` has the raw stream of each mission. Read it when a mission fails.
- This workflow has not been run on a hosted runner yet. Check the `claude` install command against the current Claude Code docs.

## Secrets

- Store the key as a repository or environment secret. The workflow reads it through `secrets`. mads reads the provider's standard variable (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `GROQ_API_KEY`, `DEEPSEEK_API_KEY`, `XAI_API_KEY`).
- mads does not write keys to the run directory. The temporary config files of agent CLIs live outside it.
- Pull requests from forks do not receive secrets. Run mads from `workflow_dispatch` or from a trusted branch.

## Exit codes

A step fails on any non-zero exit code, which is what you want.

| Code | Meaning | Typical fix |
|---|---|---|
| `0` | Success. CSVs written. | |
| `1` | Runtime failure. A provider error, a mission that failed after retries or the token budget. | Read the failed mission line. Re-run, or resume. |
| `2` | Bad input or usage. A bad `business.toml`, an unknown flag, a missing provider or model. | Fix the file or the flags. The message names the key. |
| `3` | The account is invalid. Validation errors or a failed URL check. No CSV written. | Read the annotations and `report.md`. |

See [Troubleshooting](../howto/troubleshooting.md).

## A schedule

You can run on a schedule when your input changes rarely and you want fresh drafts.

```yaml
on:
  schedule:
    - cron: "0 9 * * 1"
```

Each run produces a new directory. Nothing is uploaded to Google Ads. Someone has to review the artifact.

## Download and import

Download the `mads-run` artifact from the run page, open `report.md`, review the CSVs and upload them as in [Review and import](review-and-import.md).
