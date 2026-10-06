"""Pins how this binding refuses a bad argument today.

The bindings do not agree (a budget of 0 is ``InvalidArgument`` here,
``InvalidArguments`` in Node, a ``ParseError`` in WASM and the engine's own
budget through the C ABI), so the cross-binding scenarios cannot hold these
cases; each binding pins its own until one spelling is chosen. Note that
this binding itself spells it two ways, depending on the call.
"""

import json
import threading

import pytest

from datalogic_py import BatchItemError, DataHandle, Engine, EvaluateError


def error_type(fn):
    with pytest.raises(EvaluateError) as info:
        fn()
    return info.value.error_type


def test_budget_zero_is_invalid_argument():
    engine = Engine()
    rule = engine.compile({"+": [1, 2]})
    assert error_type(lambda: rule.evaluate_metered(None, 0)) == "InvalidArgument"
    assert error_type(lambda: engine.eval_metered({"+": [1, 2]}, None, 0)) == "InvalidArgument"
    # None falls back to the engine's budget.
    assert rule.evaluate_metered(None, None)[0] == "3"


@pytest.mark.parametrize("escape", ["", "ab"])
def test_a_long_template_key_escape_is_invalid_arguments(escape):
    assert (
        error_type(lambda: Engine(templating=True, template_key_escape=escape))
        == "InvalidArguments"
    )


def test_an_unknown_mode_is_invalid_arguments():
    engine = Engine()
    assert error_type(lambda: engine.check({"var": "a"}, "loose")) == "InvalidArguments"
    assert (
        error_type(lambda: engine.evaluate_with_trace('{"var": "a"}', "{}", "loose"))
        == "InvalidArguments"
    )


def test_an_unknown_config_key_or_family_is_a_configuration_error():
    assert error_type(lambda: Engine(config={"bogus": 1})) == "ConfigurationError"
    assert error_type(lambda: Engine(families=["Strings"])) == "ConfigurationError"


def test_a_rule_from_another_engine_is_invalid_argument():
    rule = Engine().compile({"var": "a"})
    session = Engine().session()
    data = DataHandle('{"a": 1}')
    assert error_type(lambda: session.evaluate_data(rule, data)) == "InvalidArgument"
    assert error_type(lambda: session.evaluate_bool(rule, data)) == "InvalidArgument"
    [item] = session.evaluate_many([rule], data)
    assert isinstance(item, BatchItemError)
    assert item.tag == "InvalidArgument"
    assert item.message == "rule was compiled by a different engine than this session's"


def test_a_traced_result_keeps_its_key_order():
    text = Engine().evaluate_with_trace(
        '{"var": "o"}', '{"o": {"z": 1, "a": {"y": 2, "b": 3}}}'
    )
    assert text.startswith('{"result":{"z":1,"a":{"y":2,"b":3}},'), text
    assert list(json.loads(text)["result"]) == ["z", "a"]


def test_compiling_from_many_threads():
    # Compile and check release the GIL; many threads compiling at once
    # must still each get their own rule.
    engine = Engine()
    rules = {}

    def work(i):
        rule = {"+": [{"var": "x"}] + [i] * 50}
        assert engine.check(rule) == []
        rules[i] = engine.compile(json.dumps(rule)).evaluate({"x": 1})

    threads = [threading.Thread(target=work, args=(i,)) for i in range(16)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert rules == {i: 1 + 50 * i for i in range(16)}
