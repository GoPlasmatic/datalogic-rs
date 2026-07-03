//! Python ↔ engine-value conversion.
//!
//! Two generations live here:
//!
//! - **Direct converters** (the hot path): [`py_to_datavalue`] walks a
//!   Python value straight into an arena-backed [`DataValue`] tree, and
//!   [`datavalue_to_pyobject`] walks an engine result straight back into
//!   Python objects. One walk per direction, no `serde_json::Value`
//!   intermediate. Both preserve the exact observable semantics of the
//!   pythonize path they replace (measured equivalence: see
//!   `tests/test_equivalence.py`): dict keys sort like `serde_json`'s
//!   `BTreeMap` did, non-finite floats become JSON `null`, bools stay
//!   bools, ints in `(i64::MAX, u64::MAX]` become floats.
//!
//! - **pythonize fallback**: [`dict_to_value`] / [`value_to_pyobject`]
//!   convert via `serde_json::Value`. The direct input walk covers the
//!   exact built-in types (`dict`, `list`, `tuple`, `str`, `int`,
//!   `float`, `bool`, `None`); anything else — subclasses, sets,
//!   mappings, dataclasses, out-of-range ints, unencodable strings —
//!   signals [`Unsupported`] and the caller re-runs the input through
//!   this path, which either handles it or raises exactly the
//!   [`crate::error::ParseError`] it always raised (datetime, Decimal,
//!   bytes, …).
//!
//! For payloads with exotic Python types, use the `evaluate_str` /
//! `eval_str` paths instead — they take the JSON text directly so the
//! caller controls encoding.

use datalogic_rs::DataValue;
use datalogic_rs::bumpalo::Bump;
use datalogic_rs::bumpalo::collections::Vec as BumpVec;
use datalogic_rs::datavalue::NumberValue;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pythonize::{depythonize, pythonize};
use serde_json::Value;

use crate::error::parse_error;

/// Signal that the direct walk met a shape it doesn't cover; the caller
/// must fall back to the pythonize path (which reproduces the legacy
/// behaviour, success or error, exactly).
pub(crate) struct Unsupported;

/// Walk a Python value into an arena-backed [`DataValue`] tree.
///
/// Covers exactly the built-in JSON-compatible types; everything else
/// returns [`Unsupported`] so the caller can fall back. Semantics match
/// the pythonize path byte-for-byte:
///
/// - `bool` before `int` (Python bools are ints; `True` must stay a
///   JSON boolean),
/// - `int` in `i64` → integer; in `(i64::MAX, u64::MAX]` → float (what
///   `serde_json` does with `u64` overflow); beyond → fallback (which
///   raises the legacy out-of-range `ParseError`),
/// - non-finite `float` → JSON `null` (`serde_json::Number::from_f64`
///   collapses NaN/±inf to null on the legacy path),
/// - dict keys sorted byte-wise, matching the `BTreeMap` inside
///   `serde_json::Value` (object-iteration order is observable in
///   results — the conformance suites encode it),
/// - non-`str` dict keys and lone-surrogate strings → fallback (legacy
///   `ParseError`).
pub(crate) fn py_to_datavalue<'a>(
    obj: &Bound<'_, PyAny>,
    arena: &'a Bump,
) -> Result<DataValue<'a>, Unsupported> {
    if obj.is_none() {
        return Ok(DataValue::Null);
    }
    // `bool` is final in Python — no subclass concern — but it *is* a
    // subclass of `int`, so this check must come first.
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(DataValue::Bool(b.is_true()));
    }
    // Exact types only from here down: subclasses (IntEnum, OrderedDict,
    // namedtuple, …) take the fallback, which handles them identically
    // to how the binding always did.
    if let Ok(i) = obj.cast_exact::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(DataValue::Number(NumberValue::Integer(v)));
        }
        if let Ok(v) = i.extract::<u64>() {
            // (i64::MAX, u64::MAX]: serde_json stores this as u64 and
            // the datavalue bridge converts u64-above-i64::MAX to f64.
            return Ok(DataValue::Number(NumberValue::Float(v as f64)));
        }
        return Err(Unsupported); // out of range — legacy ParseError
    }
    if let Ok(f) = obj.cast_exact::<PyFloat>() {
        let v = f.value();
        return Ok(if v.is_finite() {
            // NOT `NumberValue::from_f64` — that collapses whole floats
            // to integers; the legacy path keeps `1.0` a float.
            DataValue::Number(NumberValue::Float(v))
        } else {
            DataValue::Null
        });
    }
    if let Ok(s) = obj.cast_exact::<PyString>() {
        let Ok(text) = s.to_str() else {
            return Err(Unsupported); // lone surrogates — legacy ParseError
        };
        return Ok(DataValue::String(arena.alloc_str(text)));
    }
    if let Ok(list) = obj.cast_exact::<PyList>() {
        let mut buf = BumpVec::with_capacity_in(list.len(), arena);
        for item in list.iter() {
            buf.push(py_to_datavalue(&item, arena)?);
        }
        return Ok(DataValue::Array(buf.into_bump_slice()));
    }
    if let Ok(tuple) = obj.cast_exact::<PyTuple>() {
        let mut buf = BumpVec::with_capacity_in(tuple.len(), arena);
        for item in tuple.iter() {
            buf.push(py_to_datavalue(&item, arena)?);
        }
        return Ok(DataValue::Array(buf.into_bump_slice()));
    }
    if let Ok(dict) = obj.cast_exact::<PyDict>() {
        let mut buf: BumpVec<'_, (&str, DataValue)> = BumpVec::with_capacity_in(dict.len(), arena);
        for (k, v) in dict.iter() {
            let Ok(key) = k.cast_exact::<PyString>() else {
                return Err(Unsupported); // non-str key — legacy ParseError
            };
            let Ok(key_str) = key.to_str() else {
                return Err(Unsupported);
            };
            buf.push((
                arena.alloc_str(key_str) as &str,
                py_to_datavalue(&v, arena)?,
            ));
        }
        // Match the BTreeMap ordering of the legacy path (Python dicts
        // have unique keys, so plain byte-order sort is exact).
        if !buf.windows(2).all(|w| w[0].0 <= w[1].0) {
            buf.sort_unstable_by(|a, b| a.0.cmp(b.0));
        }
        return Ok(DataValue::Object(buf.into_bump_slice()));
    }
    Err(Unsupported)
}

/// Walk an engine result straight into Python objects.
///
/// Mirrors `pythonize(serde_json::to_value(av))` exactly: non-finite
/// floats become `None` (serde_json collapses them to null), object
/// keys come out sorted (the legacy `BTreeMap` order), and datetime /
/// duration values render as the same strings serde emitted.
pub(crate) fn datavalue_to_pyobject(py: Python<'_>, value: &DataValue<'_>) -> PyResult<Py<PyAny>> {
    Ok(match value {
        DataValue::Null => py.None(),
        DataValue::Bool(b) => PyBool::new(py, *b).to_owned().into_any().unbind(),
        DataValue::Number(NumberValue::Integer(i)) => i.into_pyobject(py)?.into_any().unbind(),
        DataValue::Number(NumberValue::Float(f)) => {
            if f.is_finite() {
                f.into_pyobject(py)?.into_any().unbind()
            } else {
                py.None()
            }
        }
        DataValue::String(s) => PyString::new(py, s).into_any().unbind(),
        DataValue::Array(items) => {
            let converted = items
                .iter()
                .map(|item| datavalue_to_pyobject(py, item))
                .collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, converted)?.into_any().unbind()
        }
        DataValue::Object(pairs) => {
            let dict = PyDict::new(py);
            if pairs.windows(2).all(|w| w[0].0 <= w[1].0) {
                for (k, v) in pairs.iter() {
                    dict.set_item(PyString::new(py, k), datavalue_to_pyobject(py, v)?)?;
                }
            } else {
                // Engine-constructed objects (templating, `preserve`, …)
                // keep rule order internally; the legacy path re-sorted
                // them through `serde_json::Value`'s BTreeMap.
                let mut sorted: Vec<&(&str, DataValue)> = pairs.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in sorted {
                    dict.set_item(PyString::new(py, k), datavalue_to_pyobject(py, v)?)?;
                }
            }
            dict.into_any().unbind()
        }
        // JSON has no datetime/duration — the legacy path serialized
        // these through serde as the same strings.
        DataValue::DateTime(d) => PyString::new(py, &d.to_iso_string()).into_any().unbind(),
        DataValue::Duration(d) => PyString::new(py, &d.to_string()).into_any().unbind(),
    })
}

pub fn dict_to_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    depythonize(value).map_err(|e| parse_error(py, format!("unsupported Python value: {e}")))
}

pub fn value_to_pyobject(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    pythonize(py, value)
        .map(|bound| bound.unbind())
        .map_err(|e| parse_error(py, format!("failed to convert result to Python: {e}")))
}
