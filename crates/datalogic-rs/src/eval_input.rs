//! Input adapter for [`crate::Engine::evaluate`] (the raw-arena tier) and
//! [`crate::Session::eval_borrowed`].
//!
//! [`EvalInput`] lets the borrowed-result entry points accept any of the
//! input shapes a caller is likely to have on hand:
//!
//! - `&'a DataValue<'a>` — already arena-resident; passed through unchanged.
//! - `DataValue<'a>` — single bumpalo allocation into the arena.
//! - `&OwnedDataValue` — deep-borrowed into the arena.
//! - `&ParsedData` — parse-once handle; passed through unchanged (zero cost).
//! - `&str` — JSON-parsed via [`datavalue::DataValue::from_str`].
//! - `&serde_json::Value` (`serde_json`) — deep-converted into the arena.
//!
//! `EvalInput` carries the arena lifetime in its trait parameter, so it
//! is the right adapter when the **caller** supplies the arena. For the
//! one-shot owned-result methods on [`crate::Engine`] and the module-
//! level `eval*` helpers (where the arena lives **inside** the call),
//! the engine instead uses [`OwnedInput`], which doesn't carry an arena
//! lifetime — see that trait for the supported shapes.
//!
//! Conversion is fallible because the `&str` impl can return a parse
//! error; the borrow / owned-clone impls always succeed and return
//! [`Ok`] without touching the arena beyond the documented per-impl
//! cost.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::Result;
use crate::arena::DataValue;

/// Sealed-trait scaffolding — the [`Sealed`] super-bound lives in this
/// private module so external crates cannot implement [`EvalInput`].
/// The set of supported input shapes is a closed class defined entirely
/// in this file.
mod sealed {
    pub trait Sealed {}
}

/// Adapter trait that converts a value into a `&'a DataValue<'a>` borrowed
/// from the caller-supplied arena. **Sealed** — the supported input
/// shapes are listed in this file; external crates cannot add new ones.
pub trait EvalInput<'a>: sealed::Sealed {
    /// Materialise `self` as a `&'a DataValue<'a>` in `arena`.
    ///
    /// Implementations either pass through an existing arena reference (zero
    /// cost), allocate one node, or deep-convert from an owned tree.
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>>;
}

impl<'a> sealed::Sealed for &'a DataValue<'a> {}
impl<'a> EvalInput<'a> for &'a DataValue<'a> {
    #[inline]
    fn into_arena_value(self, _arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self)
    }
}

impl<'a> sealed::Sealed for DataValue<'a> {}
impl<'a> EvalInput<'a> for DataValue<'a> {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self))
    }
}

impl sealed::Sealed for &str {}
impl<'a> EvalInput<'a> for &'a str {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = DataValue::from_str(self, arena)?;
        Ok(arena.alloc(av))
    }
}

// `&String` derefs to `&str`, but trait resolution doesn't autoderef
// across trait impls — accepting `&String` directly here saves callers
// from writing `payload.as_str()` at every call site.
impl sealed::Sealed for &String {}
impl<'a> EvalInput<'a> for &'a String {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        <&'a str as EvalInput<'a>>::into_arena_value(self.as_str(), arena)
    }
}

impl sealed::Sealed for &OwnedDataValue {}
impl<'a> EvalInput<'a> for &'a OwnedDataValue {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(self.to_arena(arena)))
    }
}

impl sealed::Sealed for &crate::ParsedData {}
impl<'a> EvalInput<'a> for &'a crate::ParsedData {
    #[inline]
    fn into_arena_value(self, _arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self.value())
    }
}

#[cfg(feature = "serde_json")]
impl sealed::Sealed for &serde_json::Value {}
#[cfg(feature = "serde_json")]
impl<'a> EvalInput<'a> for &'a serde_json::Value {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        let av = crate::arena::value_to_data(self, arena);
        Ok(arena.alloc(av))
    }
}

// ============================================================
// OwnedInput — arena-lifetime-free counterpart for one-shot calls
// ============================================================

/// Adapter trait for [`crate::Engine::eval`] / [`crate::Engine::eval_str`]
// `Engine::eval_into` is gated behind `serde_json`. Link it when the
// feature is on; otherwise reference it as code text to keep the docs
// resolvable in a default-features build.
#[cfg_attr(
    feature = "serde_json",
    doc = "/ [`crate::Engine::eval_into`] and the module-level `datalogic::eval*`"
)]
#[cfg_attr(
    not(feature = "serde_json"),
    doc = "(plus `Engine::eval_into` with the `serde_json` feature) and the module-level `datalogic::eval*`"
)]
/// helpers, where the engine creates and owns the arena per call.
///
/// Unlike [`EvalInput`] (which carries an arena lifetime), `OwnedInput`
/// produces an [`OwnedDataValue`] without borrowing into a caller arena.
/// The engine then deep-borrows that owned value into its per-call
/// bump. Sealed; the supported set is closed:
///
/// - `&str` — JSON-parsed.
/// - `&String` — JSON-parsed.
/// - `&OwnedDataValue` — cloned.
/// - `OwnedDataValue` — moved.
/// - `&serde_json::Value` (`serde_json`) — deep-converted.
///
/// For the borrowed-result paths, use [`EvalInput`] instead.
pub trait OwnedInput: sealed::Sealed {
    /// Materialise `self` as an owned data value.
    fn into_owned_input(self) -> Result<OwnedDataValue>;
}

impl OwnedInput for &str {
    #[inline]
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self)?)
    }
}

impl OwnedInput for &String {
    #[inline]
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self.as_str())?)
    }
}

impl OwnedInput for &OwnedDataValue {
    #[inline]
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(self.clone())
    }
}

impl sealed::Sealed for OwnedDataValue {}
impl OwnedInput for OwnedDataValue {
    #[inline]
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(self)
    }
}

#[cfg(feature = "serde_json")]
impl OwnedInput for &serde_json::Value {
    #[inline]
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(crate::serde_bridge::owned_from_serde(self))
    }
}
