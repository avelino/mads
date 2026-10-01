# Use an OpenAI-compatible server

This page shows how to point mads at any server that speaks the OpenAI chat completions API, such as vLLM, LM Studio, LiteLLM or a hosted gateway.

## Run it

```bash
mads generate business.toml \
  --provider openai-compat \
  --base-url http://localhost:8000/v1 \
  --model <model-id>
```

mads sends requests to `<base-url>/chat/completions`. The base URL ends at the version segment. A trailing slash is optional.

## API key

Set `MADS_API_KEY` when the server needs a key.

```bash
export MADS_API_KEY=...
```

Without it, mads sends the placeholder `none`. Most local servers ignore it.

## Settings through the environment

```bash
export MADS_PROVIDER=openai-compat
export MADS_BASE_URL=http://localhost:8000/v1
export MADS_MODEL=<model-id>
```

## Requirements

- The server supports tool calling (the `tools` field and `tool_calls` in the response).
- The model id is one the server knows.
- The server accepts an output limit of 8192 tokens per response.

## When it fails

A server that is down shows up as a failed plan mission and exit code `1`.

```text
12:28:43 [plan] failed: Web call failed for model 'llama3.1 (adapter: OpenAI)'.
Cause: Reqwest error: error sending request for url (http://localhost:11434/v1/chat/completions)
```

Check the URL in the message. A wrong path (for example a missing `/v1`) shows there.

Missing flags exit with code `2`.

```text
error: --base-url (or MADS_BASE_URL) is required for openai-compat
error: --model (or MADS_MODEL) is required for openai-compat
```
