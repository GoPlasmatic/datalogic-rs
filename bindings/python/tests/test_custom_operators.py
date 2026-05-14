"""Custom operator registration on the ``Engine`` pyclass.

Callbacks cross the FFI boundary as JSON-array string in / JSON value
string out. The contract matches the WASM, Node, C, and Go bindings.
"""

import json
import pytest

from datalogic_py import Engine, EvaluateError


def test_engine_without_custom_operators_behaves_like_before():
    engine = Engine()
    assert engine.eval_str('{"+": [1, 2]}', "{}") == "3"


def test_engine_ignores_empty_custom_operators():
    engine = Engine(custom_operators={})
    assert engine.eval_str('{"+": [1, 2]}', "{}") == "3"


def test_custom_operator_scalar_return():
    engine = Engine(custom_operators={
        "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2),
    })
    assert engine.eval_str('{"double": [21]}', "{}") == "42"


def test_custom_operator_string_return():
    engine = Engine(custom_operators={
        "upper": lambda a: json.dumps(json.loads(a)[0].upper()),
    })
    assert engine.eval_str('{"upper": ["hello"]}', "{}") == '"HELLO"'


def test_custom_operator_object_return_via_eval():
    engine = Engine(custom_operators={
        "wrap": lambda a: json.dumps({"value": json.loads(a)[0]}),
    })
    assert engine.eval({"wrap": ["hi"]}, {}) == {"value": "hi"}


def test_custom_operator_array_return_via_eval():
    engine = Engine(custom_operators={
        "repeat": lambda a: json.dumps([json.loads(a)[0]] * json.loads(a)[1]),
    })
    assert engine.eval({"repeat": ["x", 3]}, {}) == ["x", "x", "x"]


def test_custom_operator_composes_with_builtins():
    engine = Engine(custom_operators={
        "double": lambda a: json.dumps(json.loads(a)[0] * 2),
    })
    rule = {"map": [{"var": "xs"}, {"double": [{"var": ""}]}]}
    assert engine.eval(rule, {"xs": [1, 2, 3]}) == [2, 4, 6]


def test_custom_operator_multiple_args():
    def clamp(args_json):
        v, lo, hi = json.loads(args_json)
        return json.dumps(max(lo, min(hi, v)))

    engine = Engine(custom_operators={"clamp": clamp})
    assert engine.eval_str('{"clamp": [5, 0, 3]}', "{}") == "3"
    assert engine.eval_str('{"clamp": [-5, 0, 3]}', "{}") == "0"
    assert engine.eval_str('{"clamp": [2, 0, 3]}', "{}") == "2"


def test_two_custom_operators_one_engine():
    engine = Engine(custom_operators={
        "inc": lambda a: json.dumps(json.loads(a)[0] + 1),
        "neg": lambda a: json.dumps(-json.loads(a)[0]),
    })
    assert engine.eval_str('{"inc": [{"neg": [3]}]}', "{}") == "-2"


def test_builtin_wins_over_custom_with_same_name():
    engine = Engine(custom_operators={
        "+": lambda _a: json.dumps("hijacked"),
    })
    # Built-in `+` dispatches first; the custom registration is unreachable.
    assert engine.eval_str('{"+": [1, 2]}', "{}") == "3"


def test_custom_operator_that_raises_propagates_as_error():
    engine = Engine(custom_operators={
        "boom": lambda _a: (_ for _ in ()).throw(RuntimeError("kaboom")),
    })
    with pytest.raises(EvaluateError):
        engine.eval_str('{"boom": []}', "{}")


def test_custom_operator_returning_non_string_is_an_error():
    engine = Engine(custom_operators={
        "bad": lambda _a: 42,  # not a str
    })
    with pytest.raises(EvaluateError):
        engine.eval_str('{"bad": []}', "{}")


def test_custom_operator_returning_invalid_json_is_an_error():
    engine = Engine(custom_operators={
        "bad": lambda _a: "this is not json",
    })
    with pytest.raises(EvaluateError):
        engine.eval_str('{"bad": []}', "{}")


def test_compiled_rule_uses_engine_custom_operators():
    engine = Engine(custom_operators={
        "add5": lambda a: json.dumps(json.loads(a)[0] + 5),
    })
    rule = engine.compile('{"add5": [{"var": "x"}]}')
    assert rule.evaluate({"x": 10}) == 15
    assert rule.evaluate({"x": 100}) == 105
