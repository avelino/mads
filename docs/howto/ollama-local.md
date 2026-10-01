# Run with Ollama

This page shows how to run mads against a local Ollama server, so no data leaves your machine and no API key is needed.

## Steps

1. Install and start Ollama. It listens on `http://localhost:11434` by default.
2. Pull a model that supports tool calling.
3. Run mads with `--provider ollama` and the model name.

```bash
ollama pull <model-name>
mads generate business.toml --provider ollama --model <model-name>
```

`mads providers` shows `ollama` as `ready` always, because it needs no key. It does not test the connection.

## When the server is not running

mads reports the connection error as a failed plan mission and exits with code `1`. This is real output with no server on the port.

```text
12:28:31 [run] 20261001-122831-70203f started (ollama, llama3.1)
12:28:31 [plan] started (attempt 1)
12:28:34 [plan] failed: Web call failed for model 'ollama::llama3.1 (adapter: Ollama)'.
Cause: Reqwest error: error sending request for url (http://localhost:11434/api/chat)
12:28:34 [out] out3/20261001-122831-70203f/report.md
12:28:34 [run] failed (exit 1): 0 in, 0 out tokens, cost n/a, 1 of 1 missions failed
```

Start the server and run `mads generate --resume <run-dir>` with the same provider flags. See [Resume and export](../guides/resume-and-export.md).

## A server on another machine

`--provider ollama` always uses `http://localhost:11434`. Ollama also serves the OpenAI chat completions API under `/v1`. Point `openai-compat` at it.

```bash
mads generate business.toml \
  --provider openai-compat \
  --base-url http://<host>:11434/v1 \
  --model <model-name>
```

See [Use an OpenAI-compatible server](openai-compatible.md).

## Model quality

mads needs a model that calls tools reliably and keeps long instructions in view. Small local models can break the ad text limits (30 characters per headline) or loop on validation errors. That shows up as `max turns` or `no progress` failures. Raise `--max-turns`, use a larger model or give the plan mission a stronger model with `--plan-model`.

Local runs report tokens but no cost.
