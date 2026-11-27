# Keep token cost down

Cap what a run can spend on tokens and pictures, so a looping model or a stray API key cannot run up your bill.

mads has no price table. It counts tokens and pictures, and your provider bills you. The caps below are the only brake you have.

## Before you start

- A provider that works. See [Choose a provider](choose-a-provider.md).
- A `business.toml` that loads, for the `generate` examples. `mads init` makes one. See [Write the business file](write-the-business-file.md).
- The shell where you will run `mads`. Step 5 depends on which keys it holds.

## Steps

### 1. Cap the tokens of the run

`--max-tokens` is one budget for the whole run, input plus output, across every mission. The default is `4000000`. Lower it to the most you accept to spend.

```bash
mads generate business.toml --provider anthropic --model <model> --max-tokens 1500000
```

When the total goes over, the mission stops with `token budget exceeded` and the run exits `1`. The check runs after each model response, so a run can pass the limit by one response. With an agent CLI it can lag by one mission.

`--max-tokens 0` removes the cap. Use it only while you watch the run.

A budget failure is not retried. Finish the run with `--resume` and it starts with a fresh budget. See [Resume a run](resume-a-run.md).

### 2. Put the stronger model on the plan only

`--plan-model` sets the model for the plan mission. `--model` sets the model for everything else. Pick a cheaper `--model` and keep a stronger `--plan-model` for the one mission that decides the structure of the account.

```bash
mads generate business.toml --provider anthropic --model <cheaper-model> --plan-model <stronger-model>
```

Without `--plan-model`, the plan uses `--model`. `mads init` also reads `--plan-model` in place of `--model`.

### 3. Bound the turns and the retries

`--max-turns` is the number of model calls one mission may make. The default is `40`. mads also allows four times that many tool calls per attempt, and past it every call returns a `LIMIT` error. A mission that hits the cap fails with `max turns`.

`--mission-retries` is the number of extra attempts after a failed one. The default is `1`. A retry starts a new context and pays for it. Set `--mission-retries 0` to fail fast and read the error before you spend more.

```bash
mads generate business.toml --provider anthropic --model <model> --max-turns 30 --mission-retries 0
```

Do not set `--max-turns` so low that a big campaign cannot finish. Every ad group is at least one `upsert_ad_group` call, and the agent spends extra turns on validation errors. A mission that fails on turns costs more than one that had room.

The retry layers and what trips each limit are in [Limits and retries](../explanation/limits-and-retries.md).

### 4. Keep init off the web

Agent CLIs search the web while `mads init` runs, and every search adds tokens. API providers never search. Pass `--no-web-search` to learn from the site only.

```bash
mads init --from-url https://example.com --daily-budget 50 --currency BRL --provider claude-cli --no-web-search
```

### 5. Decide whether pictures are on

This step explains the one cost that surprises people.

`--image-provider` defaults to `auto`. With `auto`, mads uses Gemini when `GEMINI_API_KEY` is set in the shell, then OpenAI when `OPENAI_API_KEY` is set. A key you exported for another tool is enough. The run then has an image model, and when the run also has a logo, the plan agent plans an image campaign by default. Every picture is a paid call to the image API, on top of the tokens.

Choose one of these.

- Keep the run on Search. Set `--image-provider none`, or `MADS_IMAGE_PROVIDER=none` in the environment. If your `campaigns.formats` asks for pictures, `none` exits `2` before the run starts. Remove that format first. See the [`campaigns` reference](../reference/business-toml.md#campaigns).
- Keep the pictures and cap them. `--max-images` is the number of new pictures one run may draw. The default is `40`. Pictures already on disk do not count.

```bash
mads generate business.toml --provider anthropic --model <model> --image-provider none
mads generate business.toml --provider anthropic --model <model> --max-images 10
```

Pictures over the cap are dropped when the asset group still has enough without them, and the report notes it. A picture the group cannot do without stays and stops the export with `E20`. To draw it, run `mads generate --resume <run-dir>` with a higher `--max-images`.

To set pictures up on purpose, see [Add image and app campaigns](add-image-campaigns.md).

### 6. Resume and export instead of starting over

Use `mads generate --resume <run-dir>` after a failure. Only the missions that did not finish run again, and finished work is not paid twice. Pictures that pass the checks are reused too.

When only the URL check or an export setting changed, use `mads export <run-dir>`. It calls no model and draws no picture. See [Resume a run](resume-a-run.md).

## Check it worked

Run `mads providers`. The image table shows `ready` for `gemini` or `openai` when its key is set in that shell, and `auto` would pick it. With `none` set, no picture is drawn whatever the table shows.

```bash
mads providers
```

Read the last line of the run. It prints the token totals for the whole run.

Example run (replay provider, no model was called). The run used `--skip-url-check`.

```text
10:21:18 [run] done (exit 0): 135000 in, 21800 out tokens, cost n/a, 4 missions finished
```

`cost n/a` is normal for API providers, because they report tokens only. `claude-cli` reports cost in USD. `codex-cli` and `gemini-cli` report tokens only. Multiply the totals by your provider's prices.

The same numbers are in the Usage table of `report.md` and in `run.json`.

Example run (replay provider, no model was called).

```text
| Mission | Attempts | Input tokens | Output tokens | Cost | Result |
|---|---|---|---|---|---|
| campaign:vinellu-app-de-vinhos | 1 | 31000 | 5400 | n/a | finished |
| campaign:vinellu-marca | 1 | 18000 | 2100 | n/a | finished |
| campaign:vinellu-rotulos | 1 | 52000 | 9100 | n/a | finished |
| plan | 1 | 34000 | 5200 | n/a | finished |

Total: 135000 input tokens, 21800 output tokens, cost n/a.
```

## Next

- [Limits and retries](../explanation/limits-and-retries.md)
- [Run limits](../reference/limits.md)
- [Run in GitHub Actions](run-in-github-actions.md)
