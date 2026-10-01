# Providers

This page shows how to pick an LLM provider, set its key and choose models for `mads generate`.

## List them

```bash
mads providers
```

Each row shows the name, the kind and the status. The status is `ready`, `missing <ENV_VAR>` for an API provider with no key, or a "not found in PATH" message for an agent CLI whose executable is missing. The command always exits `0`.

`ready` for `ollama` and `openai-compat` only means they need no key. mads does not check that a server is listening. For `claude-cli`, `codex-cli` and `gemini-cli`, `ready` means the executable is in `PATH`.

## API providers

API providers go through the `genai` crate. mads reads the key from the provider's standard variable.

| Provider | Environment variable | Notes |
|---|---|---|
| `anthropic` | `ANTHROPIC_API_KEY` | |
| `openai` | `OPENAI_API_KEY` | |
| `gemini` | `GEMINI_API_KEY` | |
| `openrouter` | `OPENROUTER_API_KEY` | mads resolves this variable itself. |
| `groq` | `GROQ_API_KEY` | |
| `deepseek` | `DEEPSEEK_API_KEY` | |
| `xai` | `XAI_API_KEY` | |
| `ollama` | none | Local server at `http://localhost:11434`. See [Run with Ollama](../howto/ollama-local.md). |
| `openai-compat` | `MADS_API_KEY` (optional) | Needs `--base-url`. See [Use an OpenAI-compatible server](../howto/openai-compatible.md). |

A missing key fails the plan mission, not the command line parser.

```text
12:28:31 [plan] failed: Resolver error for model 'anthropic::claude-sonnet-4-5 (adapter: Anthropic)'.
Cause: ApiKeyEnvNotFound { env_name: "ANTHROPIC_API_KEY" }
```

The run then exits with code `1`.

## Choose the model

`--model` is required for every API provider. mads has no default, so it does not go stale when providers rename models. Use the model id from your provider's documentation.

```bash
mads generate business.toml --provider anthropic --model <model-id>
```

Without it you get this error and exit code `2`.

```text
error: --model (or MADS_MODEL) is required for anthropic
```

### Two models

`--plan-model` sets the model for the plan mission only. The plan mission makes the decisions that every campaign depends on. A stronger model there and a cheaper one for campaigns is a common split.

```bash
mads generate business.toml \
  --provider anthropic \
  --model <cheaper-model-id> \
  --plan-model <stronger-model-id>
```

Both models use the same provider. If you leave `--plan-model` out, the plan mission uses `--model`.

### Set it once

Every agent flag has an environment variable.

```bash
export MADS_PROVIDER=anthropic
export MADS_MODEL=<model-id>
export MADS_PLAN_MODEL=<stronger-model-id>
mads generate business.toml
```

A flag overrides its variable.

## Custom endpoint

`openai-compat` talks to any server that implements the OpenAI chat completions API. Pass the endpoint with `--base-url` (or `MADS_BASE_URL`). The URL ends at the version segment.

```bash
mads generate business.toml \
  --provider openai-compat \
  --base-url http://localhost:8000/v1 \
  --model <model-id>
```

`MADS_API_KEY` is sent as the key when set. Without it mads sends a placeholder. `--base-url` is ignored by every other provider.

```text
error: --base-url (or MADS_BASE_URL) is required for openai-compat
```

## What the model needs

mads sends tool definitions and expects tool calls back. Pick a model that supports tool calling. A model that answers in prose and never calls a tool gets nudged twice with "Continue using the tools. Call finish when the work is done." and then the mission fails with `no progress`.

mads asks for at most 8192 output tokens per response. That is the highest value groq and deepseek accept.

## Retries

A rate limit (429), a timeout (408), a server error (5xx) or a network failure counts as transient. mads tries up to 3 times, waiting 1 second and then 2 seconds. Any other error is final for that attempt and the mission fails. Mission-level retries are separate. See [Cost and limits](cost-and-limits.md).

## Cost

API providers report input and output tokens. mads does not ship a price table, so the cost shows as `n/a`. Multiply the tokens by your provider's prices.

## Agent CLIs

`claude-cli`, `codex-cli` and `gemini-cli` run a coding agent you have installed. mads gives it the mission tools through a local MCP server and turns its built-in tools off. They use the CLI's own login, so no key variable applies, and `--model` is optional. See [Agent CLIs](agent-clis.md).

```bash
mads generate business.toml --provider claude-cli
```

The CLI executable can be overridden with `MADS_CLAUDE_BIN`, `MADS_CODEX_BIN` and `MADS_GEMINI_BIN`.

## Errors

```text
error: no provider: use --provider or MADS_PROVIDER
error: unknown provider 'gpt-9000'; run `mads providers` to see the options
```

Both exit with code `2`.
