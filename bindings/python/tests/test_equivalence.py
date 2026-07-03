"""Dict-path / string-path / handle-path equivalence corpus.

The dict input path uses a direct Python-to-arena walk and the result
path a direct arena-to-Python walk (`src/conv.rs`). This corpus pins
them to the string path (`DataValue::from_str` in, same result walk
out): every case must produce *strictly* equal results through all
three input routes — same values, same types (bool is not int, int is
not float), same exception class and `error_type` on failure.

Two layers:

- a hand corpus of the conversion edge cases (bool-vs-int, whole
  floats, u64-range ints, non-finite floats, unicode, deep nesting,
  key ordering),
- several JSONLogic conformance suites re-run through both input
  paths (the suites' own expectations are covered by
  test_conformance.py; here we only require path agreement).
"""

import json
import math
from pathlib import Path

import pytest

from datalogic_py import DataHandle, DataLogicError, Engine

SUITES_DIR = (
    Path(__file__).resolve().parents[3] / "crates" / "datalogic-rs" / "tests" / "suites"
)

# A diverse slice of the conformance battery: arithmetic, comparisons,
# control flow, object iteration (key-order sensitive!), strings,
# structured objects, truthiness, var access, and datetime results.
# now.json is excluded: its results are time-dependent and the two
# paths evaluate at different instants.
SUITE_FILES = [
    "compatible.json",
    "arithmetic/plus.json",
    "comparison/softEquals.json",
    "control/if.json",
    "array/map.json",
    "array/reduce.json",
    "string/string.json",
    "structured-objects.json",
    "truthiness.json",
    "val.json",
    "datetime/datetime.json",
]

_ENGINES = {False: Engine(), True: Engine(templating=True)}


def strictly_equal(a, b):
    """Recursive equality where types matter: True != 1, 1 != 1.0."""
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(strictly_equal(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(strictly_equal(x, y) for x, y in zip(a, b))
    if isinstance(a, float):
        return (math.isnan(a) and math.isnan(b)) or a == b
    return a == b


def assert_paths_agree(engine, rule_json, data):
    """Evaluate `data` through the dict path, the string path, and the
    handle path; all outcomes (value or exception) must agree.

    The JSON text is dumped with sort_keys=True: the dict path presents
    objects to the engine in sorted-key order (the serde_json BTreeMap
    order the binding always had), while `DataValue::from_str` keeps
    document order — so a sorted document is the canonical common form.
    Object *iteration* order is observable (array/map.json), which is
    exactly why the sort is part of the pinned dict-path contract."""
    rule = engine.compile(rule_json)
    data_json = json.dumps(data, sort_keys=True)
    handle = DataHandle(data_json)

    outcomes = []
    for label, call in (
        ("dict", lambda: rule.evaluate(data)),
        ("str", lambda: rule.evaluate(data_json)),
        ("handle", lambda: rule.evaluate_data(handle)),
    ):
        try:
            outcomes.append((label, "ok", call()))
        except DataLogicError as e:
            outcomes.append((label, "err", (type(e).__name__, e.error_type)))

    (l0, k0, v0) = outcomes[0]
    for label, kind, value in outcomes[1:]:
        assert kind == k0, f"{l0} -> {k0} {v0!r} but {label} -> {kind} {value!r}"
        if kind == "ok":
            assert strictly_equal(value, v0), f"{l0} -> {v0!r} but {label} -> {value!r}"
        else:
            assert value == v0, f"{l0} raised {v0!r} but {label} raised {value!r}"
    return v0 if k0 == "ok" else None


# ---------------- hand corpus ----------------

ECHO_PAYLOADS = [
    {"int": 7, "neg": -3, "zero": 0, "large": 2**53},
    {"float": 2.5, "whole_float": 1.0, "tiny": 1e-9, "big": 1.7e308},
    {"bool_true": True, "bool_false": False, "none": None},
    {"s": "text", "empty": "", "unicode": "héllo 🎉 日本語", "quotes": 'a"b\\c'},
    {"list": [1, [2, [3, [4]]]], "empty_list": [], "empty_dict": {}},
    {"mixed": [True, 1, 1.0, "1", None, {"k": [0.5]}]},
    {"zebra": 1, "apple": 2, "Mango": 3, "_x": 4},  # key sorting
    {"nested": {"c": {"b": {"a": [{"z": 1, "y": 2}]}}}},
    [1, 2, 3],
    42,
    2.5,
    True,
    None,
    {},
    [],
]


@pytest.mark.parametrize("payload", ECHO_PAYLOADS, ids=lambda p: repr(p)[:50])
def test_echo_equivalence(payload):
    engine = _ENGINES[False]
    assert_paths_agree(engine, '{"var": ""}', payload)


def test_int_range_semantics_pin_the_legacy_dict_path():
    # The dict path keeps exact ints through the whole i64 range and
    # degrades to float in (i64::MAX, u64::MAX] — precisely what
    # depythonize → serde_json always did. (The *string* path's parser
    # legitimately floats the extreme 19-digit boundaries, a
    # pre-existing cross-path difference outside this corpus's scope.)
    engine = _ENGINES[False]
    rule = engine.compile({"var": "x"})
    for n in (2**63 - 1, -(2**63)):
        result = rule.evaluate({"x": n})
        assert result == n and type(result) is int
    result = rule.evaluate({"x": 2**63 + 5})
    assert type(result) is float and result == float(2**63 + 5)


def test_nonfinite_floats_equal_null_on_both_paths():
    # The legacy dict path collapsed NaN/inf to JSON null
    # (serde_json::Number::from_f64); the direct walk must too. The
    # string path can't express non-finite numbers, so pin against the
    # explicit-null payload instead.
    engine = _ENGINES[False]
    rule = engine.compile({"var": ""})
    for weird in (float("nan"), float("inf"), float("-inf")):
        assert rule.evaluate({"x": weird}) == rule.evaluate({"x": None}) == {"x": None}


def test_dict_result_key_order_matches_legacy_sorting():
    # Result dicts come back key-sorted (the legacy path's BTreeMap
    # order), for input echoes and rule-constructed objects alike.
    engine = _ENGINES[False]
    rule = engine.compile({"var": ""})
    assert list(rule.evaluate({"b": 1, "a": 2, "C": 3}).keys()) == ["C", "a", "b"]

    templating = _ENGINES[True]
    shaped = templating.compile({"zebra": {"var": "x"}, "apple": {"var": "x"}})
    assert list(shaped.evaluate({"x": 1}).keys()) == ["apple", "zebra"]


def test_object_iteration_sees_sorted_keys():
    # Object iteration order is observable through iterating operators;
    # the dict path must present sorted keys — the order the legacy
    # `serde_json::Value` (BTreeMap) input always presented, and the
    # order the object-iteration conformance cases encode.
    engine = _ENGINES[False]
    rule = engine.compile({"map": [{"var": ""}, {"var": ""}]})
    # Insertion order b-then-a; iteration must be a-then-b regardless.
    assert rule.evaluate({"b": "second", "a": "first"}) == ["first", "second"]


@pytest.mark.parametrize(
    "payload",
    [
        {"t": (1, 2)},
        {"s": {1, 2}},
        {"f": frozenset({3})},
        {"huge": 2**70},
        {1: "non-str key"},
        {"b": b"bytes"},
    ],
    ids=("tuple", "set", "frozenset", "bigint", "nonstr-key", "bytes"),
)
def test_fallback_shapes_keep_legacy_behaviour(payload):
    """Inputs the direct walk doesn't cover fall back to pythonize and
    must behave exactly as before: tuples/sets convert to arrays,
    out-of-range ints / non-str keys / bytes raise ParseError."""
    from datalogic_py import ParseError

    engine = _ENGINES[False]
    rule = engine.compile({"var": ""})
    key = next(iter(payload))
    value = payload[key]
    if isinstance(value, tuple):
        assert rule.evaluate(payload) == {key: list(value)}
    elif isinstance(value, (set, frozenset)):
        assert sorted(rule.evaluate(payload)[key]) == sorted(value)
    else:
        with pytest.raises(ParseError):
            rule.evaluate(payload)


# ---------------- suite corpus ----------------


def _collect_suite_cases():
    params = []
    for name in SUITE_FILES:
        path = SUITES_DIR / name
        if not path.exists():  # pragma: no cover - repo layout guard
            params.append(
                pytest.param(
                    None,
                    id=name,
                    marks=pytest.mark.skip(reason=f"suite not found at {path}"),
                )
            )
            continue
        for index, case in enumerate(json.loads(path.read_text())):
            if isinstance(case, str) or "rule" not in case:
                continue  # section header
            data = case.get("data", {})
            if isinstance(data, str):
                # A bare-string datum can't take the dict path (any str
                # argument is read as JSON text) — nothing to compare.
                continue
            desc = case.get("description", "no description")
            params.append(pytest.param(case, id=f"{name}:{index}:{desc}"))
    return params


@pytest.mark.parametrize("case", _collect_suite_cases())
def test_suite_case_paths_agree(case):
    engine = _ENGINES[bool(case.get("templating", False))]
    assert_paths_agree(engine, json.dumps(case["rule"]), case.get("data", {}))
