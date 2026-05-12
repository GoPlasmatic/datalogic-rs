//! `Session` pyclass — hot-loop arena reuse, single-threaded.
//!
//! The pyclass is `unsendable` because it owns a [`bumpalo::Bump`] that is
//! `!Sync`. PyO3 enforces that only the thread that constructed a Session
//! can call its methods. The arena is reset at the start of each
//! `evaluate*` call to bound peak memory across iterations — the previous
//! call's owned result is already materialised by then.

use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;
use datalogic_rs::bumpalo::Bump;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};
use serde_json::Value;

use crate::conv::{dict_to_value, value_to_pyobject};
use crate::engine::Rule;
use crate::error::engine_error_to_pyerr;

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
}

#[pymethods]
impl Session {
    /// Evaluate ``rule`` against ``data`` and return the result as a Python value.
    fn evaluate(
        &mut self,
        py: Python<'_>,
        rule: &Rule,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<PyObject> {
        // Reset BEFORE each call so the previous iteration's allocations
        // don't accumulate. The previous call's result was materialised as
        // an owned `serde_json::Value` (or `String`) before returning, so
        // resetting here is safe.
        self.arena.reset();

        if let Ok(s) = data.downcast::<PyString>() {
            let json = run_to_value_from_str(py, &self.engine, &mut self.arena, rule, s.to_str()?)?;
            return value_to_pyobject(py, &json);
        }
        let value = dict_to_value(py, data)?;
        let json = run_to_value(py, &self.engine, &mut self.arena, rule, &value)?;
        value_to_pyobject(py, &json)
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
        py.allow_threads(move || -> Result<String, datalogic_rs::Error> {
            let av = engine.evaluate(&logic, data_owned.as_str(), arena)?;
            Ok(av.to_string())
        })
        .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
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

fn run_to_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    arena: &mut Bump,
    rule: &Rule,
    value: &Value,
) -> PyResult<Value> {
    let engine = engine.clone();
    let logic = rule.logic().clone();
    py.allow_threads(move || -> Result<Value, datalogic_rs::Error> {
        let av = engine.evaluate(&logic, value, arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
}

fn run_to_value_from_str(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    arena: &mut Bump,
    rule: &Rule,
    data: &str,
) -> PyResult<Value> {
    let data_owned = data.to_string();
    let engine = engine.clone();
    let logic = rule.logic().clone();
    py.allow_threads(move || -> Result<Value, datalogic_rs::Error> {
        let av = engine.evaluate(&logic, data_owned.as_str(), arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(rule.logic())))
}
