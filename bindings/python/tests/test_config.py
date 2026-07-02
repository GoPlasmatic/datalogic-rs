"""Engine `config=` kwarg wired through the core's shared JSON config parser."""

import pytest

from datalogic_py import DataLogicError, Engine, EvaluateError


def test_default_engine_coerces_empty_string():
    # Baseline for the strict-preset test below: default numeric coercion
    # treats "" as 0 in arithmetic.
    assert Engine().eval({"+": ["", 1]}, {}) == 1


def test_strict_preset_changes_behavior():
    strict = Engine(config={"preset": "strict"})
    # Strict rejects non-numeric coercion, so the same rule that returns 1
    # on a default engine raises here.
    with pytest.raises(EvaluateError):
        strict.eval({"+": ["", 1]}, {})
    # Strict turns float division by zero into an error instead of the
    # default saturated value. (Integer/integer division by zero errors
    # under every config; only the float path is configurable.)
    with pytest.raises(EvaluateError):
        strict.eval({"/": [1.5, 0]}, {})


def test_division_by_zero_return_null():
    engine = Engine(config={"division_by_zero": "return_null"})
    assert engine.eval({"/": [1.5, 0]}, {}) is None
    # The default engine saturates instead.
    assert Engine().eval({"/": [1.5, 0]}, {}) is not None


def test_config_accepts_json_string():
    engine = Engine(config='{"division_by_zero": "return_null"}')
    assert engine.eval({"/": [1.5, 0]}, {}) is None


def test_preset_plus_override():
    # The preset applies first; remaining keys override on top of it.
    engine = Engine(config={"preset": "strict", "division_by_zero": "return_null"})
    assert engine.eval({"/": [1.5, 0]}, {}) is None


def test_config_combines_with_templating():
    engine = Engine(templating=True, config={"preset": "strict"})
    assert engine.eval({"ok": {"==": [1, 1]}}, {}) == {"ok": True}


def test_unknown_config_key_raises():
    with pytest.raises(EvaluateError, match="unknown config key"):
        Engine(config={"not_a_knob": True})


def test_unknown_preset_raises():
    with pytest.raises(EvaluateError, match="unknown preset"):
        Engine(config={"preset": "bogus"})


def test_invalid_config_json_string_raises():
    with pytest.raises(EvaluateError, match="not valid JSON"):
        Engine(config="{ this is not json")


def test_non_object_config_raises():
    with pytest.raises(EvaluateError, match="must be a JSON object"):
        Engine(config="42")


def test_config_error_carries_core_message_and_tag():
    with pytest.raises(DataLogicError) as exc_info:
        Engine(config={"max_recursion_depth": 0})
    err = exc_info.value
    # The core message comes through verbatim, tagged as a configuration
    # failure.
    assert "max_recursion_depth" in str(err)
    assert err.error_type == "ConfigurationError"
