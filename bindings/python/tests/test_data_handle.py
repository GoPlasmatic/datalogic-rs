"""DataHandle — parse-once data handles and handle-based evaluation."""

import json
import threading

import pytest

from datalogic_py import DataHandle, Engine, EvaluateError, ParseError


def test_construct_and_allocated_bytes():
    handle = DataHandle('{"user": {"age": 42}}')
    # Property, not a method: input copy + parsed tree.
    assert isinstance(handle.allocated_bytes, int)
    assert handle.allocated_bytes > 0
    assert "DataHandle(allocated_bytes=" in repr(handle)


def test_malformed_json_raises_parse_error():
    with pytest.raises(ParseError):
        DataHandle("{ not json")


def test_construct_from_a_python_value():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "user.age"}, 1]})
    from_value = DataHandle({"user": {"age": 34}})
    from_json = DataHandle('{"user": {"age": 34}}')
    assert from_value.allocated_bytes > 0
    assert rule.evaluate_data(from_value) == 35
    assert rule.evaluate_data(from_value) == rule.evaluate_data(from_json)


@pytest.mark.parametrize(
    "doc",
    [
        {"a": [1, -2, 3.5, 2**63 - 1], "b": "text", "c": None, "d": True},
        {"ключ": {"日本": ["é", "", "😀", 'a"b']}, "": {"": None}},
        [[], {}, [[]], {"x": {}}],
        42,
        None,
        True,
        [1, "two", 3.0],
    ],
)
def test_value_handle_agrees_with_json_handle(doc):
    engine = Engine()
    whole = engine.compile({"var": ""})
    from_value = DataHandle(doc)
    from_json = DataHandle(json.dumps(doc))
    assert whole.evaluate_data(from_value) == whole.evaluate_data(from_json)
    assert whole.evaluate_data(from_value) == doc


def test_a_str_is_always_json_text():
    # A str argument is parsed as JSON, as before: to hold a string value,
    # pass its JSON form.
    engine = Engine()
    whole = engine.compile({"var": ""})
    assert whole.evaluate_data(DataHandle('"just a string"')) == "just a string"
    with pytest.raises(ParseError):
        DataHandle("just a string")


def test_value_handle_is_independent_of_its_source():
    engine = Engine()
    rule = engine.compile({"cat": [{"var": "s"}, "!"]})
    source = {"s": "kept"}
    handle = DataHandle(source)
    source["s"] = "changed"
    assert rule.evaluate_data(handle) == "kept!"


def test_fallback_shapes_match_evaluate():
    # Shapes the direct walk does not cover go through the same pythonize
    # path `evaluate` uses, so a handle holds what `evaluate` would see.
    engine = Engine()
    whole = engine.compile({"var": ""})
    for doc in ({7}, (1, 2)):
        assert whole.evaluate_data(DataHandle(doc)) == whole.evaluate(doc)


def test_unconvertible_value_raises():
    with pytest.raises(Exception):
        DataHandle(object())


def test_rule_evaluate_data():
    engine = Engine()
    rule = engine.compile({">=": [{"var": "age"}, 18]})
    handle = DataHandle('{"age": 25}')
    assert rule.evaluate_data(handle) is True


def test_rule_evaluate_data_str():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    handle = DataHandle('{"x": 41}')
    result = rule.evaluate_data_str(handle)
    assert isinstance(result, str)
    assert result == "42"


def test_session_evaluate_data():
    engine = Engine()
    rule = engine.compile({"var": "name"})
    handle = DataHandle('{"name": "Ada"}')
    sess = engine.session()
    assert sess.evaluate_data(rule, handle) == "Ada"
    assert sess.evaluate_data_str(rule, handle) == '"Ada"'


def test_evaluate_data_matches_evaluate():
    """The handle path returns the same Python objects the dict/str
    paths return, including container results."""
    engine = Engine()
    rule = engine.compile({"var": ""})
    payload = {"a": [1, 2.5, None, True, "s"], "b": {"nested": "x"}}
    handle = DataHandle(json.dumps(payload))
    sess = engine.session()
    via_dict = rule.evaluate(payload)
    assert rule.evaluate_data(handle) == via_dict
    assert sess.evaluate_data(rule, handle) == via_dict


def test_handle_not_consumed_and_reusable():
    engine = Engine()
    add = engine.compile({"+": [{"var": "x"}, 1]})
    mul = engine.compile({"*": [{"var": "x"}, 2]})
    handle = DataHandle('{"x": 10}')
    for _ in range(3):
        assert add.evaluate_data(handle) == 11
        assert mul.evaluate_data(handle) == 20


def test_handle_is_engine_independent():
    # One handle feeds rules compiled by *different* engines.
    handle = DataHandle('{"x": 5}')
    for _ in range(2):
        engine = Engine()
        rule = engine.compile({"var": "x"})
        assert rule.evaluate_data(handle) == 5


def test_handle_shared_across_threads_for_reads():
    engine = Engine()
    rule = engine.compile({"reduce": [{"var": "items"}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]})
    handle = DataHandle(json.dumps({"items": list(range(100))}))
    expected = sum(range(100))
    errors = []

    def worker():
        try:
            for _ in range(50):
                if rule.evaluate_data(handle) != expected:
                    errors.append("wrong result")
        except Exception as e:  # pragma: no cover - failure detail
            errors.append(repr(e))

    threads = [threading.Thread(target=worker) for _ in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert errors == []


def test_session_evaluate_data_rejects_foreign_rule():
    # Mirrors the C ABI's check_pair: handle-based session entry points
    # verify the rule belongs to the session's engine.
    engine = Engine()
    other = Engine()
    foreign = other.compile({"var": "x"})
    handle = DataHandle('{"x": 1}')
    sess = engine.session()
    with pytest.raises(EvaluateError) as exc_info:
        sess.evaluate_data(foreign, handle)
    assert exc_info.value.error_type == "InvalidArgument"
    with pytest.raises(EvaluateError):
        sess.evaluate_data_str(foreign, handle)


def test_evaluate_data_propagates_runtime_errors():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "s"}, 1]})
    handle = DataHandle('{"s": "not a number"}')
    with pytest.raises(EvaluateError) as exc_info:
        rule.evaluate_data(handle)
    assert exc_info.value.operator == "+"
    sess = engine.session()
    with pytest.raises(EvaluateError):
        sess.evaluate_data(rule, handle)


def test_session_arena_reset_between_data_calls():
    # evaluate_data resets at the start of each call, like evaluate.
    engine = Engine()
    rule = engine.compile({"merge": [{"var": "a"}, {"var": "a"}]})
    handle = DataHandle(json.dumps({"a": list(range(200))}))
    sess = engine.session()
    sess.evaluate_data(rule, handle)
    high_water = sess.allocated_bytes()
    for _ in range(20):
        sess.evaluate_data(rule, handle)
    # Reused arena: repeated calls must not grow allocation ~linearly.
    assert sess.allocated_bytes() <= high_water * 2
