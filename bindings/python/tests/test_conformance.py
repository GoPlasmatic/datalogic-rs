"""JSONLogic conformance suites, run through the Python binding.

Mirrors the core runner (crates/datalogic-rs/tests/test_jsonlogic.rs):
same file discovery (suites/index.json), same case semantics (string
entries are section headers, `templating` selects a templating engine,
`error` cases must fail with the expected error shape). Every case in
the suites becomes one parametrized test here; anything that cannot be
executed surfaces as an explicit skip with a reason, never silently.
"""

import json
from pathlib import Path

import pytest

from datalogic_py import DataLogicError, Engine

# tests/ -> python/ -> bindings/ -> repo root
SUITES_DIR = (
    Path(__file__).resolve().parents[3] / "crates" / "datalogic-rs" / "tests" / "suites"
)

# One engine per templating variant, shared across all cases (the core
# runner builds one per case; Engine construction is cheap but not free,
# and both engines are stateless across evaluations).
_ENGINES = {False: Engine(), True: Engine(templating=True)}


def _collect_cases():
    """Build one pytest param per suite case, mirroring the core runner's
    discovery: read index.json, skip (visibly) anything unreadable."""
    index_path = SUITES_DIR / "index.json"
    if not index_path.exists():
        return [
            pytest.param(
                None,
                id="index.json",
                marks=pytest.mark.skip(
                    reason=f"suites directory not found at {SUITES_DIR}"
                ),
            )
        ]

    params = []
    for suite_name in json.loads(index_path.read_text()):
        suite_path = SUITES_DIR / suite_name
        if not suite_path.exists():
            # The core runner warns and moves on; a skip is our visible
            # equivalent.
            params.append(
                pytest.param(
                    None,
                    id=suite_name,
                    marks=pytest.mark.skip(
                        reason="listed in index.json but file not found"
                    ),
                )
            )
            continue
        for index, case in enumerate(json.loads(suite_path.read_text())):
            if isinstance(case, str):
                continue  # section header
            description = case.get("description", "No description")
            params.append(
                pytest.param(case, id=f"{suite_name}:{index}:{description}")
            )
    return params


def _json_equal(a, b):
    """Structural JSON equality. Unlike plain `==`, bools never equal
    numbers (mirroring serde_json); int == float is intentionally allowed
    (Python parses `1` and `1.0` into different types, both valid JSON
    encodings of the same number)."""
    if isinstance(a, bool) or isinstance(b, bool):
        return isinstance(a, bool) and isinstance(b, bool) and a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_json_equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_json_equal(x, y) for x, y in zip(a, b))
    return a == b


def _as_engine_input(value):
    """Prepare a suite value (rule or data) for `Engine.eval`.

    Non-strings cross as Python objects, which the binding converts to
    `serde_json::Value`. That is the same representation the core runner
    uses, so JSON objects iterate in sorted-key order on both sides; the
    object-iteration cases in array/map.json encode that order in their
    expected results. (Feeding JSON strings instead would exercise
    `DataValue::from_str`, which keeps document order and legitimately
    produces a different iteration order.) Strings are dumped to JSON
    because the binding reads a `str` argument as JSON text."""
    return json.dumps(value) if isinstance(value, str) else value


def _core_error_object(exc):
    """Reconstruct the error object the core runner compares against from
    the binding's exception surface, following the same three branches as
    test_jsonlogic.rs: thrown value, InvalidArguments message, and
    InvalidOperator. Returns None for any other error kind."""
    tag = getattr(exc, "error_type", None)
    message = str(exc)

    # The Display form appends " (in operator: X)" when the operator is
    # known; strip the exact suffix using the structured attribute.
    operator = getattr(exc, "operator", None)
    if operator:
        suffix = f" (in operator: {operator})"
        if message.endswith(suffix):
            message = message[: -len(suffix)]

    if tag == "InvalidOperator":
        return {"type": "Unknown Operator"}
    if tag == "Thrown" and message.startswith("Thrown: "):
        try:
            return json.loads(message[len("Thrown: ") :])
        except ValueError:
            return None
    if tag == "InvalidArguments" and message.startswith("Invalid arguments: "):
        return {"type": message[len("Invalid arguments: ") :]}
    return None


@pytest.mark.parametrize("case", _collect_cases())
def test_conformance(case):
    if "rule" not in case:
        pytest.fail("test case missing 'rule'")
    data = case["data"] if "data" in case else {}
    engine = _ENGINES[bool(case.get("templating", False))]

    expects_error = "error" in case
    if not expects_error and "result" not in case:
        pytest.fail("test case missing 'result' or 'error'")

    rule_input = _as_engine_input(case["rule"])
    data_input = _as_engine_input(data)

    if expects_error:
        with pytest.raises(DataLogicError) as exc_info:
            engine.eval(rule_input, data_input)
        expected = case["error"]
        got = _core_error_object(exc_info.value)
        if got is None or not _json_equal(got, expected):
            pytest.fail(
                f"expected error {expected!r}, got "
                f"{type(exc_info.value).__name__}"
                f"(error_type={getattr(exc_info.value, 'error_type', None)!r}): "
                f"{exc_info.value}"
            )
    else:
        result = engine.eval(rule_input, data_input)
        expected = case["result"]
        if not _json_equal(result, expected):
            pytest.fail(f"expected {expected!r}, got {result!r}")
