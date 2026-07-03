//! Parse-once data handles — the ABI v2 mirror of `bindings/c/src/data.rs`.
//!
//! Parsing the data JSON dominates a parse-eval-serialize round trip
//! (70-90%, see `tools/benchmark/BINDINGS-OVERHEAD.md`), and the
//! string-shaped entry points re-pay it on every call. [`DataHandle`]
//! pays it once; the resident tree then feeds any rule/session through
//! the core's zero-cost `&DataValue` passthrough.
//!
//! The same "resident tree" plumbing also backs the dict-input fast
//! path: [`build_py_tree`] walks Python objects straight into a
//! [`PyBuiltData`] arena, skipping the `serde_json::Value` intermediate
//! entirely (see `conv::py_to_datavalue`).

use datalogic_rs::ParsedData;
use datalogic_rs::bumpalo::Bump;
use datalogic_rs::datavalue::DataValue;
use pyo3::prelude::*;
use self_cell::self_cell;

use crate::conv::{Unsupported, py_to_datavalue};
use crate::error::engine_error_to_pyerr;

self_cell!(
    /// Owns a bump arena and the `DataValue` tree walked into it from
    /// Python objects — the dict-input twin of the core's `ParsedData`
    /// (which only builds from JSON text).
    pub(crate) struct PyBuiltData {
        owner: Bump,
        #[covariant]
        dependent: DataValue,
    }
);

/// The two ways a resident parsed tree comes to exist in this binding.
pub(crate) enum ParsedTree {
    /// Parsed from JSON text by the core (`DataHandle`).
    Json(ParsedData),
    /// Built by walking Python objects into a local arena (dict path).
    PyBuilt(PyBuiltData),
}

/// A resident, immutable parsed tree, shareable across threads for
/// reads.
///
/// SAFETY (adapted from `bindings/c/src/data.rs`): `ParsedData` and
/// `PyBuiltData` are `Send` but not `Sync` only because their backing
/// `bumpalo::Bump` is `!Sync` (allocation takes `&self`). This wrapper
/// never allocates after construction: the tree is built once (in
/// `ParsedData::from_json` or `build_py_tree`) and every subsequent
/// access is a `&`-read of a `DataValue` tree with no interior
/// mutability (verified against datavalue 0.2.2 — re-audit on datavalue
/// upgrades). Concurrent reads of immutable memory are sound, so
/// exposing the tree as thread-safe is correct as long as this crate
/// never mutates through it (it doesn't — there is no mutating entry
/// point for `SharedTree`).
pub(crate) struct SharedTree(ParsedTree);

unsafe impl Sync for SharedTree {}
// `Send` is automatic: `ParsedData` is documented `Send` and
// `PyBuiltData` is a self_cell of `Send` owner + `Send` dependent.

impl SharedTree {
    /// Borrow the parsed tree. Valid for as long as the wrapper lives;
    /// satisfies the `&DataValue` input shape of `Engine::evaluate`
    /// directly (zero per-call conversion).
    pub(crate) fn value(&self) -> &DataValue<'_> {
        match &self.0 {
            ParsedTree::Json(p) => p.value(),
            ParsedTree::PyBuilt(c) => c.borrow_dependent(),
        }
    }

    pub(crate) fn allocated_bytes(&self) -> usize {
        match &self.0 {
            ParsedTree::Json(p) => p.allocated_bytes(),
            ParsedTree::PyBuilt(c) => c.borrow_owner().allocated_bytes(),
        }
    }
}

/// Walk a Python object into a self-contained [`SharedTree`].
///
/// Returns `Err(Unsupported)` when the input contains a shape the
/// direct walk doesn't cover (subclassed containers, sets, mappings,
/// out-of-range ints, …) — the caller then falls back to the pythonize
/// path, which either handles it or raises exactly the error it always
/// raised.
pub(crate) fn build_py_tree(obj: &Bound<'_, PyAny>) -> Result<SharedTree, Unsupported> {
    let cell = PyBuiltData::try_new(Bump::new(), |arena| py_to_datavalue(obj, arena))?;
    Ok(SharedTree(ParsedTree::PyBuilt(cell)))
}

/// An immutable, pre-parsed JSON document — parse once, evaluate many.
///
/// Parsing data once and evaluating it many times skips the per-call
/// JSON parse that dominates string-based evaluation on larger
/// payloads. A `DataHandle` is independent of any `Engine` — one handle
/// can feed rules compiled by different engines, any number of times
/// (evaluation never consumes it) — and, unlike `Session`, it may be
/// shared across threads for reads: the tree is immutable after
/// construction.
///
///     data = DataHandle('{"user": {"age": 42}}')
///     rule.evaluate_data(data)
///     session.evaluate_bool(rule, data)
#[pyclass(name = "DataHandle", module = "datalogic_py", frozen)]
pub struct DataHandle {
    pub(crate) tree: SharedTree,
}

#[pymethods]
impl DataHandle {
    /// Parse ``json`` (a JSON ``str``) into a resident handle.
    ///
    /// :raises ParseError: on malformed JSON.
    #[new]
    fn new(py: Python<'_>, json: &str) -> PyResult<Self> {
        let owned = json.to_string();
        let parsed = py
            .detach(move || ParsedData::from_json(&owned))
            .map_err(|e| engine_error_to_pyerr(py, &e, None))?;
        Ok(Self {
            tree: SharedTree(ParsedTree::Json(parsed)),
        })
    }

    /// Bytes held by the handle's backing arena (input copy + parsed
    /// tree). Useful for sizing and diagnostics.
    #[getter]
    fn allocated_bytes(&self) -> usize {
        self.tree.allocated_bytes()
    }

    fn __repr__(&self) -> String {
        format!(
            "DataHandle(allocated_bytes={})",
            self.tree.allocated_bytes()
        )
    }
}
