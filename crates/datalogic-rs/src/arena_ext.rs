//! Convenience helpers for allocating `DataValue` results in a [`Bump`].
//!
//! Custom operators end every successful path with a single, conceptual
//! "return this value" step that, written longhand, takes two stages:
//!
//! ```ignore
//! Ok(arena.alloc(DataValue::from_f64(result)))
//! ```
//!
//! [`ArenaExt`] folds those two stages into one method call so the call
//! site reads as the operation it actually is. The helpers are zero-cost
//! over the manual form (they reduce to the same arena writes) and where
//! a static singleton already exists — `null`, `bool(_)`, small
//! non-negative integers, empty string / array / object — they return
//! the singleton directly and skip the arena entirely.
//!
//! Bring the trait into scope with `use datalogic_rs::ArenaExt;` from
//! inside a [`crate::CustomOperator`] impl.

use bumpalo::Bump;

use crate::DataValue;
use crate::arena::singletons;

/// Allocate borrowed [`DataValue`]s in a [`Bump`] without typing
/// `arena.alloc(DataValue::...)` at every return.
///
/// Each method returns `&'a DataValue<'a>` — directly returnable from
/// [`crate::CustomOperator::evaluate`].
///
/// # Example
///
/// ```rust
/// use bumpalo::Bump;
/// use datalogic_rs::operator::EvalContext;
/// use datalogic_rs::{ArenaExt, CustomOperator, DataValue, Result};
///
/// struct Triple;
/// impl CustomOperator for Triple {
///     fn evaluate<'a>(
///         &self,
///         args: &[&'a DataValue<'a>],
///         _ctx: &mut EvalContext<'_, 'a>,
///         arena: &'a Bump,
///     ) -> Result<&'a DataValue<'a>> {
///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
///         Ok(arena.f64(n * 3.0))
///     }
/// }
/// ```
pub trait ArenaExt<'a> {
    /// `DataValue::Null` — returns a static singleton, no allocation.
    fn null(&'a self) -> &'a DataValue<'a>;

    /// `DataValue::Bool(b)` — returns a static singleton, no allocation.
    fn bool(&'a self, b: bool) -> &'a DataValue<'a>;

    /// `DataValue::from_i64(n)` — returns a static singleton when
    /// `0 <= n <= 32`, otherwise allocates in the arena.
    fn i64(&'a self, n: i64) -> &'a DataValue<'a>;

    /// `DataValue::from_f64(n)` — always allocates in the arena.
    fn f64(&'a self, n: f64) -> &'a DataValue<'a>;

    /// `DataValue::String(...)` — copies `s` into the arena and wraps it.
    /// Empty input returns a static singleton.
    fn string(&'a self, s: &str) -> &'a DataValue<'a>;

    /// `DataValue::Array(...)` — copies `items` into the arena. Empty
    /// input returns a static singleton.
    fn array(&'a self, items: &[DataValue<'a>]) -> &'a DataValue<'a>;

    /// `DataValue::Object(...)` — copies `pairs` into the arena. Empty
    /// input returns a static singleton.
    fn object(&'a self, pairs: &[(&'a str, DataValue<'a>)]) -> &'a DataValue<'a>;
}

impl<'a> ArenaExt<'a> for Bump {
    #[inline]
    fn null(&'a self) -> &'a DataValue<'a> {
        singletons::singleton_null()
    }

    #[inline]
    fn bool(&'a self, b: bool) -> &'a DataValue<'a> {
        singletons::singleton_bool(b)
    }

    #[inline]
    fn i64(&'a self, n: i64) -> &'a DataValue<'a> {
        if let Some(s) = singletons::singleton_small_int(n) {
            return s;
        }
        self.alloc(DataValue::from_i64(n))
    }

    #[inline]
    fn f64(&'a self, n: f64) -> &'a DataValue<'a> {
        self.alloc(DataValue::from_f64(n))
    }

    #[inline]
    fn string(&'a self, s: &str) -> &'a DataValue<'a> {
        if s.is_empty() {
            return singletons::singleton_empty_string();
        }
        let s = self.alloc_str(s);
        self.alloc(DataValue::String(s))
    }

    #[inline]
    fn array(&'a self, items: &[DataValue<'a>]) -> &'a DataValue<'a> {
        if items.is_empty() {
            return singletons::singleton_empty_array();
        }
        let slice = self.alloc_slice_copy(items);
        self.alloc(DataValue::Array(slice))
    }

    #[inline]
    fn object(&'a self, pairs: &[(&'a str, DataValue<'a>)]) -> &'a DataValue<'a> {
        if pairs.is_empty() {
            return singletons::singleton_empty_object();
        }
        let slice = self.alloc_slice_copy(pairs);
        self.alloc(DataValue::Object(slice))
    }
}
