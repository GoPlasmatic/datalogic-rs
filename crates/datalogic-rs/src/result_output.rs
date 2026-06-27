//! Output adapter for [`crate::Engine::eval`] / [`crate::Session::eval`]
//! and the typed `eval_into::<T>` family.
//!
//! [`FromDataValue`] is the result-side counterpart of [`crate::EvalInput`]
//! / [`crate::IntoLogic`]: it turns a borrowed arena `DataValue` into the
//! caller's chosen output shape. Suffix variants on the public API
//! (`_str`, `_into::<T>`) are thin sugar around the matching impl.
//!
//! - [`datavalue::OwnedDataValue`] — deep-clone out of the arena.
//! - [`String`] — JSON via `Display` (matches `eval_str`).
//! - `serde_json::Value` (`serde_json`) — arena → serde walk.
//! - `T: DeserializeOwned` (`serde_json`) — arena → JSON → `T`.
//!
//! The trait is **sealed**.

use datavalue::OwnedDataValue;

use crate::Result;
use crate::arena::DataValue;

mod sealed {
    pub trait Sealed {}
}

/// Convert a borrowed arena [`DataValue`] into a concrete result type.
///
/// Sealed trait — see the module docs for the supported output shapes.
pub trait FromDataValue: sealed::Sealed + Sized {
    /// Materialise `value` as `Self`. Most impls deep-clone; the
    /// `String` impl serialises to JSON; the typed impls route through
    /// `serde_json` when the feature is enabled.
    fn from_arena(value: &DataValue<'_>) -> Result<Self>;
}

impl sealed::Sealed for OwnedDataValue {}
impl FromDataValue for OwnedDataValue {
    #[inline]
    fn from_arena(value: &DataValue<'_>) -> Result<Self> {
        Ok(value.to_owned())
    }
}

impl sealed::Sealed for String {}
impl FromDataValue for String {
    #[inline]
    fn from_arena(value: &DataValue<'_>) -> Result<Self> {
        Ok(value.to_string())
    }
}

#[cfg(feature = "serde_json")]
impl sealed::Sealed for serde_json::Value {}
#[cfg(feature = "serde_json")]
impl FromDataValue for serde_json::Value {
    #[inline]
    fn from_arena(value: &DataValue<'_>) -> Result<Self> {
        Ok(crate::arena::data_to_value(value))
    }
}

// Note on `T: DeserializeOwned`: a blanket impl would overlap with the
// per-type impls above (every `OwnedDataValue` / `String` /
// `serde_json::Value` is also `DeserializeOwned`). The typed path is
// therefore exposed as an inherent method
// `Engine::eval_into::<T>(...)` / `Session::eval_into::<T>(...)` /
// `datalogic::eval_into::<T>(...)`, which projects the result to a
// `serde_json::Value` (via `crate::arena::data_to_value`) and then calls
// `serde_json::from_value`. See `top_level::eval_into` and the
// `Engine::eval_into` definition.
