//! `Session` pyclass — hot-loop arena reuse, single-threaded — plus the
//! ABI v2 tiers built on it: data-handle evaluation, typed scalar
//! results, and batch entry points. Semantics mirror
//! `bindings/c/src/session.rs` so every binding reports the same
//! outcomes.
//!
//! The pyclass is `unsendable` because it owns a [`bumpalo::Bump`] that is
//! `!Sync`. PyO3 enforces that only the thread that constructed a Session
//! can call its methods. The arena is reset at the start of each
//! `evaluate*` call to bound peak memory across iterations — the previous
//! call's owned result is already materialised by then. (Batch calls
//! reset between items instead, matching the C ABI.)

use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::datavalue::DataValue;
use datalogic_rs::{Engine as RsEngine, Error as DlError, Logic};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PyString};
use serde_json::Value;

use crate::conv::{datavalue_to_pyobject, dict_to_value, value_to_pyobject};
use crate::data::{DataHandle, build_py_tree};
use crate::engine::{DetachInput, Rule, eval_borrowing};
use crate::error::{engine_error_to_pyerr, evaluate_error_with_type};

#[pyclass(name = "Session", module = "datalogic_py", unsendable)]
pub struct Session {
    engine: Arc<RsEngine>,
    arena: Bump,
}

impl Session {
    pub(crate) fn new(engine: Arc<RsEngine>) -> Self {
        Self {
            engine,
            arena: Bump::new(),
        }
    }

    /// Mirror of the C ABI's `check_pair`: every handle-based session
    /// entry point verifies the rule belongs to this session's engine.
    fn check_same_engine(&self, py: Python<'_>, rule: &Rule) -> PyResult<()> {
        if Arc::ptr_eq(&self.engine, rule.engine_arc()) {
            Ok(())
        } else {
            Err(evaluate_error_with_type(
                py,
                "rule was compiled by a different engine than this session's".to_string(),
                "InvalidArgument",
            ))
        }
    }

    /// Shared body of the typed entry points: reset, evaluate over the
    /// session arena with the GIL released, then project the borrowed
    /// result through `extract`.
    fn typed_eval<T>(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &DataHandle,
        extract: impl for<'v> FnOnce(&'v DataValue<'v>, &RsEngine) -> Result<T, PyErr>,
    ) -> PyResult<T> {
        self.check_same_engine(py, rule)?;
        self.arena.reset();
        let res = eval_borrowing(
            py,
            &self.engine,
            rule.logic(),
            DetachInput::Tree(data.tree.value()),
            &mut self.arena,
        );
        match res {
            Ok(av) => extract(av, &self.engine),
            Err(e) => Err(engine_error_to_pyerr(py, &e, Some(rule.logic()))),
        }
    }
}

/// JSON type name for TypeMismatch messages — keep the wording aligned
/// with `type_of` in `bindings/c/src/session.rs`.
fn type_of(v: &DataValue<'_>) -> &'static str {
    if v.is_null() {
        "null"
    } else if v.is_bool() {
        "boolean"
    } else if v.is_number() {
        "number"
    } else if v.is_string() {
        "string"
    } else if v.is_array() {
        "array"
    } else {
        "object"
    }
}

/// Per-item failure inside :meth:`Session.evaluate_batch` /
/// :meth:`Session.evaluate_many`. Item failures never raise — the batch
/// result list holds a ``BatchItemError`` in the failed item's slot
/// (and JSON-string results in every successful slot), so one bad
/// payload can't abort the other N-1.
///
/// ``tag`` is the stable engine error tag (e.g. ``"NaN"``,
/// ``"Thrown"``), ``operator`` the outermost failing operator when
/// known.
#[pyclass(name = "BatchItemError", module = "datalogic_py", frozen)]
pub struct BatchItemError {
    /// Stable error tag (`datalogic_rs::Error::tag()`), or
    /// `"InvalidArgument"` for handle/rule argument problems.
    #[pyo3(get)]
    pub tag: String,
    /// Human-readable failure message.
    #[pyo3(get)]
    pub message: String,
    /// Outermost failing operator, when known.
    #[pyo3(get)]
    pub operator: Option<String>,
}

#[pymethods]
impl BatchItemError {
    fn __repr__(&self) -> String {
        match &self.operator {
            Some(op) => format!(
                "BatchItemError(tag={:?}, operator={:?}, message={:?})",
                self.tag, op, self.message
            ),
            None => format!(
                "BatchItemError(tag={:?}, message={:?})",
                self.tag, self.message
            ),
        }
    }
}

/// Owned item-failure detail carried out of the GIL-released batch
/// loop; converted into [`BatchItemError`] once the GIL is back.
struct ItemFailure {
    tag: String,
    message: String,
    operator: Option<String>,
}

impl ItemFailure {
    fn from_engine(e: &DlError) -> Self {
        Self {
            tag: e.tag().to_string(),
            message: e.to_string(),
            operator: e.operator().map(str::to_owned),
        }
    }
}

/// Materialise batch outcomes as a Python list: JSON ``str`` for
/// successes, [`BatchItemError`] instances for failures.
fn batch_outcomes_to_pylist(
    py: Python<'_>,
    outcomes: Vec<Result<String, ItemFailure>>,
) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for outcome in outcomes {
        match outcome {
            Ok(json) => list.append(json)?,
            Err(f) => list.append(Py::new(
                py,
                BatchItemError {
                    tag: f.tag,
                    message: f.message,
                    operator: f.operator,
                },
            )?)?,
        }
    }
    Ok(list.into_any().unbind())
}

#[pymethods]
impl Session {
    /// Evaluate ``rule`` against ``data`` and return the result as a Python value.
    fn evaluate(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Reset BEFORE each call so the previous iteration's allocations
        // don't accumulate. The previous call's result was materialised
        // into Python objects (or a `String`) before returning, so
        // resetting here is safe.
        self.arena.reset();

        if let Ok(s) = data.cast::<PyString>() {
            let text = s.to_str()?.to_string();
            let res = eval_borrowing(
                py,
                &self.engine,
                rule.logic(),
                DetachInput::Str(&text),
                &mut self.arena,
            );
            return match res {
                Ok(av) => datavalue_to_pyobject(py, av),
                Err(e) => Err(engine_error_to_pyerr(py, &e, Some(rule.logic()))),
            };
        }
        match build_py_tree(data) {
            Ok(tree) => {
                let res = eval_borrowing(
                    py,
                    &self.engine,
                    rule.logic(),
                    DetachInput::Tree(tree.value()),
                    &mut self.arena,
                );
                match res {
                    Ok(av) => datavalue_to_pyobject(py, av),
                    Err(e) => Err(engine_error_to_pyerr(py, &e, Some(rule.logic()))),
                }
            }
            Err(_) => {
                let value = dict_to_value(py, data)?;
                let json = run_to_value(py, &self.engine, &mut self.arena, rule, &value)?;
                value_to_pyobject(py, &json)
            }
        }
    }

    /// Evaluate ``rule`` against ``data`` (a JSON ``str``) and return the
    /// result as a JSON ``str``.
    fn evaluate_str(&mut self, py: Python<'_>, rule: &Rule, data: &str) -> PyResult<String> {
        self.arena.reset();
        let engine = self.engine.clone();
        let logic = rule.logic().clone();
        let arena: &mut Bump = &mut self.arena;
        let data_owned = data.to_string();
        // `move` is load-bearing: without it the closure captures
        // `arena` by `&` (reborrow only needs immutable access), giving
        // a captured field of type `&&mut Bump`. That's `!Send` because
        // `Bump: !Sync`. With `move`, the closure owns `&mut Bump`
        // directly — `&mut Bump: Send` (since `Bump: Send`).
        py.detach(move || -> Result<String, datalogic_rs::Error> {
            let av = engine.evaluate(&logic, data_owned.as_str(), arena)?;
            Ok(av.to_string())
        })
        .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
    }

    /// Evaluate ``rule`` against a pre-parsed :class:`DataHandle` using
    /// this session's arena and return the result as a Python value —
    /// the hot path: zero parse work per call.
    ///
    /// The rule must have been compiled by the same :class:`Engine` this
    /// session was opened on. Like every ``Session`` method,
    /// single-threaded.
    fn evaluate_data(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &DataHandle,
    ) -> PyResult<Py<PyAny>> {
        self.check_same_engine(py, rule)?;
        self.arena.reset();
        let res = eval_borrowing(
            py,
            &self.engine,
            rule.logic(),
            DetachInput::Tree(data.tree.value()),
            &mut self.arena,
        );
        match res {
            Ok(av) => datavalue_to_pyobject(py, av),
            Err(e) => Err(engine_error_to_pyerr(py, &e, Some(rule.logic()))),
        }
    }

    /// Same as :meth:`evaluate_data`, returning the result as a JSON
    /// ``str``.
    fn evaluate_data_str(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &DataHandle,
    ) -> PyResult<String> {
        self.check_same_engine(py, rule)?;
        self.arena.reset();
        let engine: &RsEngine = &self.engine;
        let logic: &Logic = rule.logic();
        let tree = &data.tree;
        let arena: &mut Bump = &mut self.arena;
        py.detach(move || -> Result<String, datalogic_rs::Error> {
            let av = engine.evaluate(logic, tree.value(), arena)?;
            Ok(av.to_string())
        })
        .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
    }

    /// Evaluate and return the result as a strict JSON boolean. Any
    /// other result type raises :class:`EvaluateError` with
    /// ``error_type == "TypeMismatch"``; for JSONLogic truthiness
    /// coercion use :meth:`evaluate_truthy`.
    fn evaluate_bool(&mut self, py: Python<'_>, rule: &Rule, data: &DataHandle) -> PyResult<bool> {
        self.typed_eval(py, rule, data, |av, _| {
            av.as_bool().ok_or_else(|| {
                evaluate_error_with_type(
                    py,
                    format!("result is not a boolean (got {})", type_of(av)),
                    "TypeMismatch",
                )
            })
        })
    }

    /// Evaluate and return the result as an ``int``. A non-number, or a
    /// number that is not an exact integer, raises
    /// :class:`EvaluateError` with ``error_type == "TypeMismatch"``.
    fn evaluate_int(&mut self, py: Python<'_>, rule: &Rule, data: &DataHandle) -> PyResult<i64> {
        self.typed_eval(py, rule, data, |av, _| {
            av.as_i64().ok_or_else(|| {
                evaluate_error_with_type(
                    py,
                    format!("result is not an integer number (got {})", type_of(av)),
                    "TypeMismatch",
                )
            })
        })
    }

    /// Evaluate and return the result as a ``float``. Accepts any JSON
    /// number; raises :class:`EvaluateError` with
    /// ``error_type == "TypeMismatch"`` otherwise.
    fn evaluate_float(&mut self, py: Python<'_>, rule: &Rule, data: &DataHandle) -> PyResult<f64> {
        self.typed_eval(py, rule, data, |av, _| {
            av.as_f64().ok_or_else(|| {
                evaluate_error_with_type(
                    py,
                    format!("result is not a number (got {})", type_of(av)),
                    "TypeMismatch",
                )
            })
        })
    }

    /// Evaluate and collapse the result to a ``bool`` via the engine's
    /// configured truthiness rules (the same coercion ``if``/``and``/
    /// ``or`` apply). Never type-mismatches — any result
    /// truthy-converts.
    fn evaluate_truthy(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &DataHandle,
    ) -> PyResult<bool> {
        self.typed_eval(py, rule, data, |av, engine| Ok(engine.truthy(av)))
    }

    /// Evaluate one rule against many pre-parsed data handles in a
    /// single native call, returning one item per input, in order: the
    /// JSON ``str`` result on success, a :class:`BatchItemError` on
    /// per-item failure. Item failures never raise and never abort the
    /// remaining items; only argument-level problems (a rule from a
    /// different engine, a non-``DataHandle`` list element, …) raise.
    ///
    /// The arena is reset between items, so peak memory tracks the
    /// largest single evaluation, not the batch.
    fn evaluate_batch(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        handles: Vec<Py<DataHandle>>,
    ) -> PyResult<Py<PyAny>> {
        self.check_same_engine(py, rule)?;
        if handles.is_empty() {
            return Ok(PyList::empty(py).into_any().unbind());
        }
        let trees: Vec<_> = handles.iter().map(|h| &h.get().tree).collect();
        let engine: &RsEngine = &self.engine;
        let logic: &Logic = rule.logic();
        let arena: &mut Bump = &mut self.arena;
        let outcomes: Vec<Result<String, ItemFailure>> = py.detach(move || {
            trees
                .iter()
                .map(|tree| {
                    // Scratch from the previous item is dead — its result
                    // was materialised into an owned String.
                    arena.reset();
                    match engine.evaluate(logic, tree.value(), &*arena) {
                        Ok(av) => Ok(av.to_string()),
                        Err(e) => Err(ItemFailure::from_engine(&e)),
                    }
                })
                .collect()
        });
        batch_outcomes_to_pylist(py, outcomes)
    }

    /// Evaluate many rules against one pre-parsed data handle in a
    /// single native call — the rule-set / feature-flag shape. Same
    /// per-item semantics and result shape as :meth:`evaluate_batch`;
    /// a rule compiled by a different engine fails **its own item**
    /// (``tag == "InvalidArgument"``) without affecting the rest.
    fn evaluate_many(
        &mut self,
        py: Python<'_>,
        rules: Vec<Py<Rule>>,
        data: &DataHandle,
    ) -> PyResult<Py<PyAny>> {
        if rules.is_empty() {
            return Ok(PyList::empty(py).into_any().unbind());
        }
        let rule_refs: Vec<&Rule> = rules.iter().map(|r| r.get()).collect();
        let engine_arc = &self.engine;
        let engine: &RsEngine = &self.engine;
        let tree = &data.tree;
        let arena: &mut Bump = &mut self.arena;
        let outcomes: Vec<Result<String, ItemFailure>> = py.detach(move || {
            rule_refs
                .iter()
                .map(|rule| {
                    if !Arc::ptr_eq(engine_arc, rule.engine_arc()) {
                        return Err(ItemFailure {
                            tag: "InvalidArgument".to_string(),
                            message: "rule was compiled by a different engine than this session's"
                                .to_string(),
                            operator: None,
                        });
                    }
                    arena.reset();
                    match engine.evaluate(rule.logic(), tree.value(), &*arena) {
                        Ok(av) => Ok(av.to_string()),
                        Err(e) => Err(ItemFailure::from_engine(&e)),
                    }
                })
                .collect()
        });
        batch_outcomes_to_pylist(py, outcomes)
    }

    /// Reset the underlying arena. Returns no value. Calling this is
    /// optional — `evaluate*` resets at the start of each call.
    fn reset(&mut self) {
        self.arena.reset();
    }

    /// Bytes currently allocated to the session's arena (sum of all
    /// chunks). Useful for sizing or diagnostics.
    fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> bool {
        self.arena.reset();
        false
    }

    fn __repr__(&self) -> String {
        format!("Session(allocated_bytes={})", self.arena.allocated_bytes())
    }
}

/// pythonize-fallback evaluation over the session arena. Only reached
/// for input shapes the direct walk doesn't cover.
fn run_to_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    arena: &mut Bump,
    rule: &Rule,
    value: &Value,
) -> PyResult<Value> {
    let engine = engine.clone();
    let logic = rule.logic().clone();
    py.detach(move || -> Result<Value, datalogic_rs::Error> {
        let av = engine.evaluate(&logic, value, arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
}
