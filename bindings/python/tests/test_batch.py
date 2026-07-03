"""Batch evaluation — one rule x many handles, many rules x one handle.

Mirrors the C ABI batch semantics: item failures never raise and never
abort the remaining items; the failed slot holds a `BatchItemError`
(tag/message/operator) while every successful slot holds the item's
JSON string. Only argument-level problems raise.
"""

import pytest

from datalogic_py import BatchItemError, DataHandle, Engine, EvaluateError


@pytest.fixture()
def engine():
    return Engine()


@pytest.fixture()
def sess(engine):
    return engine.session()


def test_evaluate_batch_success_order(engine, sess):
    rule = engine.compile({">": [{"var": "age"}, 18]})
    handles = [DataHandle(f'{{"age": {age}}}') for age in (25, 10, 19)]
    results = sess.evaluate_batch(rule, handles)
    assert results == ["true", "false", "true"]
    assert all(type(r) is str for r in results)


def test_evaluate_batch_item_failure_does_not_raise(engine, sess):
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    handles = [
        DataHandle('{"x": 1}'),
        DataHandle('{"x": "boom"}'),  # arithmetic on a string fails
        DataHandle('{"x": 3}'),
    ]
    results = sess.evaluate_batch(rule, handles)
    assert results[0] == "2"
    assert results[2] == "4"
    err = results[1]
    assert isinstance(err, BatchItemError)
    assert err.tag  # stable engine tag, e.g. "NaN"/"TypeError"
    assert err.message
    assert err.operator == "+"
    assert "BatchItemError(tag=" in repr(err)


def test_evaluate_batch_empty_returns_empty_list(engine, sess):
    rule = engine.compile({"var": "x"})
    assert sess.evaluate_batch(rule, []) == []


def test_evaluate_batch_argument_errors_raise(engine, sess):
    rule = engine.compile({"var": "x"})
    # Non-DataHandle elements are an argument error, not an item error.
    with pytest.raises(TypeError):
        sess.evaluate_batch(rule, [DataHandle("{}"), '{"x": 1}'])
    # A rule from a different engine fails the call, not the items.
    foreign = Engine().compile({"var": "x"})
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_batch(foreign, [DataHandle("{}")])
    assert exc_info.value.error_type == "InvalidArgument"


def test_evaluate_many_success_order(engine, sess):
    rules = [
        engine.compile({"var": "age"}),
        engine.compile({">": [{"var": "age"}, 18]}),
        engine.compile({"cat": ["age:", {"var": "age"}]}),
    ]
    handle = DataHandle('{"age": 25}')
    results = sess.evaluate_many(rules, handle)
    assert results == ["25", "true", '"age:25"']


def test_evaluate_many_item_failure_isolated(engine, sess):
    good = engine.compile({"var": "age"})
    bad = engine.compile({"throw": "kaput"})
    handle = DataHandle('{"age": 25}')
    results = sess.evaluate_many([good, bad, good], handle)
    assert results[0] == "25"
    assert results[2] == "25"
    err = results[1]
    assert isinstance(err, BatchItemError)
    assert err.tag == "Thrown"
    assert "kaput" in err.message


def test_evaluate_many_foreign_rule_fails_its_item_only(engine, sess):
    # Mirrors the C ABI: in evaluate_many the engine check is per item
    # (the rule *is* the item), so one foreign rule cannot poison the
    # rest of the rule set.
    good = engine.compile({"var": "x"})
    foreign = Engine().compile({"var": "x"})
    handle = DataHandle('{"x": 7}')
    results = sess.evaluate_many([good, foreign, good], handle)
    assert results[0] == "7"
    assert results[2] == "7"
    err = results[1]
    assert isinstance(err, BatchItemError)
    assert err.tag == "InvalidArgument"
    assert "different engine" in err.message
    assert err.operator is None


def test_evaluate_many_empty_returns_empty_list(engine, sess):
    handle = DataHandle("{}")
    assert sess.evaluate_many([], handle) == []


def test_evaluate_many_argument_errors_raise(engine, sess):
    handle = DataHandle("{}")
    with pytest.raises(TypeError):
        sess.evaluate_many(["not a rule"], handle)


def test_batch_item_error_is_not_an_exception():
    # It is a plain result object; callers branch on isinstance, they
    # don't catch it.
    assert not issubclass(BatchItemError, BaseException)


def test_batch_arena_stays_bounded(engine, sess):
    # The arena is reset between items — a 200-item batch must not hold
    # ~200x one item's allocation.
    rule = engine.compile({"merge": [{"var": "a"}, {"var": "a"}]})
    handle = DataHandle('{"a": [1, 2, 3, 4, 5, 6, 7, 8]}')
    sess.evaluate_batch(rule, [handle])
    single = sess.allocated_bytes()
    sess.evaluate_batch(rule, [handle] * 200)
    assert sess.allocated_bytes() <= max(single, 1) * 8
