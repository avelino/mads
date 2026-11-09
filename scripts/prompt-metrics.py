#!/usr/bin/env python3
"""Cost and retries per run, read from out/<run>/events.ndjson.

Use it before and after a change to crates/mads-core/prompts/*.md, over runs made with the same
input and the same provider, and compare the medians. One run says little.

    scripts/prompt-metrics.py out/20261109-104551-c53418 out/20261109-103951-f354e2
    scripts/prompt-metrics.py out/2026110*            # a glob works too
"""

import json
import statistics
import sys
from pathlib import Path


def read(run: Path):
    rows = {"plan": [0, 0, 0.0], "campaigns": [0, 0, 0.0]}
    calls = {"plan": 0, "campaigns": 0}
    failed = {"plan": 0, "campaigns": 0}
    for line in (run / "events.ndjson").read_text().splitlines():
        e = json.loads(line)["event"]
        kind = e.get("type")
        mission = e.get("mission", "")
        side = "plan" if mission == "plan" else "campaigns"
        if kind == "usage":
            rows[side][0] += e["input_tokens"]
            rows[side][1] += e["output_tokens"]
            rows[side][2] += e["cost_usd"]
        elif kind == "tool_called":
            calls[side] += 1
        elif kind == "tool_finished" and e.get("ok") is False:
            failed[side] += 1
    return rows, calls, failed


def main(paths):
    runs = [Path(p) for p in paths if (Path(p) / "events.ndjson").exists()]
    if not runs:
        sys.exit("no run with events.ndjson in the arguments")
    table = []
    print(
        f"{'run':<28}{'plan $':>8}{'camp $':>8}{'plan calls':>11}{'plan fail':>10}{'camp fail':>10}"
    )
    for run in runs:
        rows, calls, failed = read(run)
        if not calls["plan"]:
            continue
        table.append(
            (
                rows["plan"][2],
                rows["campaigns"][2],
                calls["plan"],
                failed["plan"],
                failed["campaigns"],
            )
        )
        print(
            f"{run.name:<28}{rows['plan'][2]:>8.2f}{rows['campaigns'][2]:>8.2f}"
            f"{calls['plan']:>11}{failed['plan']:>10}{failed['campaigns']:>10}"
        )
    if len(table) > 1:
        med = [statistics.median(col) for col in zip(*table)]
        print(
            f"{'median of ' + str(len(table)):<28}{med[0]:>8.2f}{med[1]:>8.2f}{med[2]:>11.1f}{med[3]:>10.1f}{med[4]:>10.1f}"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
