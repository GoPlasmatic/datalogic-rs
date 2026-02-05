//! Common constants used throughout the codebase

/// Error message for invalid arguments
pub const INVALID_ARGS: &str = "Invalid Arguments";

/// Error message for NaN (Not a Number)
pub const NAN_ERROR: &str = "NaN";

/// Returns a NaN error with a JSON value `{"type": "NaN"}`.
#[inline]
pub fn nan_error() -> crate::Error {
    crate::Error::Thrown(serde_json::json!({"type": "NaN"}))
}
