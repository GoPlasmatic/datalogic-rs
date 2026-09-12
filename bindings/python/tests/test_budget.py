"""Operation budget: what an evaluation costs, and what happens when it
costs too much.

The count is the engine's to define, so these assert the properties the
binding promises — a cost is reported, a ceiling is enforced, the error
carries both numbers — rather than exact figures a new fast path in the
core would move.
"""

import json

import pytest

from datalogic_py import Engine, EvaluateError

MAP = {"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]}
SMALL = {"xs": [1, 2, 3]}
LARGE = {"xs": list(range(200))}


def test_eval_metered_reports_the_result_and_the_cost():
    result, ops = Engine().eval_metered(MAP, SMALL)
    assert json.loads(result) == [2, 4, 6]
    # One per dispatched node plus one per item; pin the floor, not the
    # exact number.
    assert ops >= 3


def test_a_constant_folded_rule_costs_nothing():
    _, ops = Engine().eval_metered({"+": [1, 2]}, {})
    assert ops == 0


def test_an_explicit_budget_refuses_an_evaluation_that_would_cross_it():
    with pytest.raises(EvaluateError) as exc_info:
        Engine().eval_metered(MAP, LARGE, 10)
    err = exc_info.value
    assert err.error_type == "BudgetExceeded"
    assert err.budget == 10
    assert err.spent > 10


def test_a_budget_that_fits_is_not_refused():
    _, ops = Engine().eval_metered(MAP, SMALL)
    assert Engine().eval_metered(MAP, SMALL, ops)[1] == ops
    with pytest.raises(EvaluateError):
        Engine().eval_metered(MAP, SMALL, ops - 1)


def test_the_ops_budget_config_key_bounds_every_entry_point():
    engine = Engine(config={"ops_budget": 10})
    for call in (
        lambda: engine.eval(MAP, LARGE),
        lambda: engine.eval_str(MAP, LARGE),
        lambda: engine.compile(MAP).evaluate(LARGE),
    ):
        with pytest.raises(EvaluateError) as exc_info:
            call()
        assert exc_info.value.error_type == "BudgetExceeded"
    # ...and an explicit argument overrides it for one call.
    assert engine.eval_metered(MAP, LARGE, 1_000_000)[1] > 10


def test_metering_falls_back_to_the_configured_budget():
    engine = Engine(config={"ops_budget": 10})
    with pytest.raises(EvaluateError):
        engine.eval_metered(MAP, LARGE)


def test_try_cannot_recover_from_an_exhausted_budget():
    engine = Engine(config={"ops_budget": 10})
    with pytest.raises(EvaluateError) as exc_info:
        engine.eval(({"try": [MAP, "fallback"]}), LARGE)
    assert exc_info.value.error_type == "BudgetExceeded"


def test_rule_evaluate_metered_meters_a_compiled_rule():
    rule = Engine().compile(MAP)
    result, ops = rule.evaluate_metered(SMALL)
    assert json.loads(result) == [2, 4, 6]
    assert ops >= 3
    with pytest.raises(EvaluateError):
        rule.evaluate_metered(LARGE, 5)


def test_metering_accepts_a_json_string_as_well_as_a_dict():
    result, ops = Engine().eval_metered(json.dumps(MAP), json.dumps(SMALL))
    assert json.loads(result) == [2, 4, 6]
    assert ops >= 3


def test_an_unset_budget_leaves_evaluation_unbounded():
    result, _ = Engine().eval_metered(MAP, LARGE)
    assert len(json.loads(result)) == 200


def test_an_invalid_budget_in_the_config_is_a_configuration_error():
    from datalogic_py import DataLogicError

    for bad in (0, -1, "many"):
        with pytest.raises(DataLogicError):
            Engine(config={"ops_budget": bad})


def test_tensor_operators_are_priced_by_the_elements_they_move():
    _, ops = Engine().eval_metered({"zeros": [[16, 16], "f32"]}, {})
    assert ops >= 256
