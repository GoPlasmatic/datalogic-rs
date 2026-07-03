//! Structured error surface exposed to JS.
//!
//! Unlike pyo3 (which has class-based exception inheritance), napi-rs
//! throws plain JS `Error` instances. We mark the kind via `.name` —
//! `"ParseError"` or `"EvaluateError"` — and attach the same structured
//! fields the Python binding exposes:
//!
//! ```text
//! Error
//!   .name        "ParseError" | "EvaluateError"
//!   .message     human-readable message from datalogic_rs::Error
//!   .errorType   stable tag from datalogic_rs::Error::tag()
//!   .operator    outermost failing operator (or null)
//!   .nodeIds     leaf-to-root breadcrumb of compiled-node ids
//!   .path        resolved root-to-leaf array of { nodeId, operator,
//!                argIndex, jsonPointer } objects (only when the binding
//!                has the compiled Logic at hand; null otherwise)
//! ```
//!
//! Consumer pattern:
//!
//! ```js
//! try { rule.evaluate(data) }
//! catch (e) {
//!   if (e.name === 'ParseError') { ... }
//!   else { console.log(e.errorType, e.operator, e.path) }
//! }
//! ```
//!
//! Why throw via `env.throw` rather than returning `napi::Error`: napi's
//! plain `Error` type carries only `status` + `reason`, no extra fields.
//! Building the JS Error object directly lets us attach `errorType`,
//! `operator`, `nodeIds`, and `path` before throwing, then signal the
//! pending exception back to the napi runtime via `Status::PendingException`.

use datalogic_rs::{Error as RsError, Logic};
use napi::bindgen_prelude::*;
use napi::{Env, JsValue};
use serde_json::{Value, json};

/// Convert a `datalogic_rs::Error` into a thrown JS Error and return a
/// `napi::Error` with `Status::PendingException` so the napi runtime
/// knows JS already has a throw in flight.
pub fn engine_error(env: &Env, err: &RsError, compiled: Option<&Logic>) -> napi::Error {
    let message = err.to_string();
    let attrs = engine_attrs(err, &message, compiled);
    throw_attrs(env, &attrs).unwrap_or_else(|| napi::Error::from_reason(message))
}

/// Build (without throwing) the same decorated JS Error object
/// [`engine_error`] would throw, wrapped as a `napi::Error` that carries
/// the JS object by reference. Used by the async path: `Task::reject`
/// cannot throw into the env, but rejecting a promise with a
/// `napi::Error` constructed from a real JS Error preserves every
/// structured field (`name`, `errorType`, `operator`, `nodeIds`,
/// `path`). Returns `None` when object construction fails; callers fall
/// back to a plain reason string.
pub fn engine_error_value(env: &Env, err: &RsError, compiled: Option<&Logic>) -> Option<napi::Error> {
    let message = err.to_string();
    let attrs = engine_attrs(err, &message, compiled);
    let obj = build_attrs_object(env, &attrs)?;
    Some(napi::Error::from(obj.to_unknown()))
}

/// Throw an `EvaluateError` with `errorType: "TypeMismatch"` — the
/// typed-eval outcome where the rule evaluated fine but the result is
/// not of the requested type. Mirrors the C ABI's
/// `DATALOGIC_STATUS_TYPE_MISMATCH` (no operator, no path — the failure
/// is at the result boundary, not inside the rule).
pub fn type_mismatch_error(env: &Env, message: &str) -> napi::Error {
    let attrs = ErrorAttrs {
        name: "EvaluateError",
        message,
        error_type: "TypeMismatch",
        operator: None,
        node_ids: &[],
        path: None,
    };
    throw_attrs(env, &attrs).unwrap_or_else(|| napi::Error::from_reason(message.to_string()))
}

fn engine_attrs<'a>(err: &'a RsError, message: &'a str, compiled: Option<&Logic>) -> ErrorAttrs<'a> {
    let tag = err.tag();
    let name = if tag == "ParseError" {
        "ParseError"
    } else {
        "EvaluateError"
    };
    ErrorAttrs {
        name,
        message,
        error_type: tag,
        operator: err.operator(),
        node_ids: err.node_ids(),
        path: compiled.map(|c| resolve_path(err, c)),
    }
}

/// Parse-stage shorthand for the path where there's no `RsError` value
/// (e.g. the binding rejected malformed JSON before handing anything to
/// the engine). Raises a `ParseError` with `.errorType = "ParseError"`
/// and the other structured fields set to neutral defaults.
pub fn parse_error(env: &Env, message: &str) -> napi::Error {
    let attrs = ErrorAttrs {
        name: "ParseError",
        message,
        error_type: "ParseError",
        operator: None,
        node_ids: &[],
        path: Some(Vec::new()),
    };
    throw_attrs(env, &attrs).unwrap_or_else(|| napi::Error::from_reason(message.to_string()))
}

struct ErrorAttrs<'a> {
    name: &'a str,
    message: &'a str,
    error_type: &'a str,
    operator: Option<&'a str>,
    node_ids: &'a [u32],
    /// `None` → not resolvable (binding had no `&Logic`); surfaces as JS `null`.
    /// `Some(vec)` → resolved (possibly empty); surfaces as a JS array.
    path: Option<Vec<Value>>,
}

/// Create the decorated JS Error object (a real `Error` instance with
/// the structured fields attached) without throwing it.
fn build_attrs_object<'env>(env: &'env Env, attrs: &ErrorAttrs<'_>) -> Option<Object<'env>> {
    // `env.create_error` wants a `napi::Error`; the `Object` it returns
    // is a real JS Error instance we can decorate before throwing.
    let mut obj = env
        .create_error(napi::Error::from_reason(attrs.message.to_string()))
        .ok()?;
    obj.set_named_property("name", attrs.name).ok()?;
    obj.set_named_property("errorType", attrs.error_type).ok()?;
    match attrs.operator {
        Some(op) => obj.set_named_property("operator", op).ok()?,
        None => obj.set_named_property("operator", Null).ok()?,
    }
    obj.set_named_property("nodeIds", attrs.node_ids.to_vec())
        .ok()?;
    match &attrs.path {
        Some(steps) => {
            let array_value = Value::Array(steps.clone());
            obj.set_named_property("path", array_value).ok()?;
        }
        None => obj.set_named_property("path", Null).ok()?,
    }
    Some(obj)
}

fn throw_attrs(env: &Env, attrs: &ErrorAttrs<'_>) -> Option<napi::Error> {
    let obj = build_attrs_object(env, attrs)?;
    env.throw(obj).ok()?;
    // PendingException tells napi-rs "JS already has a throw queued; do
    // not throw the napi::Error I'm returning". The reason string is
    // never seen by JS in this path.
    Some(napi::Error::new(
        napi::Status::PendingException,
        String::new(),
    ))
}

fn resolve_path(err: &RsError, compiled: &Logic) -> Vec<Value> {
    err.resolve_path(compiled)
        .into_iter()
        .map(|s| {
            json!({
                "nodeId": s.node_id,
                "operator": s.operator,
                "argIndex": s.arg_index,
                "jsonPointer": s.json_pointer,
            })
        })
        .collect()
}
