"""`Engine.evaluate_with_trace` returns the WASM-shaped trace envelope."""

import json

from datalogic_py import Engine


def test_trace_envelope_success():
    engine = Engine()
    raw = engine.evaluate_with_trace('{">": [{"var": "score"}, 50]}', '{"score": 75}')
    envelope = json.loads(raw)

    # Same envelope as the WASM binding's evaluateWithTrace: result +
    # expression_tree + steps; error keys are omitted on success.
    assert envelope["result"] is True
    assert "error" not in envelope
    assert "structured_error" not in envelope

    tree = envelope["expression_tree"]
    assert set(tree) == {"id", "expression", "children"}
    assert ">" in tree["expression"]
    # {"var": "score"} survives as an operator child.
    assert tree["children"], "expected at least one operator child"

    steps = envelope["steps"]
    assert steps, "expected at least one execution step"
    for step in steps:
        assert {"step_id", "node_id", "context", "result", "error"} <= set(step)


def test_trace_envelope_runtime_error():
    engine = Engine()
    envelope = json.loads(engine.evaluate_with_trace('{"+": ["x", 1]}', "null"))

    # Runtime failures do not raise; the envelope carries them instead.
    assert envelope["result"] is None
    assert envelope["error"]
    structured = envelope["structured_error"]
    assert structured["type"]
    assert structured["operator"] == "+"


def test_trace_envelope_compile_error():
    # Malformed logic JSON fails before evaluation: no steps, but the
    # envelope shape holds and carries the error.
    engine = Engine()
    envelope = json.loads(engine.evaluate_with_trace('{"not json', "{}"))
    assert envelope["result"] is None
    assert envelope["error"]
    assert envelope["structured_error"]["type"] == "ParseError"
    assert envelope["steps"] == []


def test_trace_respects_engine_templating():
    engine = Engine(templating=True)
    envelope = json.loads(
        engine.evaluate_with_trace('{"a": {"+": [1, 2]}, "b": 1}', "{}")
    )
    assert envelope["result"] == {"a": 3, "b": 1}


def test_trace_step_result_matches():
    engine = Engine()
    envelope = json.loads(engine.evaluate_with_trace('{"+": [1, 2, 3]}', "null"))
    assert envelope["result"] == 6
    # The one-shot trace path skips constant folding, so the "+" node
    # records a step whose result is the final value.
    assert any(step["result"] == 6 for step in envelope["steps"])
