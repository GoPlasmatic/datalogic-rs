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
pub(crate) use coercion::coerce_arena_to_number;
pub(crate) use coercion::{coerce_arena_to_number_cfg, try_coerce_arena_to_integer_cfg};
#[cfg(feature = "compat")]
pub(crate) use conversion::arena_to_value;
pub(crate) use conversion::reborrow_arena_value;
#[cfg(feature = "compat")]
pub use conversion::value_to_arena;
pub(crate) use lookup::arena_object_lookup_field;
pub(crate) use strings::{data_to_json_string, is_truthy_arena, to_string_arena};
pub(crate) use traversal::arena_apply_path_element;
pub(crate) use traversal::{
    arena_access_path_str_ref, arena_path_exists_segments, arena_path_exists_str,
    arena_traverse_segments,
};

pub use datavalue::DataValue;

/// JavaScript/Python-style default truthiness for a [`DataValue`].
/// `is_truthy_arena` (config-aware) delegates here for the common
/// truthiness modes; operators can call this directly when they need the
/// default rules unconditionally.
#[inline]
pub(crate) fn is_truthy_default(v: &DataValue<'_>) -> bool {
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
