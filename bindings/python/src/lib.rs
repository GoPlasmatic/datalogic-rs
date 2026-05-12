//! Python bindings for `datalogic-rs`.
//!
//! See `bindings/python/README.md` for the user-facing API; this file is
//! the pyo3 wiring that exposes [`engine::Engine`], [`engine::Rule`],
//! [`session::Session`], the exception hierarchy in [`error`], and the
//! top-level [`apply`] convenience.

mod conv;
mod engine;
mod error;
mod session;

use pyo3::prelude::*;
use pyo3::types::PyAny;

use crate::engine::{Engine, Rule, compile_inner, evaluate_value};
use crate::error::{DataLogicError, EvaluateError, ParseError};
use crate::session::Session;

/// Top-level convenience: compile ``rule`` and evaluate against ``data``
/// in one call. Equivalent to ``Engine().compile(rule).evaluate(data)``.
///
/// Use this for ad-hoc one-shots. For repeated evaluations of the same
/// rule, hold an :class:`Engine` and a :class:`Rule` instance — that
/// path skips the per-call compile.
#[pyfunction]
fn apply(py: Python<'_>, rule: &Bound<'_, PyAny>, data: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let engine = std::sync::Arc::new(datalogic_rs::Engine::new());
    let logic = compile_inner(py, &engine, rule)?;
    evaluate_value(py, &engine, &logic, data)
}

#[pymodule]
fn datalogic_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();

    m.add_class::<Engine>()?;
    m.add_class::<Rule>()?;
    m.add_class::<Session>()?;

    m.add("DataLogicError", py.get_type::<DataLogicError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    m.add("EvaluateError", py.get_type::<EvaluateError>())?;

    m.add_function(wrap_pyfunction!(apply, m)?)?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
