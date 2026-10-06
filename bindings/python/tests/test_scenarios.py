"""The cross-binding scenarios in bindings/scenarios/api.json, through the
Python API. Every binding runs the same file (see bindings/BINDINGS.md)."""

import json
import pathlib

import pytest

from datalogic_py import DataLogicError, Engine

SCENARIOS = [
    case
    for case in json.loads(
        (pathlib.Path(__file__).parents[2] / "scenarios" / "api.json").read_text()
    )
    if isinstance(case, dict)
]


def engine_for(case):
    opts = case.get("engine", {})
    return Engine(
        templating=opts.get("templating", False),
        template_key_escape=opts.get("template_key_escape"),
        config=opts.get("config"),
        families=opts.get("families"),
    )


def run(case):
    """The call's value, or ("error", type) when it raised."""
    call = case["call"]
    try:
        engine = engine_for(case)
        if call == "check":
            return [[d["code"], d["pointer"]] for d in engine.check(case["rule"], case.get("mode"))]
        if call == "truthy":
            return engine.truthy(case["value"])
        if call == "facts":
            return engine.compile(case["rule"]).facts()
        if call == "trace":
            run = json.loads(
                engine.evaluate_with_trace(json.dumps(case["rule"]), json.dumps(case["data"]))
            )
            return {
                "result": run["result"],
                "pointers": sorted(set(run.get("pointers", {}).values())),
            }
        compile_ = {
            "evaluate": engine.compile,
            "compile_template": engine.compile_template,
            "compile_strict": engine.compile_strict,
            "compile_checked": engine.compile_checked,
            "metered": engine.compile,
        }[call]
        rule = compile_(case["rule"])
        if call == "metered":
            result, _ops = rule.evaluate_metered(case["data"], case["budget"])
            return json.loads(result)
        return rule.evaluate(case["data"])
    except DataLogicError as e:
        return ("error", e.error_type)


@pytest.mark.parametrize("case", SCENARIOS, ids=[c["description"] for c in SCENARIOS])
def test_scenario(case):
    got = run(case)
    if "error" in case:
        assert got == ("error", case["error"])
    elif "diagnostics" in case:
        assert got == case["diagnostics"]
    elif "trace" in case:
        assert got == case["trace"]
    elif "facts" in case:
        assert {k: got[k] for k in case["facts"]} == case["facts"]
    else:
        assert got == case["result"]
