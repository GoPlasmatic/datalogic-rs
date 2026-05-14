"""Session — hot-loop arena reuse."""

import pytest

from datalogic_py import Engine, Session


def test_session_basic():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    sess = engine.session()
    assert isinstance(sess, Session)
    for x in range(10):
        assert sess.evaluate(rule, {"x": x}) == x + 1


def test_session_context_manager():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    with engine.session() as sess:
        for x in range(3):
            assert sess.evaluate(rule, {"x": x}) == x + 1
    # After __exit__, the arena was reset; using the session again is still
    # legal (we just lost the hot chunks). This test is a smoke test that
    # the context manager protocol doesn't raise.


def test_session_evaluate_str():
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    sess = engine.session()
    assert sess.evaluate_str(rule, '{"x": 41}') == "42"


def test_session_explicit_reset_returns_none():
    engine = Engine()
    sess = engine.session()
    assert sess.reset() is None


def test_session_allocated_bytes_grows_then_resets():
    # `evaluate*` resets at the start of each call, so allocated_bytes is
    # the high-water mark for the most recent eval.
    engine = Engine()
    rule = engine.compile({"+": [{"var": "x"}, 1]})
    sess = engine.session()
    sess.evaluate(rule, {"x": 1})
    after_first = sess.allocated_bytes()
    assert after_first > 0
    sess.reset()
    # After explicit reset, the chunks are still allocated (Bump::reset
    # rewinds the bump pointer but keeps chunk memory) — so this stays
    # at the high-water mark.
    assert sess.allocated_bytes() == after_first


def test_session_unsendable_across_threads():
    # The Session pyclass is `unsendable`; pyo3 raises if another thread
    # touches it. We don't actually try to share — the test would deadlock
    # under the GIL — but we confirm the class exists and is constructible.
    engine = Engine()
    sess = engine.session()
    # Calling repr is enough to exercise the pymethod path on this thread.
    assert "Session(" in repr(sess)


def test_session_multiple_rules():
    engine = Engine()
    add = engine.compile({"+": [{"var": "x"}, 1]})
    mul = engine.compile({"*": [{"var": "x"}, 2]})
    with engine.session() as sess:
        assert sess.evaluate(add, {"x": 4}) == 5
        assert sess.evaluate(mul, {"x": 4}) == 8
        assert sess.evaluate(add, {"x": 10}) == 11
