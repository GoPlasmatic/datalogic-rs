//! Input adapter for [`crate::Engine::compile`] and the module-level
//! [`crate::compile`].
//!
//! [`IntoLogic`] mirrors [`crate::EvalInput`] but on the *rule* side: it
//! lets a single `compile` entry point accept any of the rule shapes a
//! caller is likely to have on hand.
//!
//! - `&str` — JSON-parsed via `OwnedDataValue::from_json`.
//! - `&OwnedDataValue` — cloned (cheap; usually just an `Arc` bump in
//!   practice).
//! - `OwnedDataValue` — moved.
//! - `&serde_json::Value` (`serde_json`) — deep-converted.
//! - `&T: Serialize` (`serde_json`) — routed via `serde_json::to_value`.
//!
//! The trait is **sealed**. The supported set is closed.

use datavalue::OwnedDataValue;

use crate::Result;

mod sealed {
    pub trait Sealed {}
}

/// Convert `self` into an [`OwnedDataValue`] suitable for compilation.
///
/// Sealed trait — the supported input shapes are listed in this file;
/// external crates cannot add new ones.
pub trait IntoLogic: sealed::Sealed {
    /// Materialise the rule source as an owned value.
    ///
    /// Implementations either parse (`&str`), clone (`&OwnedDataValue`),
    /// move (`OwnedDataValue`), or deep-convert from a serde shape.
    fn into_owned_logic(self) -> Result<OwnedDataValue>;
}

impl sealed::Sealed for &str {}
impl IntoLogic for &str {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self)?)
    }
}

impl sealed::Sealed for &String {}
impl IntoLogic for &String {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self.as_str())?)
    }
}

impl sealed::Sealed for &OwnedDataValue {}
impl IntoLogic for &OwnedDataValue {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(self.clone())
    }
}

impl sealed::Sealed for OwnedDataValue {}
impl IntoLogic for OwnedDataValue {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(self)
    }
}

#[cfg(feature = "serde_json")]
impl sealed::Sealed for &serde_json::Value {}
#[cfg(feature = "serde_json")]
impl IntoLogic for &serde_json::Value {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(crate::serde_bridge::owned_from_serde(self))
    }
}

// Note: a blanket `impl<T: Serialize> IntoLogic for &T` would conflict
// with the per-type impls above (every &T is also an &T: Serialize when
// `serde` is in scope). So there is no blanket impl and no typed-Serialize
// constructor: a caller holding a `&T: Serialize` converts it first with
// `serde_json::to_value(&t)?` (yielding a `serde_json::Value`, which does
// impl `IntoLogic`) and passes that to `Engine::compile`.
