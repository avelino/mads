# Choose a provider

Pick the model provider mads will use, set its key or login, and choose the model for `mads generate`.

Every real run calls a model and spends your tokens. `mads providers` calls none.

## Before you start

- mads is installed. See [Install](install.md).
- You have one of these:
  - an API key for a hosted provider,
  - a coding agent CLI that is installed and logged in (`claude`, `codex` or `gemini`),
  - a local or self-hosted server, see [Run with Ollama](run-with-ollama.md) and [Use an OpenAI-compatible server](use-an-openai-compatible-server.md).
- A `business.toml`, only for the `generate` examples. `mads init` makes one. See [Write the business file](write-the-business-file.md).

## Steps

1. List the providers.

   ```bash
   mads providers
   ```

   Example output of `mads providers` with no key set and no agent CLI installed (no model was called).

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
   claude-cli       cli   `claude` not found in PATH
   codex-cli        cli   `codex` not found in PATH
   gemini-cli       cli   `gemini` not found in PATH

   IMAGE PROVIDER         STATUS
   gemini                 missing GEMINI_API_KEY
   openai                 missing OPENAI_API_KEY
   none                   ready
   ```

   The command always exits `0`. The second table lists image providers, which you choose separately in step 6.

2. Pick one row.

   | You have | Use | Needs |
   |---|---|---|
   | An API key | `anthropic`, `openai`, `gemini`, `openrouter`, `groq`, `deepseek` or `xai` | The key variable, and `--model`. |
   | An installed, logged-in agent CLI | `claude-cli`, `codex-cli` or `gemini-cli` | The CLI in `PATH`. `--model` is optional. |
   | A local Ollama server | `ollama` | The server running, and `--model`. See [Run with Ollama](run-with-ollama.md). |
   | Any OpenAI chat completions server | `openai-compat` | `--base-url` and `--model`. See [Use an OpenAI-compatible server](use-an-openai-compatible-server.md). |

   The model must support tool calling. A model that answers in prose and never calls a tool fails the mission with `no progress`.

3. Set the key or log in.

   For an API provider, export the variable from the table in [Providers](../reference/providers.md). The status changes from `missing <VAR>` to `ready`.

   ```bash
   export ANTHROPIC_API_KEY=<your-key>
   ```

   For an agent CLI, log in with the CLI itself. mads does not manage logins. `claude-cli` needs `claude` 2.1.259 or newer, and mads checks that before any mission runs. If the executable is not in `PATH`, point `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN` or `MADS_GEMINI_BIN` at it.

   ```bash
   export MADS_CLAUDE_BIN=/path/to/claude
   ```

4. Choose the model.

   API providers have no default model. Use a model id from your provider's documentation.

   ```bash
   mads generate business.toml --provider anthropic --model <model-id>
   ```

   For an agent CLI, leave `--model` out and the CLI uses its own default.

   ```bash
   mads generate business.toml --provider claude-cli
   ```

5. Use a stronger model for the plan, if you want one.

   The plan mission decides what every campaign depends on. `--plan-model` sets the model for that mission only, on the same provider. Without it the plan uses `--model`.

   ```bash
   mads generate business.toml \
     --provider anthropic \
     --model <campaign-model-id> \
     --plan-model <plan-model-id>
   ```

6. Set the image provider, only if the run has image campaigns.

   `--image-provider` is independent of `--provider`. `auto`, the default, picks the first image provider that has a key. See [Add image campaigns](add-image-campaigns.md).

7. Set your choice once.

   Each flag has a variable. A flag overrides its variable.

   ```bash
   export MADS_PROVIDER=anthropic
   export MADS_MODEL=<model-id>
   export MADS_PLAN_MODEL=<plan-model-id>
   mads generate business.toml
   ```

   For an agent CLI, `MADS_PROVIDER` is enough. For CI, see [Run in GitHub Actions](run-in-github-actions.md).

## Check it worked

Run the list again.

```bash
mads providers
```

Your provider shows `ready`. For `ollama` and `openai-compat`, `ready` only means no key is needed. mads does not check that a server is listening.

Selection errors exit with code `2` before any mission starts.

```text
error: no provider: use --provider or MADS_PROVIDER
error: --model (or MADS_MODEL) is required for anthropic
```

A missing key is different. The plan mission fails with `ApiKeyEnvNotFound` and the run exits with code `1`. The full list of errors is in [Providers](../reference/providers.md#errors). Fixes are in [Troubleshoot](troubleshoot.md).

## Next

- [Providers reference](../reference/providers.md)
- [Agent CLIs](../explanation/agent-clis.md)
- [Keep token cost down](keep-cost-down.md)
