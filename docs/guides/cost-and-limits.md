# Cost and limits

This page shows which flags bound a run, how mads reports usage and how to keep token spend under control.

## The limits

| Flag | Default | Scope | What it bounds |
|---|---|---|---|
| `--max-turns` | `40` | per mission | Model turns before the mission fails with `max turns`. `claude-cli` and `gemini-cli` pass it to the CLI. |
| `--mission-timeout` | `15m` | per mission | Wall-clock time before the mission fails with `timeout`. |
| `--max-tokens` | `4000000` | whole run | Input plus output tokens. `0` disables the limit. |
| `--mission-retries` | `1` | per mission | Extra attempts after a failed one. |
| `--parallel` | `4` | run | Campaign missions that run at the same time. |
| `--max-ad-groups` | `50` | account | Planned ad groups across all campaigns. |
| `--max-images` | `40` | run | New pictures for image campaigns. Every picture is a paid call to the image API, apart from the tokens. Pictures already on disk are reused and do not count. |

`mads init` accepts `--max-turns`, `--mission-timeout`, `--max-tokens` and `--mission-retries` too. With an agent CLI, init searches the web by default, and each search adds tokens. `--no-web-search` gives a cheaper run that learns from the site only.

`--mission-timeout` takes `90s`, `15m`, `2h` or bare seconds. Zero is rejected.

## max-turns

A turn is one model call. In each turn the model can call several tools. The mission fails with `max turns` when it reaches the limit without calling `finish`.

mads also enforces a tool call budget of `max-turns * 4` calls per mission attempt. Past it, every call returns a `LIMIT` error. At the default that is 160 calls.

Raise `--max-turns` for large campaigns. A campaign with 20 ad groups needs at least 20 `upsert_ad_group` calls plus the rest, and the agent usually spends extra turns fixing validation errors.

## mission-timeout

When the time runs out the attempt fails with `timeout`. Tokens of that attempt are not counted in the report, because the driver is cancelled. The retry rules apply.

## max-tokens

The budget counts input plus output tokens across every mission in the run. After each model response mads adds the usage. When the total is over the limit, that mission stops with `token budget exceeded`. Other running missions stop at their next turn. A mission that has not started does not start. The run exits `1`.

```text
12:28:46 [plan] failed: token budget exceeded
12:28:46 [run] failed (exit 1): 10400 in, 1800 out tokens, cost n/a, 1 of 1 missions failed
```

The check runs after a response arrives, so the run can go over the limit by one response.

CLI providers report usage when the CLI's stream ends, often once per mission. For them the check can lag by one mission. When the check trips while a CLI mission runs, mads stops the CLI process. See [Agent CLIs](agent-clis.md).

Use `--resume` to continue. The new run starts with a fresh budget.

Set `--max-tokens 0` only when you watch the run.

## Retries

Two retry layers exist.

**Provider calls.** A rate limit (429), a timeout (408), a 5xx or a network failure is retried inside the loop. Up to 3 attempts, waiting 1 second and then 2 seconds. This does not use `--mission-retries`.

**Missions.** A failed mission (`max turns`, `timeout`, `no progress`, a fatal provider error) runs again with a fresh context while attempts are below `1 + --mission-retries`. The workspace keeps the valid partial state, and `get_brief` shows the ad groups already built, so finished work is not paid twice. The tool call budget resets for each attempt.

Set `--mission-retries 0` to fail fast.

A model that answers without calling a tool gets two nudges ("Continue using the tools. Call finish when the work is done."). A third silent answer fails the mission with `no progress`.

A token budget failure is not retried.

## parallel

`--parallel` caps how many campaign missions run at once. The plan mission always runs alone first. Lower it if your provider rate-limits you. Raise it to finish sooner. A failed campaign does not stop the others.

## Token and cost reporting

Every model response emits a `usage` event. The plain format prints it.

```text
12:28:50 [plan] tokens 5200 in, 900 out
```

The report has a Usage table with attempts, input tokens, output tokens, cost and result for each mission, plus totals. The final line of the run prints the totals.

```text
12:28:52 [run] done (exit 0): 104000 in, 18000 out tokens, cost n/a, 3 missions finished
```

API providers report tokens only, so cost shows `n/a`. `claude-cli` reports cost in USD and mads shows it. `codex-cli` and `gemini-cli` report tokens only. mads has no price table. Multiply the totals by your provider's prices.

`run.json` has the same totals for scripts.

## Keep spend down

- Use a smaller model for campaigns and a stronger one for the plan with `--plan-model`.
- Keep `--max-tokens` set in CI so a looping model stops.
- Keep the catalog tight. Every entity row goes to the agent.
- Write a precise `description`. Fewer validation retries mean fewer turns.
- Use `--resume` instead of a new run after a failure.
- Use `mads export` instead of `generate` when only the URL check or an export setting changed.
