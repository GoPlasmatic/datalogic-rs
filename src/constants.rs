//! Common constants used throughout the codebase

/// Error message for invalid arguments
pub const INVALID_ARGS: &str = "Invalid Arguments";

/// Error message for NaN (Not a Number)
pub const NAN_ERROR: &str = "NaN";

/// Returns a NaN error with a JSON value `{"type": "NaN"}`.
#[inline]
pub fn nan_error() -> crate::Error {
    use datavalue::OwnedDataValue;
    crate::Error::Thrown(OwnedDataValue::Object(vec![(
        "type".to_string(),
        OwnedDataValue::String("NaN".to_string()),
    )]))
}

/// Returns the canonical "Invalid Arguments" error.
#[inline]
pub fn invalid_args() -> crate::Error {
    crate::Error::InvalidArguments(INVALID_ARGS.into())
}
