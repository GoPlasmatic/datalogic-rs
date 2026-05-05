//! Input adapter for [`crate::Engine::evaluate`] and
//! [`crate::Scratch::evaluate`].
//!
//! [`IntoEvalData`] lets the evaluation entry points accept any of the input
//! shapes a caller is likely to have on hand:
//!
//! - `&'a DataValue<'a>` — already arena-resident; passed through unchanged.
//! - `DataValue<'a>` — single bumpalo allocation into the arena.
//! - `&OwnedDataValue` — deep-borrowed into the arena.
//! - `&str` — JSON-parsed via [`datavalue::DataValue::from_str`].
//! - `&serde_json::Value` (`compat`) — deep-converted into the arena.
//!
//! Conversion is fallible because the `&str` impl can return a parse error;
//! the borrow / owned-clone impls always succeed and return [`Ok`] without
//! touching the arena beyond the documented per-impl cost.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::Result;
use crate::arena::DataValue;

/// Sealed-trait scaffolding — the [`Sealed`] super-bound lives in this
/// private module so external crates cannot implement [`IntoEvalData`].
/// The set of supported input shapes is a closed class defined entirely
/// in this file.
mod sealed {
    pub trait Sealed {}
}

/// Adapter trait that converts a value into a `&'a DataValue<'a>` borrowed
/// from the caller-supplied arena. **Sealed** — the supported input
/// shapes are listed in this file; external crates cannot add new ones.
pub trait IntoEvalData<'a>: sealed::Sealed {
    /// Materialise `self` as a `&'a DataValue<'a>` in `arena`.
    ///
    /// Implementations either pass through an existing arena reference (zero
    /// cost), allocate one node, or deep-convert from an owned tree.
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>>;
}

impl<'a> sealed::Sealed for &'a DataValue<'a> {}
impl<'a> IntoEvalData<'a> for &'a DataValue<'a> {
    #[inline]
    fn into_eval_data(self, _arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self)
    }
}

impl<'a> sealed::Sealed for DataValue<'a> {}
impl<'a> IntoEvalData<'a> for DataValue<'a> {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self))
    }
}

impl sealed::Sealed for &str {}
impl<'a> IntoEvalData<'a> for &'a str {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = DataValue::from_str(self, arena)?;
        Ok(arena.alloc(av))
    }
}

impl sealed::Sealed for &OwnedDataValue {}
impl<'a> IntoEvalData<'a> for &'a OwnedDataValue {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self.to_arena(arena)))
    }
}

#[cfg(feature = "compat")]
impl sealed::Sealed for &serde_json::Value {}
#[cfg(feature = "compat")]
impl<'a> IntoEvalData<'a> for &'a serde_json::Value {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = crate::arena::value_to_data(self, arena);
        Ok(arena.alloc(av))
    }
}
