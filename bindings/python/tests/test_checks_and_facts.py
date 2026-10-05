"""The 5.8 introspection surface: check / compile_checked, operators(),
rule facts, per-compile templating mode, truthy and strict operator names."""

import json
import pathlib

import pytest

from datalogic_py import CompileError, Engine, EvaluateError

DOCS = pathlib.Path(__file__).parents[3] / "docs" / "src" / "operators" / "operators.json"


def test_check_reports_every_problem_with_a_pointer():
    engine = Engine()
    diags = engine.check({"if": [True, {"vr": "x"}, {"map": [1]}]})
    assert [(d["code"], d["severity"], d["pointer"]) for d in diags] == [
        ("UnknownOperator", "error", "/if/1"),
        ("ArgumentCount", "error", "/if/2"),
    ]
    assert "did you mean `var`" in diags[0]["message"]
    assert engine.check('{"+": [1, 2]}') == []


def test_check_takes_a_mode():
    engine = Engine()
    template = {"a": {"var": "x"}, "b": 1}
    assert engine.check(template)[0]["code"] == "NotAnOperator"
    assert engine.check(template, "template") == []
    with pytest.raises(EvaluateError):
        engine.check(template, "loose")


def test_compile_checked_carries_the_diagnostics():
    engine = Engine()
    with pytest.raises(CompileError) as caught:
        engine.compile_checked({"if": [{"bogus": 1}, {"map": [1]}]})
    assert len(caught.value.diagnostics) == 2
    assert engine.compile_checked({"+": [1, {"var": "x"}]}).evaluate({"x": 2}) == 3


def test_operators_is_the_documented_catalogue():
    assert Engine().operators() == json.loads(DOCS.read_text())


def test_facts():
    engine = Engine()
    facts = engine.compile({"+": [{"var": "a.b"}, {"var": "c"}]}).facts()
    assert facts["reads"] == [["a", "b"], ["c"]]
    assert facts["operators"] == ["+", "val"]
    assert facts["reads_complete"] is True
    assert facts["deterministic"] is True
    assert engine.compile({"now": []}).facts()["deterministic"] is False


def test_compile_mode_per_compile():
    template = {"user": {"var": "name"}, "source": "api"}
    with pytest.raises(EvaluateError):
        Engine().compile(template)
    assert Engine().compile_template(template).evaluate({"name": "ana"}) == {
        "user": "ana",
        "source": "api",
    }
    with pytest.raises(EvaluateError):
        Engine(templating=True).compile_strict(template)


def test_truthy():
    engine = Engine()
    assert engine.truthy({}) is False
    assert engine.truthy([]) is False
    assert engine.truthy({"a": 1}) is True
    assert engine.truthy("0") is True
    assert Engine(config={"truthy_evaluator": "python"}).truthy(0) is False


def test_strict_operator_names():
    op = lambda args: json.dumps(len(json.loads(args)))  # noqa: E731
    with pytest.raises(EvaluateError) as caught:
        Engine(strict_operator_names=True, custom_operators={"length": op})
    assert caught.value.error_type == "ConfigurationError"
    engine = Engine(strict_operator_names=True, custom_operators={"count": op})
    assert engine.eval({"count": [1, 2]}, None) == 2
    Engine(custom_operators={"length": op})  # accepted without the option
