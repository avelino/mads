# Use an OpenAI-compatible server

Run `mads generate` against any server that speaks the OpenAI chat completions API, such as vLLM, LM Studio, LiteLLM or a hosted gateway.

## Before you start

- A `business.toml` for your business. See [Write the business file](write-the-business-file.md).
- A server that supports tool calling. Every mission sends tool definitions and expects `tool_calls` back. A server that ignores `tools` fails the mission with `no progress`.
- The server's base URL. It ends at the version segment, for example `http://localhost:8000/v1`. A trailing slash is optional.
- A model id the server knows.
- A key, if the server asks for one.

A real run sends requests to your server. If the server is a hosted gateway, it spends your tokens there.

## Steps

1. Set the key if the server needs one.

   ```bash
   export MADS_API_KEY=your-key
   ```

   Without `MADS_API_KEY`, or with an empty value, mads sends the placeholder `none`. Most local servers ignore it.

2. Run `generate` with the provider, the base URL and the model.

   ```bash
   mads generate business.toml \
     --provider openai-compat \
     --base-url http://localhost:8000/v1 \
     --model your-model-id
   ```

   mads posts to `<base-url>/chat/completions`. Each response asks for 8192 output tokens, so the server must accept that limit.

3. Optional. Use a smaller model for the plan mission.

   ```bash
   mads generate business.toml \
     --provider openai-compat \
     --base-url http://localhost:8000/v1 \
     --model your-model-id \
     --plan-model your-plan-model-id
   ```

   `--plan-model` defaults to `--model`. Both models go to the same base URL with the same key.

4. Optional. Keep the settings in the environment instead of flags.

   ```bash
   export MADS_PROVIDER=openai-compat
   export MADS_BASE_URL=http://localhost:8000/v1
   export MADS_MODEL=your-model-id
   mads generate business.toml
   ```

   A flag overrides its variable. The full list is in [Environment variables](../reference/environment-variables.md).

## Check it worked

`mads providers` lists `openai-compat` as `ready`. That status does not prove a server is listening. It only says the provider needs no key check.

A run that reached your server prints progress lines as it works and writes the run directory under `out/`. A run where every mission finished exits `0`.

Two settings are checked before any request leaves your machine. `--model` is checked first.

```text
error: --model (or MADS_MODEL) is required for openai-compat
```

```text
error: --base-url (or MADS_BASE_URL) is required for openai-compat
```

Both exit `2`. Output above comes from running the binary without a model call.

If the server is down or the path is wrong, the plan mission fails after its retries and the run exits `1`. Network failures, HTTP 408, 429 and 5xx are retried up to 3 attempts per call. Other HTTP errors fail at once. The failure line shows the URL mads called. The reasons and fixes are in [Troubleshoot a run](troubleshoot.md) and [Exit codes and errors](../reference/exit-codes-and-errors.md).

## Next

- [Choose a provider](choose-a-provider.md)
- [Providers](../reference/providers.md)
- [Run with Ollama](run-with-ollama.md)
