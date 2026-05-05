//! Re-export of the arena-allocated value type plus the helpers built
//! around it.
//!
//! `DataValue` is just `datavalue::DataValue` — the two crates share a
//! single bump-allocated value type. Helpers (truthiness, coercion,
//! traversal, lookup, conversion at the serde_json boundary) stay in
//! datalogic-rs because they're tied to engine config / op semantics.

mod coercion;
mod conversion;
mod lookup;
mod strings;
mod traversal;

#[cfg(feature = "datetime")]
pub(crate) use coercion::coerce_to_number;
pub(crate) use coercion::{coerce_to_number_cfg, try_coerce_to_integer_cfg};
#[cfg(feature = "compat")]
pub use conversion::data_to_value;
#[cfg(feature = "compat")]
pub use conversion::value_to_data;
pub(crate) use lookup::object_lookup_field;
pub use strings::data_to_json_string;
pub(crate) use strings::{data_to_str, truthy_arena};
pub(crate) use traversal::apply_path_element;
pub(crate) use traversal::{
    access_path_str_ref, path_exists_segments, path_exists_str, traverse_segments,
};

pub use datavalue::DataValue;

/// Reborrow an arena-resident `DataValue` reference up to the arena's `'a`
/// lifetime. Iterators over `&'a [DataValue<'a>]` (slice / pair) yield
/// references with the iterator's *shorter* borrow lifetime; this cast
/// restores the outer `'a` so the result composes with arena dispatch.
///
/// # Safety
///
/// `v` must point into storage that lives for `'a` (in practice, an
/// arena-allocated `&'a [DataValue<'a>]` or `&'a [(&'a str, DataValue<'a>)]`).
/// Nothing must reallocate or reset the arena between this call and the
/// last use of the returned reference.
#[inline(always)]
pub(crate) unsafe fn reborrow_arena_value<'a>(v: &DataValue<'a>) -> &'a DataValue<'a> {
    unsafe { &*(v as *const DataValue<'a>) }
}

/// JavaScript/Python-style default truthiness for a [`DataValue`].
/// `truthy_arena` (config-aware) delegates here for the common
/// truthiness modes; operators can call this directly when they need the
/// default rules unconditionally.
#[inline]
pub(crate) fn truthy_js_arena(v: &DataValue<'_>) -> bool {
    match v {
        DataValue::Null => false,
        DataValue::Bool(b) => *b,
        DataValue::Number(n) => !n.is_zero() && !n.is_nan(),
        DataValue::String(s) => !s.is_empty(),
        DataValue::Array(items) => !items.is_empty(),
        DataValue::Object(pairs) => !pairs.is_empty(),
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) | DataValue::Duration(_) => true,
    }
}
