//! Input adapter for [`crate::Engine::compile`] and the module-level
//! [`crate::compile`].
//!
//! [`IntoLogic`] mirrors [`crate::EvalInput`] but on the *rule* side: it
//! lets a single `compile` entry point accept any of the rule shapes a
//! caller is likely to have on hand.
//!
//! - `&str` — JSON-parsed via `OwnedDataValue::from_json`.
//! - `&OwnedDataValue` — compiled in place by the engine's compile
//!   methods. [`IntoLogic::into_owned_logic`] deep-clones it:
//!   `OwnedDataValue` holds no `Arc`, so a clone copies the whole rule.
//! - `OwnedDataValue` — moved.
//! - `&serde_json::Value` (`serde_json`) — deep-converted.
//!
//! There is no `&T: Serialize` shape (see the note at the bottom of this
//! file): convert with `serde_json::to_value(&t)?` first and pass the
//! resulting `&serde_json::Value`.
//!
//! The trait is **sealed**. The supported set is closed.

use std::borrow::Cow;

use datavalue::OwnedDataValue;

use crate::Result;

pub(crate) mod sealed {
    use std::borrow::Cow;

    use datavalue::OwnedDataValue;

    /// The sealing supertrait. It also carries the compile methods' way in:
    /// the rule source as a [`Cow`], so a borrowed `&OwnedDataValue` is
    /// compiled in place rather than deep-cloned first. The module is
    /// private, so this adds nothing a caller outside the crate can name.
    pub trait Sealed {
        fn logic_source<'s>(self) -> crate::Result<Cow<'s, OwnedDataValue>>
        where
            Self: 's;
    }
}

/// Convert `self` into an [`OwnedDataValue`] suitable for compilation.
///
/// Sealed trait — the supported input shapes are listed in this file;
/// external crates cannot add new ones.
pub trait IntoLogic: sealed::Sealed {
    /// Materialise the rule source as an owned value.
    ///
    /// Implementations either parse (`&str`), deep-clone
    /// (`&OwnedDataValue`), move (`OwnedDataValue`), or deep-convert from
    /// a serde shape. The engine's own compile methods do not go through
    /// this for a borrowed `&OwnedDataValue`; they compile it in place.
    fn into_owned_logic(self) -> Result<OwnedDataValue>;
}

impl sealed::Sealed for &str {
    #[inline]
    fn logic_source<'s>(self) -> Result<Cow<'s, OwnedDataValue>>
    where
        Self: 's,
    {
        self.into_owned_logic().map(Cow::Owned)
    }
}
impl IntoLogic for &str {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self)?)
    }
}

impl sealed::Sealed for &String {
    #[inline]
    fn logic_source<'s>(self) -> Result<Cow<'s, OwnedDataValue>>
    where
        Self: 's,
    {
        self.into_owned_logic().map(Cow::Owned)
    }
}
impl IntoLogic for &String {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(OwnedDataValue::from_json(self.as_str())?)
    }
}

impl sealed::Sealed for &OwnedDataValue {
    #[inline]
    fn logic_source<'s>(self) -> Result<Cow<'s, OwnedDataValue>>
    where
        Self: 's,
    {
        Ok(Cow::Borrowed(self))
    }
}
impl IntoLogic for &OwnedDataValue {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(self.clone())
    }
}

impl sealed::Sealed for OwnedDataValue {
    #[inline]
    fn logic_source<'s>(self) -> Result<Cow<'s, OwnedDataValue>>
    where
        Self: 's,
    {
        Ok(Cow::Owned(self))
    }
}
impl IntoLogic for OwnedDataValue {
    #[inline]
    fn into_owned_logic(self) -> Result<OwnedDataValue> {
        Ok(self)
    }
}

#[cfg(feature = "serde_json")]
impl sealed::Sealed for &serde_json::Value {
    #[inline]
    fn logic_source<'s>(self) -> Result<Cow<'s, OwnedDataValue>>
    where
        Self: 's,
    {
        self.into_owned_logic().map(Cow::Owned)
    }
}
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
