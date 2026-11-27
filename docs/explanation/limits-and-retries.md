# Limits and retries

Predict what happens when a model loops, stalls or hits a rate limit, from how mads stops a run, which failures it retries and how it reports usage.

For the flags and their defaults, see the [CLI reference](../reference/cli.md).

## Why a run needs bounds

A mission is a model talking to tools until it calls `finish`. Nothing in the model forces it to stop. A weak model can loop on the same validation error, and a slow provider can hang. mads bounds a mission three ways: turns, wall-clock time, and a token budget shared by the whole run.

## Turns and the tool call budget

A turn is one model call. In one turn the model can call several tools. When the mission reaches `--max-turns` without calling `finish`, it fails with `max turns`. With claude-cli and gemini-cli the CLI counts turns itself, because mads passes it `--max-turns`. When the CLI stops short of `finish`, the failure carries the CLI's own message, not `max turns`.

Turns are not the only cap. mads also counts tool calls and allows four times `--max-turns` per mission attempt. Past that, every call returns a `LIMIT` error and changes nothing. This stops a model that packs hundreds of calls into a few turns.

A big campaign needs room. Every ad group is at least one `upsert_ad_group` call, and the agent usually spends extra turns on validation errors.

## Time

`--mission-timeout` is wall-clock time per attempt. When it runs out, the attempt fails with `timeout`. The driver is cancelled, so the tokens of that attempt are not counted in the report.

## The token budget

`--max-tokens` is one budget for the whole run, input plus output, across every mission. mads adds the usage after each model response. When the total goes over, that mission stops with `token budget exceeded`. Missions that are running stop at their next turn. Missions that have not started do not start. The run exits `1`.

```text
12:28:46 [plan] failed: token budget exceeded
12:28:46 [run] failed (exit 1): 10400 in, 1800 out tokens, cost n/a, 1 of 1 missions failed
```

The check runs after a response arrives. A run can go over the limit by one response.

Agent CLIs report usage when their stream ends, often once per mission, so for them the check can lag by one mission. When it trips while a CLI mission runs, mads kills the CLI process. See [Agent CLIs](agent-clis.md).

A budget failure is never retried. A retry would spend the same budget again. Resume the run instead, and the new run starts with a fresh budget. See [Resume a run](../how-to/resume-a-run.md).

## Two retry layers

Retries happen at two levels, and they do not share a setting.

**Provider calls.** A rate limit (429), a timeout (408), a 5xx or a network failure is a transient error. The loop retries inside the same turn, up to 3 attempts in total, waiting 1 second and then 2 seconds. A fatal error, such as a bad key, is not retried. This layer has no flag.

**Missions.** When an attempt fails with `max turns`, `timeout`, `no progress` or a fatal provider error, mads runs the mission again with a fresh context. The number of extra attempts is `--mission-retries`. The tool call budget resets for each attempt.

The retry is cheap because the workspace keeps the valid partial state. The next attempt calls `get_brief`, sees the ad groups already built, and keeps them unless it replaces them. Finished work is not paid twice. Set `--mission-retries 0` to fail fast.

## Nudges and `no progress`

Sometimes a model answers in text and calls no tool. mads replies with "Continue using the tools. Call finish when the work is done." It does this twice. A third silent answer fails the mission with `no progress`, and the mission retry layer takes over.

## Parallel missions

The plan mission runs alone first. Then campaign missions run in parallel, up to `--parallel` at a time. A failed campaign does not stop the others. The token budget is shared, so more parallel missions spend it faster.

## What the run reports

Every model response emits a `usage` event, and the plain log prints it. The last line of the run prints the totals, and `run.json` has the same totals for scripts. The [report](../reference/report-md.md) has a Usage table with attempts, tokens, cost and result for each mission.

Cost depends on the provider. API providers report tokens only, so cost shows `n/a`. `claude-cli` reports cost in USD and mads shows it. `codex-cli` and `gemini-cli` report tokens only. mads has no price table, so multiply the token totals by your provider's prices.

## Next

- [Keep cost down](../how-to/keep-cost-down.md)
- [Resume a run](../how-to/resume-a-run.md)
- [Exit codes and errors](../reference/exit-codes-and-errors.md)
