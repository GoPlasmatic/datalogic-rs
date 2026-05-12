//! `Engine` and `Rule` pyclasses — the heart of the binding.

use std::collections::HashMap;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error as DlError, Logic, Result as DlResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};
use serde_json::Value;

use crate::conv::{dict_to_value, value_to_pyobject};
use crate::error::engine_error_to_pyerr;
use crate::session::Session;

/// JSONLogic compile/evaluate engine.
///
/// Construct once at startup and share across threads — `Engine` is
/// internally `Arc<datalogic_rs::Engine>` and Python's reference semantics
/// mean every reference points at the same underlying engine.
///
/// # Custom operators
///
/// Pass ``custom_operators={"name": callable, ...}`` to register custom
/// JSONLogic operators. Each callable receives the evaluated args as a
/// JSON-array string and must return a JSON string of the result::
///
///     engine = Engine(custom_operators={
///         "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2),
///     })
///     engine.eval_str('{"double": [21]}', '{}')  # "42"
///
/// Callbacks run while the GIL is held; the binding re-acquires the GIL
/// inside each operator call even if the surrounding evaluation released
/// it. **Built-ins win over custom registrations of the same name** — the
/// engine's built-in dispatcher never reaches a custom op with a name like
/// ``"+"`` or ``"if"``.
#[pyclass(name = "Engine", module = "datalogic_py", frozen)]
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

#[pymethods]
impl Engine {
    /// Create a new engine.
    ///
    /// :param templating: when ``True``, multi-key objects in compiled
    ///     rules become output-shaping templates (the engine's "templating
    ///     mode"). Off by default.
    /// :param custom_operators: optional dict ``{name: callable}`` whose
    ///     values are ``Callable[[str], str]`` — JSON-array string in,
    ///     JSON value string out.
    #[new]
    #[pyo3(signature = (*, templating = false, custom_operators = None))]
    fn new(templating: bool, custom_operators: Option<HashMap<String, Py<PyAny>>>) -> Self {
        let mut builder = if templating {
            RsEngine::builder().with_templating(true)
        } else {
            RsEngine::builder()
        };
        if let Some(map) = custom_operators {
            for (name, callback) in map {
                builder = builder.add_operator(name.clone(), PyOperator { name, callback });
            }
        }
        Self {
            inner: Arc::new(builder.build()),
        }
    }

    /// Compile a JSONLogic rule into a reusable [`Rule`].
    ///
    /// :param rule: a Python ``dict``/``list``/scalar describing the rule,
    ///     or a ``str`` containing the rule as JSON.
    fn compile(&self, py: Python<'_>, rule: &Bound<'_, PyAny>) -> PyResult<Rule> {
        let logic = compile_inner(py, &self.inner, rule)?;
        Ok(Rule {
            engine: self.inner.clone(),
            logic,
        })
    }

    /// One-shot evaluation. Compiles ``rule`` against ``data`` and returns
    /// the result as a Python value (``dict``/``list``/scalar/``None``).
    ///
    /// For repeated evaluations of the same rule, prefer
    /// :meth:`compile` + :meth:`Rule.evaluate` — it skips re-parsing.
    fn eval(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<PyObject> {
        let logic = compile_inner(py, &self.inner, rule)?;
        evaluate_value(py, &self.inner, &logic, data)
    }

    /// One-shot evaluation returning the result as a JSON ``str``.
    fn eval_str(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let logic = compile_inner(py, &self.inner, rule)?;
        evaluate_str(py, &self.inner, &logic, data)
    }

    /// Open a hot-loop [`Session`] bound to this engine. The session
    /// reuses one bumpalo arena across calls and is reset between
    /// evaluations to bound peak memory.
    ///
    /// Sessions are **not thread-safe** — open one per thread.
    fn session(&self) -> Session {
        Session::new(self.inner.clone())
    }

    fn __repr__(&self) -> String {
        "Engine()".to_string()
    }
}

/// A compiled JSONLogic rule.
///
/// Hold one and call :meth:`evaluate` against many data inputs without
/// re-parsing. ``Rule`` is thread-safe — share the same instance across
/// worker threads to evaluate in parallel; the binding releases the GIL
/// around each Rust evaluate call.
#[pyclass(name = "Rule", module = "datalogic_py", frozen)]
pub struct Rule {
    engine: Arc<RsEngine>,
    logic: Arc<Logic>,
}

impl Rule {
    pub(crate) fn logic(&self) -> &Arc<Logic> {
        &self.logic
    }
}

#[pymethods]
impl Rule {
    /// Evaluate against ``data`` and return the result as a Python value.
    ///
    /// :param data: a Python ``dict``/``list``/scalar, or a ``str``
    ///     containing the data as JSON. The dict path uses ``pythonize``
    ///     (≈3-10× faster than a JSON round-trip).
    fn evaluate(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        evaluate_value(py, &self.engine, &self.logic, data)
    }

    /// Evaluate against ``data`` (a JSON ``str``) and return the result as
    /// a JSON ``str``. Skips dict ↔ value conversion entirely — the
    /// fastest path through the binding.
    fn evaluate_str(&self, py: Python<'_>, data: &str) -> PyResult<String> {
        // Capture sendable references for the GIL-released closure.
        let engine: &RsEngine = &self.engine;
        let logic: &Logic = &self.logic;
        let result = py.allow_threads(|| -> Result<String, datalogic_rs::Error> {
            let arena = Bump::new();
            let av = engine.evaluate(logic, data, &arena)?;
            Ok(av.to_string())
        });
        result.map_err(|e| engine_error_to_pyerr(py, &e, Some(&self.logic)))
    }

    fn __repr__(&self) -> String {
        "Rule(<compiled>)".to_string()
    }
}

// =============== Custom operator bridge ===============

/// Custom operator backed by a Python callable. Args cross the boundary
/// as a JSON-array string; the return must be a JSON string.
struct PyOperator {
    name: String,
    callback: Py<PyAny>,
}

impl CustomOperator for PyOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Build the args JSON array.
        let mut json = String::from("[");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&a.to_json_string());
        }
        json.push(']');

        // 2. Acquire the GIL and call the Python callable. `with_gil`
        //    re-acquires the GIL even if the surrounding evaluation
        //    released it via `py.allow_threads`.
        let result_str: String = Python::with_gil(|py| -> Result<String, DlError> {
            let callable = self.callback.bind(py);
            let ret = callable.call1((json.as_str(),)).map_err(|e| {
                DlError::custom_message(format!("custom operator '{}' raised: {}", self.name, e))
            })?;
            ret.extract::<String>().map_err(|e| {
                DlError::custom_message(format!(
                    "custom operator '{}' must return a JSON string: {}",
                    self.name, e
                ))
            })
        })?;

        // 3. Parse the returned JSON into the eval arena so the borrowed
        //    `DataValue` outlives this call.
        let arena_str = arena.alloc_str(&result_str);
        let parsed = DataValue::from_str(arena_str, arena).map_err(|e| {
            DlError::custom_message(format!(
                "custom operator '{}' returned invalid JSON: {}",
                self.name, e
            ))
        })?;
        Ok(arena.alloc(parsed))
    }
}

// ---------------- shared helpers ----------------

pub(crate) fn compile_inner(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    rule: &Bound<'_, PyAny>,
) -> PyResult<Arc<Logic>> {
    if let Ok(s) = rule.downcast::<PyString>() {
        let s = s.to_str()?;
        return engine
            .compile_arc(s)
            .map_err(|e| engine_error_to_pyerr(py, &e, None));
    }
    let value = dict_to_value(py, rule)?;
    engine
        .compile_arc(&value)
        .map_err(|e| engine_error_to_pyerr(py, &e, None))
}

pub(crate) fn evaluate_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    // Fast path: if the caller already has a JSON string, skip dict conversion.
    if let Ok(s) = data.downcast::<PyString>() {
        let json = run_eval_to_value_from_str(py, engine, logic, s.to_str()?)?;
        return value_to_pyobject(py, &json);
    }
    let value = dict_to_value(py, data)?;
    let json = run_eval_to_value(py, engine, logic, &value)?;
    value_to_pyobject(py, &json)
}

pub(crate) fn evaluate_str(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &Bound<'_, PyAny>,
) -> PyResult<String> {
    if let Ok(s) = data.downcast::<PyString>() {
        let s_owned = s.to_str()?.to_string();
        let engine_ref: &RsEngine = engine;
        let logic_ref: &Logic = logic;
        return py
            .allow_threads(|| -> Result<String, datalogic_rs::Error> {
                let arena = Bump::new();
                let av = engine_ref.evaluate(logic_ref, s_owned.as_str(), &arena)?;
                Ok(av.to_string())
            })
            .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)));
    }
    let value = dict_to_value(py, data)?;
    let engine_ref: &RsEngine = engine;
    let logic_ref: &Logic = logic;
    py.allow_threads(|| -> Result<String, datalogic_rs::Error> {
        let arena = Bump::new();
        let av = engine_ref.evaluate(logic_ref, &value, &arena)?;
        Ok(av.to_string())
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)))
}

fn run_eval_to_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    value: &Value,
) -> PyResult<Value> {
    let engine_ref: &RsEngine = engine;
    let logic_ref: &Logic = logic;
    py.allow_threads(|| -> Result<Value, datalogic_rs::Error> {
        let arena = Bump::new();
        let av = engine_ref.evaluate(logic_ref, value, &arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)))
}

fn run_eval_to_value_from_str(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &str,
) -> PyResult<Value> {
    let data_owned = data.to_string();
    let engine_ref: &RsEngine = engine;
    let logic_ref: &Logic = logic;
    py.allow_threads(|| -> Result<Value, datalogic_rs::Error> {
        let arena = Bump::new();
        let av = engine_ref.evaluate(logic_ref, data_owned.as_str(), &arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)))
}
