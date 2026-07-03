//! Exception hierarchy exposed to Python.
//!
//! ```text
//! Exception
//! └── DataLogicError          base for everything raised by this binding
//!     ├── ParseError          rule/data parse failure or unsupported Python type
//!     └── EvaluateError       runtime operator failure
//!                              .error_type — stable tag from datalogic_rs::Error::tag()
//!                              .operator   — outermost failing operator (or None)
//!                              .node_ids   — leaf-to-root breadcrumb of compiled-node ids
//!                              .path       — list of {operator, json_pointer, ...} dicts
//!                                            (populated when the binding has the compiled
//!                                            Logic at hand to resolve)
//! ```
//!
//! Conversion goes through [`engine_error_to_pyerr`] (or its helpers below)
//! rather than `From` so the binding can attach `.path` when it has the
//! `&Logic` available — the `From` trait can't take that extra argument.

use datalogic_rs::{Error, Logic};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// First arg to `create_exception!` is the module name the exception
// reports itself under; must match `[lib] name` in Cargo.toml.
create_exception!(
    datalogic_py,
    DataLogicError,
    PyException,
    "Base exception raised by datalogic_py."
);
create_exception!(
    datalogic_py,
    ParseError,
    DataLogicError,
    "Raised when a rule or data input cannot be parsed."
);
create_exception!(
    datalogic_py,
    EvaluateError,
    DataLogicError,
    "Raised when an operator fails at evaluation time. Carries .error_type, .operator, .node_ids, and (when resolvable) .path."
);

/// Convert any [`datalogic_rs::Error`] into the right Python exception
/// instance, attaching structured attributes. When `compiled` is provided
/// the breadcrumb is resolved into a list of step dicts attached as `.path`.
pub fn engine_error_to_pyerr(py: Python<'_>, err: &Error, compiled: Option<&Logic>) -> PyErr {
    let tag = err.tag();
    let message = err.to_string();

    // ParseError is the only kind that maps to the ParseError exception
    // class. Everything else (TypeError, NaN/Thrown, Custom, …) is a
    // runtime evaluation failure from the caller's perspective.
    let pyerr = if tag == "ParseError" {
        PyErr::new::<ParseError, _>(message)
    } else {
        PyErr::new::<EvaluateError, _>(message)
    };

    attach_attrs(py, &pyerr, err, compiled);
    pyerr
}

/// Convenience for the parse-stage path where there is no `Error` value yet
/// (e.g. the binding rejected a Python `datetime` before handing anything to
/// the engine). Raises `ParseError` with `.error_type = "ParseError"` and no
/// other attributes.
pub fn parse_error<S: Into<String>>(py: Python<'_>, message: S) -> PyErr {
    let pyerr = PyErr::new::<ParseError, _>(message.into());
    if let Ok(value) = pyerr.value(py).cast::<PyAny>() {
        let _ = value.setattr("error_type", "ParseError");
        let _ = value.setattr("operator", py.None());
        let _ = value.setattr("node_ids", Vec::<u32>::new());
        let _ = value.setattr("path", py.None());
    }
    pyerr
}

/// Build an [`EvaluateError`] for failures the binding detects itself —
/// typed-result mismatches (`error_type = "TypeMismatch"`) and argument
/// problems (`error_type = "InvalidArgument"`), mirroring the C ABI's
/// tags so every binding reports these identically. No engine `Error`
/// exists in these paths, so the breadcrumb attributes are empty.
pub fn evaluate_error_with_type(py: Python<'_>, message: String, error_type: &str) -> PyErr {
    let pyerr = PyErr::new::<EvaluateError, _>(message);
    let value = pyerr.value(py);
    let _ = value.setattr("error_type", error_type);
    let _ = value.setattr("operator", py.None());
    let _ = value.setattr("node_ids", Vec::<u32>::new());
    let _ = value.setattr("path", py.None());
    pyerr
}

fn attach_attrs(py: Python<'_>, pyerr: &PyErr, err: &Error, compiled: Option<&Logic>) {
    let value = pyerr.value(py);

    let _ = value.setattr("error_type", err.tag());

    match err.operator() {
        Some(op) => {
            let _ = value.setattr("operator", op);
        }
        None => {
            let _ = value.setattr("operator", py.None());
        }
    }

    let node_ids: Vec<u32> = err.node_ids().to_vec();
    let _ = value.setattr("node_ids", node_ids);

    // `.path` is the resolved, root-to-leaf list of step dicts. We can only
    // produce this when the caller hands us the compiled `Logic` — the
    // raw breadcrumb on its own doesn't carry operator names.
    let path_value = compiled
        .map(|c| serialize_path(py, &err.resolve_path(c)))
        .unwrap_or_else(|| py.None());
    let _ = value.setattr("path", path_value);
}

fn serialize_path(py: Python<'_>, steps: &[datalogic_rs::PathStep]) -> Py<PyAny> {
    let list = pyo3::types::PyList::empty(py);
    for step in steps {
        let dict = pyo3::types::PyDict::new(py);
        let _ = dict.set_item("node_id", step.node_id);
        let _ = dict.set_item("operator", step.operator.as_deref());
        let _ = dict.set_item("arg_index", step.arg_index);
        let _ = dict.set_item("json_pointer", step.json_pointer.as_str());
        let _ = list.append(dict);
    }
    list.into()
}
