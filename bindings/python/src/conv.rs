//! Python ↔ `serde_json::Value` conversion.
//!
//! [`dict_to_value`] consumes any Python value (`dict`, `list`, `str`,
//! `int`, `float`, `bool`, `None`) and produces a `serde_json::Value`.
//! [`value_to_pyobject`] does the reverse.
//!
//! Conversion failures (datetime, Decimal, bytes, set, tuple, NaN/Infinity,
//! …) surface as [`crate::error::ParseError`] with the underlying serde
//! message attached, so callers can `except ParseError` rather than wading
//! through cryptic serde diagnostics.
//!
//! For payloads with exotic Python types, use the `evaluate_str` /
//! `eval_str` paths instead — they take the JSON text directly so the
//! caller controls encoding.

use pyo3::prelude::*;
use pythonize::{depythonize, pythonize};
use serde_json::Value;

use crate::error::parse_error;

pub fn dict_to_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    depythonize(value).map_err(|e| parse_error(py, format!("unsupported Python value: {e}")))
}

pub fn value_to_pyobject(py: Python<'_>, value: &Value) -> PyResult<PyObject> {
    pythonize(py, value)
        .map(|bound| bound.unbind())
        .map_err(|e| parse_error(py, format!("failed to convert result to Python: {e}")))
}
