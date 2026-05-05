//! Input adapter for [`crate::Engine::evaluate`] and
//! [`crate::Scratch::eval`].
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

/// Adapter trait that converts a value into a `&'a DataValue<'a>` borrowed
/// from the caller-supplied arena. Internal trait — you don't need to
/// implement it yourself.
pub trait IntoEvalData<'a> {
    /// Materialise `self` as a `&'a DataValue<'a>` in `arena`.
    ///
    /// Implementations either pass through an existing arena reference (zero
    /// cost), allocate one node, or deep-convert from an owned tree.
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>>;
}

impl<'a> IntoEvalData<'a> for &'a DataValue<'a> {
    #[inline]
    fn into_eval_data(self, _arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self)
    }
}

impl<'a> IntoEvalData<'a> for DataValue<'a> {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self))
    }
}

impl<'a> IntoEvalData<'a> for &'a str {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = DataValue::from_str(self, arena)?;
        Ok(arena.alloc(av))
    }
}

impl<'a> IntoEvalData<'a> for &'a OwnedDataValue {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self.to_arena(arena)))
    }
}

#[cfg(feature = "compat")]
impl<'a> IntoEvalData<'a> for &'a serde_json::Value {
    #[inline]
    fn into_eval_data(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = crate::arena::value_to_data(self, arena);
        Ok(arena.alloc(av))
    }
}
