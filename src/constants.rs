//! Common constants used throughout the codebase

use std::sync::LazyLock;

/// Error message for invalid arguments
pub const INVALID_ARGS: &str = "Invalid Arguments";

/// Error message for NaN (Not a Number)
pub const NAN_ERROR: &str = "NaN";

/// Pre-built NaN error JSON value, shared across all error sites.
/// Cloning this is cheaper than constructing via `json!({"type": "NaN"})` each time.
static NAN_ERROR_VALUE: LazyLock<serde_json::Value> =
    LazyLock::new(|| serde_json::json!({"type": "NaN"}));

/// Returns a NaN error with a pre-built JSON value.
#[inline]
pub fn nan_error() -> crate::Error {
    crate::Error::Thrown(NAN_ERROR_VALUE.clone())
}
