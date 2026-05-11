"""Exception hierarchy and structured error attributes."""

import pytest

from datalogic_py import (
    DataLogicError,
    Engine,
    EvaluateError,
    ParseError,
    apply,
)


def test_exception_hierarchy():
    assert issubclass(ParseError, DataLogicError)
    assert issubclass(EvaluateError, DataLogicError)
    assert issubclass(DataLogicError, Exception)


def test_parse_error_on_malformed_rule_string():
    engine = Engine()
    with pytest.raises(ParseError):
        engine.compile("{ this is not json")


def test_parse_error_on_malformed_data_string():
    engine = Engine()
    rule = engine.compile({"var": "x"})
    with pytest.raises(ParseError):
        rule.evaluate("{ also not json")


def test_evaluate_error_carries_attributes():
    engine = Engine()
    rule = engine.compile({"+": ["x", 1]})
    with pytest.raises(EvaluateError) as exc_info:
        rule.evaluate({})
    err = exc_info.value
    assert err.error_type, "error_type should be populated"
    # The `+` operator surfaces as the failing op.
    assert err.operator == "+"
    # node_ids is a list of compiled-node ids; non-empty for runtime errors.
    assert isinstance(err.node_ids, list)
    assert all(isinstance(n, int) for n in err.node_ids)
    # path is resolved when the binding has the compiled Logic at hand.
    assert err.path is not None
    assert isinstance(err.path, list)
    assert err.path[0]["operator"] == "+"


def test_apply_raises_evaluate_error():
    with pytest.raises(EvaluateError):
        apply({"+": ["nope", 1]}, {})


def test_caught_as_datalogic_error():
    # A consumer that wants to swallow everything from this binding can
    # catch the base class.
    try:
        apply({"+": ["x", 1]}, {})
    except DataLogicError as e:
        assert hasattr(e, "error_type")
        return
    pytest.fail("expected DataLogicError")


def test_unsupported_python_type_is_parse_error():
    # `bytes` is not a JSON-compatible scalar; pythonize rejects it and
    # the binding wraps the failure in ParseError.
    engine = Engine()
    rule = engine.compile({"var": "x"})
    with pytest.raises(ParseError):
        rule.evaluate({"x": b"raw bytes"})


def test_thrown_error_surfaces_as_evaluate_error():
    # The `throw` operator (when available) raises a Thrown error inside
    # the engine, which the binding converts to EvaluateError.
    engine = Engine()
    rule = engine.compile({"throw": "boom"})
    with pytest.raises(EvaluateError) as exc_info:
        rule.evaluate({})
    # Thrown errors carry a "Thrown" tag.
    assert exc_info.value.error_type == "Thrown"
