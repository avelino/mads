# Run with Ollama

Run `mads generate` against a local Ollama server, so the run needs no API key and sends no prompt to a hosted provider.

mads sends the run's prompts to whatever model you name. A local model costs you compute time, not API tokens. It also means the quality of the plan depends on the model you pull.

## Before you start

- Ollama is installed and running on the same machine. `--provider ollama` talks to `http://localhost:11434`.
- You pulled a model that returns tool calls. Every mission works through tools, so a model that answers in plain text fails with `no progress`. See [Providers](../reference/providers.md#what-an-api-provider-call-does).
- You have a `business.toml`. See [Write the business file](write-the-business-file.md).
- You built or installed `mads`. See [Install](install.md).

## Steps

1. Pull the model. Replace `<model-name>` with the name you use in Ollama.

   ```bash
   ollama pull <model-name>
   ```

2. Run `generate` with the `ollama` provider and the same model name. `--model` is required for every API provider.

   ```bash
   mads generate business.toml --provider ollama --model <model-name>
   ```

3. Give the plan mission a stronger model if the plan comes out thin. The plan mission is the first step of every run, and the campaigns follow it. `--plan-model` sets the model for that mission only.

   ```bash
   mads generate business.toml --provider ollama --model <small-model> --plan-model <larger-model>
   ```

4. To reach Ollama on another machine, use `openai-compat`. `--provider ollama` always uses `localhost`. The base URL ends at the version segment.

   ```bash
   mads generate business.toml \
     --provider openai-compat \
     --base-url http://<host>:11434/v1 \
     --model <model-name>
   ```

   The full recipe is in [Use an OpenAI-compatible server](use-an-openai-compatible-server.md).

## Check it worked

Run `mads providers`. The `ollama` row says `ready`.

Example output (real binary, `mads providers` calls no model), the `ollama` row:

```text
ollama           api   ready
```

`ready` only means mads needs no key for this provider. It does not test the connection. The proof is the run. It ends with a `report.md` in the run directory, and the exit code is `0`. See [Output files](../reference/output-files.md) for what the directory holds.

If the server is not running, the plan mission fails with the connection error and the run exits `1`. mads retries a network failure three times inside one call, then fails the mission. Start Ollama and continue the same run with `mads generate --resume <run-dir>` and the same provider flags. See [Resume or re-export a run](resume-a-run.md).

If a mission fails with `max turns` or `no progress`, the model did not finish the work with tools. Use a larger model, raise `--max-turns` (default `40`) or set `--plan-model`. The reasons are in [Limits and retries](../explanation/limits-and-retries.md). A model that breaks the ad text limits shows up as validation errors. The limits are in [Limits](../reference/limits.md).

A local run reports tokens and shows cost as `n/a`.

## Next

- [Choose a provider](choose-a-provider.md)
- [Keep token cost down](keep-cost-down.md)
- [Troubleshoot](troubleshoot.md)
