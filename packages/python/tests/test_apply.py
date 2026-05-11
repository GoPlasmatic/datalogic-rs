"""One-shot `apply()` — parity surface with classic JSONLogic bindings."""

from datalogic import apply


def test_apply_dict_inputs():
    rule = {"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]}
    assert apply(rule, {"score": 75}) == "pass"
    assert apply(rule, {"score": 25}) == "fail"


def test_apply_string_inputs():
    rule = '{"+": [{"var": "x"}, 1]}'
    data = '{"x": 41}'
    assert apply(rule, data) == 42


def test_apply_mixed_inputs():
    rule = {"+": [1, 2]}
    data = "null"
    assert apply(rule, data) == 3


def test_apply_nested_data():
    rule = {"var": "a.b.c"}
    data = {"a": {"b": {"c": "deep"}}}
    assert apply(rule, data) == "deep"


def test_apply_returns_dict():
    # The `merge` operator returns an array; `cat` returns a string. Pick
    # something that returns object-shaped output through normal operators.
    rule = {"var": "user"}
    data = {"user": {"name": "Ada", "active": True}}
    assert apply(rule, data) == {"name": "Ada", "active": True}


def test_apply_returns_array():
    rule = {"var": "items"}
    data = {"items": [1, 2, 3]}
    assert apply(rule, data) == [1, 2, 3]


def test_apply_boolean_result():
    assert apply({"==": [1, 1]}, {}) is True
    assert apply({"==": [1, 2]}, {}) is False


def test_apply_null_result():
    rule = {"var": "missing_with_default"}
    # `var` returns null when the key is missing and there's no default.
    assert apply({"var": ["missing", None]}, {}) is None
