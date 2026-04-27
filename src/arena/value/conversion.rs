//! Boundary helpers around the arena value type.
//!
//! The `serde_json::Value` ↔ `DataValue` bridges live behind the `compat`
//! feature — they're only needed for the deprecated v4 API surface in
//! `crate::compat`. The lifetime-only `reborrow_arena_value` helper is
//! always available since it doesn't touch serde_json.

use super::DataValue;

/// Build a fresh `DataValue` mirroring the source variant. Used to
/// reborrow into an arena slice (e.g., when constructing an Object from
/// arena-resident field values). Most variants are Copy-shaped; only
/// DateTime/Duration require Clone.
#[inline]
pub(crate) fn reborrow_arena_value<'a>(av: &DataValue<'a>) -> DataValue<'a> {
    match av {
        DataValue::Null => DataValue::Null,
        DataValue::Bool(b) => DataValue::Bool(*b),
        DataValue::Number(n) => DataValue::Number(*n),
        DataValue::String(s) => DataValue::String(s),
        DataValue::Array(items) => DataValue::Array(items),
        DataValue::Object(pairs) => DataValue::Object(pairs),
        #[cfg(feature = "datetime")]
        DataValue::DateTime(dt) => DataValue::DateTime(*dt),
        #[cfg(feature = "datetime")]
        DataValue::Duration(d) => DataValue::Duration(*d),
    }
}

#[cfg(feature = "compat")]
pub(crate) use compat_impl::arena_to_value;
#[cfg(feature = "compat")]
pub use compat_impl::value_to_arena;

#[cfg(feature = "compat")]
mod compat_impl {
    use super::DataValue;
    use bumpalo::Bump;
    use serde_json::Value;

    /// Walk the arena value tree and produce an owned `serde_json::Value`. Called
    /// once at the public API boundary on the final result. Allocations here are
    /// the same ones the user would have paid for in the non-arena path — we
    /// haven't added work, only deferred and consolidated it.
    pub(crate) fn arena_to_value(v: &DataValue<'_>) -> Value {
        match v {
            DataValue::Null => Value::Null,
            DataValue::Bool(b) => Value::Bool(*b),
            DataValue::Number(n) => match crate::value::number_to_serde(*n) {
                Some(num) => Value::Number(num),
                None => Value::Null, // NaN/Inf collapse to Null at the JSON boundary.
            },
            DataValue::String(s) => Value::String((*s).to_string()),
            DataValue::Array(items) => Value::Array(items.iter().map(arena_to_value).collect()),
            DataValue::Object(pairs) => {
                let mut map = serde_json::Map::new();
                for (k, v) in *pairs {
                    map.insert((*k).to_string(), arena_to_value(v));
                }
                Value::Object(map)
            }
            #[cfg(feature = "datetime")]
            DataValue::DateTime(dt) => {
                // Match the boundary contract honored by extract_datetime_value:
                // `{"datetime": "<ISO>"}`. This shape round-trips through the
                // input boundary so a datetime computed in arena mode can be
                // re-fed as input.
                let mut map = serde_json::Map::new();
                map.insert("datetime".to_string(), Value::String(dt.to_iso_string()));
                Value::Object(map)
            }
            #[cfg(feature = "datetime")]
            DataValue::Duration(d) => {
                let mut map = serde_json::Map::new();
                map.insert("timestamp".to_string(), Value::String(d.to_string()));
                Value::Object(map)
            }
        }
    }

    /// Deep-convert a `&Value` into an arena-resident `DataValue`. Allocates
    /// strings, arrays and objects in `arena`; primitives are inline. Used at the
    /// API boundary by `evaluate_*` and at the input edge of arena-native helpers.
    pub fn value_to_arena<'a>(v: &Value, arena: &'a Bump) -> DataValue<'a> {
        match v {
            Value::Null => DataValue::Null,
            Value::Bool(b) => DataValue::Bool(*b),
            Value::Number(n) => DataValue::Number(crate::value::number_from_serde(n)),
            Value::String(s) => DataValue::String(arena.alloc_str(s)),
            Value::Array(arr) => {
                let items =
                    arena.alloc_slice_fill_iter(arr.iter().map(|x| value_to_arena(x, arena)));
                DataValue::Array(items)
            }
            Value::Object(obj) => {
                // Build sorted (key, value) pairs in the arena. JSON objects from
                // serde_json (without `preserve_order`) are already sorted by key
                // because the underlying `Map` is `BTreeMap`. We rely on that here.
                let pairs = arena.alloc_slice_fill_iter(obj.iter().map(|(k, v)| {
                    (
                        arena.alloc_str(k.as_str()) as &str,
                        value_to_arena(v, arena),
                    )
                }));
                DataValue::Object(pairs)
            }
        }
    }

}
