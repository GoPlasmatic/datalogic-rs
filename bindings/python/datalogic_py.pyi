# Type stubs for the `datalogic_py` extension module (PEP 561).
#
# The module is a compiled pyo3 extension; this file is its complete
# typed surface. Keep it in sync with the `#[pymethods]` blocks in
# `src/{lib,engine,session,data,error}.rs`; the release workflow
# type-checks `examples/` against these stubs, and maturin bundles the
# file (plus a `py.typed` marker) into every wheel because it sits next
# to `pyproject.toml` and is named after `[tool.maturin] module-name`.

from collections.abc import Callable, Mapping, Sequence
from typing import Any, final

__all__ = [
    "Engine",
    "Rule",
    "Session",
    "DataHandle",
    "BatchItemError",
    "DataLogicError",
    "ParseError",
    "EvaluateError",
    "apply",
    "__version__",
]

__version__: str

class DataLogicError(Exception):
    """Base exception raised by datalogic_py.

    Engine-reported failures carry structured attributes (set on the
    raised instance, on this class and both subclasses):

    - ``error_type``: stable engine error tag (e.g. ``"ParseError"``,
      ``"TypeMismatch"``, ``"Thrown"``, ``"NaN"``).
    - ``operator``: outermost failing operator, or ``None``.
    - ``node_ids``: leaf-to-root breadcrumb of compiled-node ids.
    - ``path``: root-to-leaf list of step dicts (``node_id``,
      ``operator``, ``arg_index``, ``json_pointer``) when the binding
      could resolve it, else ``None``.
    """

    error_type: str
    operator: str | None
    node_ids: list[int]
    path: list[dict[str, Any]] | None

class ParseError(DataLogicError):
    """Raised when a rule or data input cannot be parsed."""

class EvaluateError(DataLogicError):
    """Raised when an operator fails at evaluation time."""

def apply(rule: Any, data: Any) -> Any:
    """Compile ``rule`` and evaluate against ``data`` in one call.

    Equivalent to ``Engine().compile(rule).evaluate(data)``. Use for
    ad-hoc one-shots; for repeated evaluations hold an :class:`Engine`
    and a compiled :class:`Rule`.
    """

@final
class Engine:
    """JSONLogic compile/evaluate engine.

    Construct once at startup and share freely: the engine is
    internally reference-counted and thread-safe.
    """

    def __new__(
        cls,
        *,
        templating: bool = False,
        custom_operators: Mapping[str, Callable[[str], str]] | None = None,
        config: Mapping[str, Any] | str | None = None,
    ) -> Engine: ...
    def compile(self, rule: Any) -> Rule:
        """Compile a JSONLogic rule (dict/list/scalar, or a JSON str)."""

    def eval(self, rule: Any, data: Any) -> Any:
        """One-shot: compile ``rule`` and evaluate against ``data``."""

    def eval_str(self, rule: Any, data: Any) -> str:
        """One-shot evaluation returning the result as a JSON str."""

    def evaluate_with_trace(self, logic: str, data: str) -> str:
        """Evaluate with step-by-step tracing (both args JSON strs).

        Returns a JSON envelope ``{"result", "expression_tree",
        "steps", "error"?, "structured_error"?}``; runtime failures are
        reported inside the envelope instead of raising.
        """

    def session(self) -> Session:
        """Open a hot-loop session bound to this engine.

        Sessions are not thread-safe; open one per thread.
        """

@final
class Rule:
    """A compiled JSONLogic rule.

    Thread-safe: share one instance across threads and call
    :meth:`evaluate` in parallel.
    """

    def evaluate(self, data: Any) -> Any:
        """Evaluate against ``data`` (dict/list/scalar, or a JSON str)."""

    def evaluate_str(self, data: str) -> str:
        """Evaluate against a JSON str, returning a JSON str."""

    def evaluate_data(self, data: DataHandle) -> Any:
        """Evaluate against a pre-parsed handle (zero parse per call)."""

    def evaluate_data_str(self, data: DataHandle) -> str:
        """Like :meth:`evaluate_data`, returning a JSON str."""

@final
class Session:
    """Hot-loop evaluation session reusing one memory arena.

    Not thread-safe: only the thread that created a session may call
    its methods. Usable as a context manager:
    ``with engine.session() as sess: ...``
    """

    def evaluate(self, rule: Rule, data: Any) -> Any:
        """Evaluate ``rule`` against ``data`` (dict/list/scalar or JSON str)."""

    def evaluate_str(self, rule: Rule, data: str) -> str:
        """Evaluate against a JSON str, returning a JSON str."""

    def evaluate_data(self, rule: Rule, data: DataHandle) -> Any:
        """Evaluate against a pre-parsed handle (the hot path)."""

    def evaluate_data_str(self, rule: Rule, data: DataHandle) -> str:
        """Like :meth:`evaluate_data`, returning a JSON str."""

    def evaluate_bool(self, rule: Rule, data: DataHandle) -> bool:
        """Strict-boolean result; raises ``EvaluateError`` (``error_type
        == "TypeMismatch"``) for any other result type."""

    def evaluate_int(self, rule: Rule, data: DataHandle) -> int:
        """Exact-integer result; raises TypeMismatch otherwise."""

    def evaluate_float(self, rule: Rule, data: DataHandle) -> float:
        """Any JSON number; raises TypeMismatch otherwise."""

    def evaluate_truthy(self, rule: Rule, data: DataHandle) -> bool:
        """Collapse the result via the engine's truthiness rules;
        never type-mismatches."""

    def evaluate_batch(
        self, rule: Rule, handles: Sequence[DataHandle]
    ) -> list[str | BatchItemError]:
        """One rule, many data handles, one native call. Per-item
        failures land as :class:`BatchItemError` slots, never raise."""

    def evaluate_many(
        self, rules: Sequence[Rule], data: DataHandle
    ) -> list[str | BatchItemError]:
        """Many rules, one data handle: the feature-flag shape. Same
        per-item semantics as :meth:`evaluate_batch`."""

    def reset(self) -> None:
        """Reset the arena (optional; ``evaluate*`` resets per call)."""

    def allocated_bytes(self) -> int:
        """Bytes currently held by the session's arena."""

    def __enter__(self) -> Session: ...
    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> bool: ...

@final
class DataHandle:
    """An immutable, pre-parsed JSON document: parse once, evaluate many.

    Independent of any engine, shareable across threads for reads.
    """

    def __new__(cls, json: str) -> DataHandle: ...
    @property
    def allocated_bytes(self) -> int:
        """Bytes held by the handle's backing arena."""

@final
class BatchItemError:
    """Per-item failure inside a batch evaluation (never raised)."""

    tag: str
    message: str
    operator: str | None
