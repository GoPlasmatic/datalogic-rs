"""Typed session evaluations — strict bool/int, lenient float, truthy.

Semantics mirror `bindings/c/src/session.rs`: strict bool and strict
int raise `TypeMismatch` on any other result type, float accepts any
JSON number, truthy never mismatches. The mismatch messages use the C
ABI's `type_of` wording (null/boolean/number/string/array/object).
"""

import pytest

from datalogic_py import DataHandle, Engine, EvaluateError


@pytest.fixture()
def engine():
    return Engine()


@pytest.fixture()
def sess(engine):
    return engine.session()


HANDLE = DataHandle(
    '{"flag": true, "off": false, "n": 25, "f": 2.5, "whole": 3.0,'
    ' "s": "text", "arr": [1], "obj": {"k": 1}, "nil": null}'
)


def rule_for(engine, var):
    return engine.compile({"var": var})


# ---------------- evaluate_bool ----------------


def test_bool_true_false(engine, sess):
    assert sess.evaluate_bool(rule_for(engine, "flag"), HANDLE) is True
    assert sess.evaluate_bool(rule_for(engine, "off"), HANDLE) is False


@pytest.mark.parametrize(
    ("var", "got"),
    [("n", "number"), ("s", "string"), ("arr", "array"), ("obj", "object"), ("nil", "null")],
)
def test_bool_mismatch(engine, sess, var, got):
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_bool(rule_for(engine, var), HANDLE)
    err = exc_info.value
    assert err.error_type == "TypeMismatch"
    assert str(err) == f"result is not a boolean (got {got})"


# ---------------- evaluate_int ----------------


def test_int_result(engine, sess):
    assert sess.evaluate_int(rule_for(engine, "n"), HANDLE) == 25


def test_int_accepts_whole_float(engine, sess):
    # Same as the C ABI's as_i64: a number that *is* an exact integer
    # passes, whatever its JSON spelling.
    assert sess.evaluate_int(rule_for(engine, "whole"), HANDLE) == 3


@pytest.mark.parametrize(
    ("var", "got"),
    [("f", "number"), ("flag", "boolean"), ("s", "string"), ("arr", "array"), ("nil", "null")],
)
def test_int_mismatch(engine, sess, var, got):
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_int(rule_for(engine, var), HANDLE)
    err = exc_info.value
    assert err.error_type == "TypeMismatch"
    assert str(err) == f"result is not an integer number (got {got})"


# ---------------- evaluate_float ----------------


def test_float_accepts_any_number(engine, sess):
    assert sess.evaluate_float(rule_for(engine, "f"), HANDLE) == 2.5
    result = sess.evaluate_float(rule_for(engine, "n"), HANDLE)
    assert result == 25.0
    assert isinstance(result, float)


@pytest.mark.parametrize(
    ("var", "got"),
    [("flag", "boolean"), ("s", "string"), ("arr", "array"), ("obj", "object"), ("nil", "null")],
)
def test_float_mismatch(engine, sess, var, got):
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_float(rule_for(engine, var), HANDLE)
    err = exc_info.value
    assert err.error_type == "TypeMismatch"
    assert str(err) == f"result is not a number (got {got})"


# ---------------- evaluate_truthy ----------------


def test_truthy_never_mismatches(engine, sess):
    truthy = {"flag": True, "n": True, "s": True, "arr": True}
    falsy = {"off": False, "nil": False}
    for var, expected in {**truthy, **falsy}.items():
        assert sess.evaluate_truthy(rule_for(engine, var), HANDLE) is expected


def test_truthy_uses_engine_config():
    # `truthy` collapses through the *engine's* configured rules — a
    # strict_boolean engine rejects non-bool coercion with an error
    # rather than coercing, exactly like `if` would.
    engine = Engine(config={"truthy_evaluator": "strict_boolean"})
    sess = engine.session()
    rule = engine.compile({"var": "flag"})
    assert sess.evaluate_truthy(rule, HANDLE) is True


# ---------------- shared typed semantics ----------------


def test_typed_rejects_foreign_rule(engine, sess):
    other = Engine()
    foreign = other.compile({"var": "n"})
    for method in (sess.evaluate_bool, sess.evaluate_int, sess.evaluate_float, sess.evaluate_truthy):
        with pytest.raises(EvaluateError) as exc_info:
            method(foreign, HANDLE)
        assert exc_info.value.error_type == "InvalidArgument"


def test_typed_propagates_runtime_errors(engine, sess):
    rule = engine.compile({"+": [{"var": "s"}, 1]})
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_bool(rule, HANDLE)
    # A genuine evaluation failure, not a mismatch.
    assert exc_info.value.error_type != "TypeMismatch"
    assert exc_info.value.operator == "+"


def test_mismatch_carries_empty_breadcrumbs(engine, sess):
    # Binding-detected failures have no engine breadcrumb: operator is
    # None, node_ids empty, path None.
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_bool(rule_for(engine, "n"), HANDLE)
    err = exc_info.value
    assert err.operator is None
    assert err.node_ids == []
    assert err.path is None
