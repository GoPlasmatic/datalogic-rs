"""Compile-once / evaluate-many via Engine + Rule."""

import json

import pytest

from datalogic_py import Engine, Rule


def test_engine_constructor():
    engine = Engine()
    assert repr(engine) == "Engine()"


def test_engine_templating_kwarg():
    # Templating must be a keyword argument (signature uses `*`).
    engine = Engine(templating=True)
    rule = engine.compile({"name": {"var": "user.name"}, "ok": {">": [{"var": "score"}, 50]}})
    result = rule.evaluate({"user": {"name": "Ada"}, "score": 99})
    assert result == {"name": "Ada", "ok": True}


def test_engine_templating_positional_rejected():
    with pytest.raises(TypeError):
        Engine(True)  # type: ignore[misc]


def test_compile_returns_rule():
    engine = Engine()
    rule = engine.compile({"+": [1, 2]})
    assert isinstance(rule, Rule)


def test_rule_evaluate_dict():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    for x in range(5):
        assert rule.evaluate({"x": x}) == x + 1


def test_rule_evaluate_str_returns_json():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    out = rule.evaluate_str('{"x": 41}')
    assert out == "42"


def test_rule_evaluate_string_payload():
    # A `str` argument to evaluate() is treated as JSON text — the binding
    # short-circuits the dict→Value path and parses with serde_json.
    engine = Engine()
    rule = engine.compile({"var": "msg"})
    assert rule.evaluate('{"msg": "hi"}') == "hi"


def test_compile_string_rule():
    engine = Engine()
    rule = engine.compile('{"+": [1, 2]}')
    assert rule.evaluate({}) == 3


def test_engine_eval_one_shot():
    engine = Engine()
    assert engine.eval({"+": [1, 2]}, {}) == 3
    assert engine.eval_str({"+": [1, 2]}, {}) == "3"


def test_rule_shared_across_evaluations():
    engine = Engine()
    rule = engine.compile({"if": [{">": [{"var": "n"}, 0]}, "positive", "non-positive"]})
    payloads = [{"n": 5}, {"n": -5}, {"n": 0}]
    expected = ["positive", "non-positive", "non-positive"]
    assert [rule.evaluate(p) for p in payloads] == expected


def test_rule_handles_nested_objects():
    engine = Engine()
    rule = engine.compile({"var": "a.b.0.c"})
    assert rule.evaluate({"a": {"b": [{"c": "deep"}, {"c": "shallow"}]}}) == "deep"


def test_rule_returns_array():
    engine = Engine()
    rule = engine.compile({"var": "items"})
    assert rule.evaluate({"items": [1, 2, 3]}) == [1, 2, 3]


def test_rule_str_input_mirrors_dict_input():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, {"var": "y"}]})
    data = {"x": 10, "y": 32}
    assert rule.evaluate(data) == rule.evaluate(json.dumps(data))
