# Run limits

Look up every flag that bounds a run, its default, and what happens when a run hits it.

## Flags

| Flag | Default | Scope | Commands |
|---|---|---|---|
| `--max-turns` | `40` | One mission attempt | `init`, `generate`, `optimize` |
| `--mission-timeout` | `15m` | One mission attempt | `init`, `generate`, `optimize` |
| `--max-tokens` | `4000000` | Whole run | `init`, `generate`, `optimize` |
| `--mission-retries` | `1` | One mission | `init`, `generate`, `optimize` |
| `--parallel` | `4` | Whole run | `generate`, `optimize` |
| `--max-ad-groups` | `50` | Export check | `generate`, `optimize`, `export` |
| `--max-images` | `40` | Whole run | `generate`, `optimize` |

`mads export` has no model, so it takes only `--max-ad-groups` (and `--layout`).

## What each flag bounds

| Flag | Bounds | When it trips |
|---|---|---|
| `--max-turns` | Model turns, and tool calls at four times that. | The mission fails with `max turns`. Past the tool call budget every call returns a `LIMIT` error. |
| `--mission-timeout` | Wall-clock time of one attempt. | The attempt fails with `timeout`. |
| `--max-tokens` | Input plus output tokens across all missions. `0` turns the limit off. | The mission fails with `token budget exceeded` and the run exits `1`. |
| `--mission-retries` | Extra attempts after a failed one. | A mission runs at most `1 + --mission-retries` times. |
| `--parallel` | Campaign missions running at the same time. | The plan mission always runs alone first. |
| `--max-ad-groups` | Ad groups across all campaigns. | Rule `E13` fails when the planned ad group count is over it. See [Validation rules](validation-rules.md). |
| `--max-images` | New pictures for image campaigns. Pictures already on disk do not count. | Pictures over the limit are dropped when the asset group has enough without them, and noted in the report otherwise. A picture the asset group cannot do without stays and stops the export with `E20`. |

## Values

- `--max-turns` passes through to `claude-cli` and `gemini-cli`.
- The tool call budget is `max-turns * 4` per attempt. At the default that is 160 calls. It resets on each attempt.
- `--mission-timeout` takes `90s`, `15m`, `2h` or bare seconds. Zero is rejected.
- `--max-tokens` is checked after each model response, so a run can pass the limit by one response. Agent CLIs report usage when their stream ends, so the check can lag by one mission.
- A token budget failure is not retried.
- Tokens of a timed-out attempt are not counted, because the driver is cancelled.

## Retries

| Layer | Trigger | Attempts | Wait |
|---|---|---|---|
| Provider call | Rate limit (429), timeout (408), 5xx or network failure | 3 | 1 second, then 2 seconds |
| Mission | `max turns`, `timeout`, `no progress`, fatal provider error | `1 + --mission-retries` | None |

Provider call retries do not use `--mission-retries`.

A model that answers without a tool call gets two nudges. A third silent answer fails the mission with `no progress`.

## Next

- [Limits and retries](../explanation/limits-and-retries.md)
- [Keep cost down](../how-to/keep-cost-down.md)
- [CLI](cli.md)
