#!/usr/bin/env python3
"""Render collected boundary-benchmark JSON lines into the two tables
BINDINGS-OVERHEAD.md uses.

Input: one or more .jsonl files produced by run.sh (defaults to the
newest output/boundary-*.jsonl). Lines look like:
  {"runtime": "c-abi", "mode": "session-evaluate", "workload": "simple", "ns_op": 123.0}
Later lines win on duplicate (runtime, mode, workload) keys, so re-runs
appended to the same file supersede earlier numbers.

Output (stdout, markdown):
  1. The hot-path table (one row per runtime's best documented pattern,
     with fixed overhead vs the string-contract floor on `simple`).
  2. The full appendix table (every runtime/mode row, fenced block).
"""

import glob
import json
import os
import sys

WORKLOADS = ["simple", "eligibility", "array100"]

# Appendix row order. Runtimes and modes not listed here are appended in
# first-seen order, so new tiers show up without touching this file.
RUNTIME_ORDER = [
    "rust-core", "c-abi", "dotnet", "python", "node", "go", "jvm", "php", "wasm",
]
MODE_ORDER = {
    "rust-core": [
        "eval-preparsed", "parseddata-eval", "parse-eval", "parse-eval-serialize",
        "parse-eval-serialize-fresharena", "serde-value-in-out",
    ],
    "c-abi": [
        "session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
        "rule-evaluate", "engine-apply-oneshot",
    ],
    "dotnet": [
        "session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
        "rule-evaluate", "engine-apply-oneshot",
    ],
    "python": [
        "session-evaluate-str", "rule-evaluate-str", "rule-evaluate-dict",
        "dumps-str-loads-roundtrip", "engine-eval-oneshot",
    ],
    "node": [
        "session-evaluateStr-str", "rule-evaluateStr-str", "rule-evaluate-obj",
        "stringify-str-parse-roundtrip", "engine-eval-oneshot",
    ],
    "go": [
        "session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
        "rule-evaluate", "engine-apply-oneshot",
    ],
    "jvm": [
        "session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
        "rule-evaluate", "engine-apply-oneshot",
    ],
    "php": [
        "session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
        "rule-evaluate", "encode-eval-decode-roundtrip", "engine-apply-oneshot",
    ],
    "wasm": [
        "session-evaluate-str", "compiledrule-evaluate-str", "oneshot-evaluate",
    ],
}

# The best documented pattern per binding — the "hot path" table rows.
# rust-core's parse-eval-serialize is the string-contract floor every
# binding is judged against.
HOT_PATH = {
    "rust-core": ("parse-eval-serialize", "string-contract floor"),
    "c-abi": ("session-evaluate", "C ABI, called from C"),
    "dotnet": ("session-evaluate", ".NET (`Session.Evaluate`)"),
    "python": ("session-evaluate-str", "Python (`evaluate_str`)"),
    "node": ("session-evaluateStr-str", "Node (`evaluateStr`)"),
    "go": ("session-evaluate", "Go (cgo)"),
    "wasm": ("session-evaluate-str", "WASM (session)"),
    "php": ("session-evaluate", "PHP (FFI)"),
    "jvm": ("session-evaluate", "JVM"),
}


def load(paths):
    cells = {}
    order_seen = []  # (runtime, mode) first-seen order for unknown rows
    for path in paths:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    continue  # runner stderr lines never hit stdout; be lenient anyway
                key = (rec["runtime"], rec["mode"])
                if key not in order_seen:
                    order_seen.append(key)
                cells[(rec["runtime"], rec["mode"], rec["workload"])] = float(rec["ns_op"])
    return cells, order_seen


def fmt(ns):
    if ns is None:
        return "—"
    return f"{ns:,.1f}"


def row_order(cells, order_seen):
    """All (runtime, mode) pairs present, in canonical order."""
    present = {(r, m) for (r, m, _w) in cells}
    runtimes = [r for r in RUNTIME_ORDER if any(pr == r for pr, _ in present)]
    runtimes += [r for r, _ in order_seen if r not in runtimes]
    rows = []
    for r in runtimes:
        listed = [m for m in MODE_ORDER.get(r, []) if (r, m) in present]
        extra = [m for (pr, m) in order_seen if pr == r and (r, m) in present and m not in listed]
        for m in listed + extra:
            if (r, m) not in [(a, b) for a, b in rows]:
                rows.append((r, m))
    return rows


def hot_path_table(cells):
    floor = cells.get(("rust-core", "parse-eval-serialize", "simple"))
    rows = []
    for runtime, (mode, label) in HOT_PATH.items():
        vals = [cells.get((runtime, mode, w)) for w in WORKLOADS]
        if all(v is None for v in vals):
            continue
        rows.append((label, runtime, mode, vals))
    # Floor row pinned first (even if a borrowed-result binding beats
    # it), then ascending by `simple` — same shape as the doc.
    rows.sort(
        key=lambda r: (
            r[1] != "rust-core",
            r[3][0] is None,
            r[3][0] if r[3][0] is not None else 0,
        )
    )

    out = ["## Hot path per binding", ""]
    out.append("| Binding | simple | eligibility | array100 | Fixed overhead vs floor (simple) |")
    out.append("|---------|-------:|------------:|---------:|---------------------------------:|")
    for label, runtime, _mode, vals in rows:
        if runtime == "rust-core":
            overhead = "0"
        elif floor is not None and vals[0] is not None:
            overhead = f"{vals[0] - floor:+,.0f}"
        else:
            overhead = "—"
        out.append(
            f"| {label} | {fmt(vals[0])} | {fmt(vals[1])} | {fmt(vals[2])} | {overhead} |"
        )
    return "\n".join(out)


def appendix_table(cells, order_seen):
    rows = row_order(cells, order_seen)
    rt_w = max([len("runtime")] + [len(r) for r, _ in rows])
    md_w = max([len("mode")] + [len(m) for _, m in rows])

    out = ["## Appendix: full result tables", "", "```"]
    header = f"{'runtime':<{rt_w}}  {'mode':<{md_w}}  {'simple':>10}  {'eligibility':>11}  {'array100':>10}"
    out.append(header)
    for r, m in rows:
        vals = [fmt(cells.get((r, m, w))) for w in WORKLOADS]
        out.append(f"{r:<{rt_w}}  {m:<{md_w}}  {vals[0]:>10}  {vals[1]:>11}  {vals[2]:>10}")
    out.append("```")
    return "\n".join(out)


def main():
    paths = sys.argv[1:]
    if not paths:
        here = os.path.dirname(os.path.abspath(__file__))
        candidates = sorted(glob.glob(os.path.join(here, "output", "boundary-*.jsonl")))
        if not candidates:
            print("render.py: no output/boundary-*.jsonl found and no file given", file=sys.stderr)
            return 1
        paths = [candidates[-1]]
        print(f"render.py: rendering {paths[0]}", file=sys.stderr)

    cells, order_seen = load(paths)
    if not cells:
        print("render.py: no result lines found", file=sys.stderr)
        return 1

    print(hot_path_table(cells))
    print()
    print(appendix_table(cells, order_seen))
    return 0


if __name__ == "__main__":
    sys.exit(main())
