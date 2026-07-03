#!/usr/bin/env python3
"""Boundary-benchmark runner for the Python binding (`runtime: "python"`).

Runs against `datalogic_py` built from bindings/python — build a wheel
with `maturin build --release` and install it into the venv at
`boundary/.venv` (run.sh automates this), then invoke this script with
that venv's interpreter.

Modes:
  session-evaluate-str       session.evaluate_str(rule, data_str) — hot path
  session-evaluate-data      session.evaluate_data_str(rule, handle) — ABI v2
                             data-handle tier: zero parse work per call
  session-evaluate-many-100  one session.evaluate_many call over 100
                             separately-compiled copies of the rule and one
                             handle (ns_op reported per evaluation: call/100)
  rule-evaluate-str          rule.evaluate_str(data_str)
  rule-evaluate-dict         rule.evaluate(data_dict) — the object path
  dumps-str-loads-roundtrip  json.loads(rule.evaluate_str(json.dumps(data_dict)))
  engine-eval-oneshot        engine.eval_str(rule_str, data_str) — compile per call

Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 2,000
iterations (CPython is not a JIT tier), pilot pass sizing N so one timed
sample lands near ~250 ms, median of 5 samples, results consumed into a
sink. Emits JSON lines:
  {"runtime": "python", "mode": ..., "workload": ..., "ns_op": ...}

Usage: runner-python.py <workloads-dir> [--modes=a,b] [--workloads=x,y]
"""

import json
import os
import sys
import time

try:
    import datalogic_py
except ImportError as e:
    print(
        "runner-python.py: cannot import datalogic_py — build the wheel "
        "(cd bindings/python && maturin build --release) and install it "
        f"into the venv this script runs under: {e}",
        file=sys.stderr,
    )
    sys.exit(1)

RUNTIME = "python"
WARMUP = 2_000
TARGET_SAMPLE_NS = 250e6
PILOT_MIN_NS = 10e6
SAMPLES = 5
MANY_N = 100

_global_sink = 0


def measure(batch):
    """batch(n) runs n iterations and returns a sink value; median ns/op."""
    global _global_sink
    _global_sink += batch(WARMUP)

    n = 32
    while True:
        t0 = time.perf_counter_ns()
        _global_sink += batch(n)
        elapsed = time.perf_counter_ns() - t0
        if elapsed >= PILOT_MIN_NS:
            per_op = elapsed / n
            break
        n *= 2

    iters = max(1, round(TARGET_SAMPLE_NS / per_op))
    samples = []
    for _ in range(SAMPLES):
        t0 = time.perf_counter_ns()
        _global_sink += batch(iters)
        samples.append((time.perf_counter_ns() - t0) / iters)
    samples.sort()
    return samples[SAMPLES // 2]


def emit(mode, workload, ns_op):
    print(
        json.dumps(
            {"runtime": RUNTIME, "mode": mode, "workload": workload, "ns_op": round(ns_op, 3)}
        ),
        flush=True,
    )


def fail(mode, workload, expected, got):
    print(
        f"runner-python.py: verification failed for mode={mode} workload={workload}\n"
        f"  expected: {expected}\n  got:      {got}",
        file=sys.stderr,
    )
    sys.exit(1)


def verify_str(mode, workload, got, expected):
    if got != expected:
        fail(mode, workload, expected, got)


def verify_obj(mode, workload, got, expected):
    # Workload results are a bool, a str, and a list of ints — direct
    # equality against the parsed expectation is exact. Guard bool vs
    # int with a type check (True == 1 in Python).
    want = json.loads(expected)
    if got != want or type(got) is not type(want):
        fail(mode, workload, want, got)


def main():
    dir_arg = None
    mode_filter = None
    workload_filter = None
    for arg in sys.argv[1:]:
        if arg.startswith("--modes="):
            mode_filter = arg[len("--modes=") :].split(",")
        elif arg.startswith("--workloads="):
            workload_filter = arg[len("--workloads=") :].split(",")
        else:
            dir_arg = arg
    if dir_arg is None:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        sys.exit(1)

    engine = datalogic_py.Engine()

    for name in ("simple", "eligibility", "array100"):
        if workload_filter and name not in workload_filter:
            continue
        with open(os.path.join(dir_arg, f"{name}.rule.json")) as f:
            rule_str = f.read()
        with open(os.path.join(dir_arg, f"{name}.data.json")) as f:
            data_str = f.read()
        with open(os.path.join(dir_arg, f"{name}.expected.json")) as f:
            expected = f.read()

        rule = engine.compile(rule_str)
        session = engine.session()
        # Single hot dict identity across the run (matches the capture).
        data_dict = json.loads(data_str)
        rule_dict = json.loads(rule_str)
        # v2: parse-once data handle.
        data_handle = datalogic_py.DataHandle(data_str)
        # 100 identical rules, compiled separately (a rule-set of
        # identical rules — separate compiles so the batch doesn't
        # flatter one hot compiled tree). Mirrors runner-go.
        many_rules = [engine.compile(rule_str) for _ in range(MANY_N)]

        def batch_session_str(n, session=session, rule=rule, data=data_str):
            sink = 0
            for _ in range(n):
                sink += len(session.evaluate_str(rule, data))
            return sink

        def batch_session_data(n, session=session, rule=rule, data=data_handle):
            sink = 0
            for _ in range(n):
                sink += len(session.evaluate_data_str(rule, data))
            return sink

        def batch_session_many(n, session=session, rules=many_rules, data=data_handle):
            sink = 0
            for _ in range(n):
                results = session.evaluate_many(rules, data)
                sink += len(results[0]) + len(results[MANY_N - 1])
            return sink

        def verify_many():
            results = session.evaluate_many(many_rules, data_handle)
            for r in results:
                if not isinstance(r, str):
                    fail("session-evaluate-many-100", name, expected, r)
                verify_str("session-evaluate-many-100", name, r, expected)

        def batch_rule_str(n, rule=rule, data=data_str):
            sink = 0
            for _ in range(n):
                sink += len(rule.evaluate_str(data))
            return sink

        def batch_rule_dict(n, rule=rule, data=data_dict):
            sink = 0
            for _ in range(n):
                res = rule.evaluate(data)
                sink += res is not None
            return sink

        def batch_roundtrip(n, rule=rule, data=data_dict, dumps=json.dumps, loads=json.loads):
            sink = 0
            for _ in range(n):
                res = loads(rule.evaluate_str(dumps(data)))
                sink += res is not None
            return sink

        # The convenience tier the way Python callers naturally hold it:
        # dict rule and dict data, compiled per call, Python-object
        # result (str in/out one-shots measure noticeably faster — the
        # object shape is what the documented capture reflects).
        def batch_oneshot(n, engine=engine, rule=rule_dict, data=data_dict):
            sink = 0
            for _ in range(n):
                res = engine.eval(rule, data)
                sink += res is not None
            return sink

        # mode -> (verify, batch, evaluations per batch iteration).
        modes = {
            "session-evaluate-str": (
                lambda: verify_str("session-evaluate-str", name,
                                   session.evaluate_str(rule, data_str), expected),
                batch_session_str,
                1,
            ),
            "session-evaluate-data": (
                lambda: verify_str("session-evaluate-data", name,
                                   session.evaluate_data_str(rule, data_handle), expected),
                batch_session_data,
                1,
            ),
            "session-evaluate-many-100": (
                verify_many,
                batch_session_many,
                MANY_N,
            ),
            "rule-evaluate-str": (
                lambda: verify_str("rule-evaluate-str", name,
                                   rule.evaluate_str(data_str), expected),
                batch_rule_str,
                1,
            ),
            "rule-evaluate-dict": (
                lambda: verify_obj("rule-evaluate-dict", name,
                                   rule.evaluate(data_dict), expected),
                batch_rule_dict,
                1,
            ),
            "dumps-str-loads-roundtrip": (
                lambda: verify_obj("dumps-str-loads-roundtrip", name,
                                   json.loads(rule.evaluate_str(json.dumps(data_dict))),
                                   expected),
                batch_roundtrip,
                1,
            ),
            "engine-eval-oneshot": (
                lambda: verify_obj("engine-eval-oneshot", name,
                                   engine.eval(rule_dict, data_dict), expected),
                batch_oneshot,
                1,
            ),
        }

        for mode, (verify, batch, per_call_evals) in modes.items():
            if mode_filter and mode not in mode_filter:
                continue
            verify()
            emit(mode, name, measure(batch) / per_call_evals)

    print(f"{RUNTIME}: sink={_global_sink}", file=sys.stderr)


if __name__ == "__main__":
    main()
