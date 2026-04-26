//! Boundary conversion: `serde_json::Value` ↔ `ArenaValue`, plus the
//! `reborrow` helper used to lift a borrowed `ArenaValue` back to a fresh
//! `ArenaValue<'a>` for arena slice construction.

use bumpalo::Bump;
use serde_json::Value;
use std::borrow::Cow;

use super::ArenaValue;
use crate::value::NumberValue;

/// Walk the arena value tree and produce an owned `serde_json::Value`. Called
/// once at the public API boundary on the final result. Allocations here are
/// the same ones the user would have paid for in the non-arena path — we
/// haven't added work, only deferred and consolidated it.
pub(crate) fn arena_to_value(v: &ArenaValue<'_>) -> Value {
    match v {
        ArenaValue::Null => Value::Null,
        ArenaValue::Bool(b) => Value::Bool(*b),
        ArenaValue::Number(n) => match n.to_serde() {
            Some(num) => Value::Number(num),
            None => Value::Null, // NaN/Inf collapse to Null at the JSON boundary.
        },
        ArenaValue::String(s) => Value::String((*s).to_string()),
        ArenaValue::Array(items) => Value::Array(items.iter().map(arena_to_value).collect()),
        ArenaValue::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in *pairs {
                map.insert((*k).to_string(), arena_to_value(v));
            }
            Value::Object(map)
        }
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(dt) => {
            // Match the boundary contract honored by extract_datetime_value:
            // `{"datetime": "<ISO>"}`. This shape round-trips through the
            // input boundary so a datetime computed in arena mode can be
            // re-fed as input.
            let mut map = serde_json::Map::new();
            map.insert("datetime".to_string(), Value::String(dt.to_iso_string()));
            Value::Object(map)
        }
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(d) => {
            let mut map = serde_json::Map::new();
            map.insert("timestamp".to_string(), Value::String(d.to_string()));
            Value::Object(map)
        }
    }
}

/// Deep-convert a `&Value` into an arena-resident `ArenaValue`. Allocates
/// strings, arrays and objects in `arena`; primitives are inline. Used at the
/// API boundary by `evaluate_*` and at the input edge of arena-native helpers.
pub fn value_to_arena<'a>(v: &Value, arena: &'a Bump) -> ArenaValue<'a> {
    match v {
        Value::Null => ArenaValue::Null,
        Value::Bool(b) => ArenaValue::Bool(*b),
        Value::Number(n) => ArenaValue::Number(NumberValue::from_serde(n)),
        Value::String(s) => ArenaValue::String(arena.alloc_str(s)),
        Value::Array(arr) => {
            let items = arena.alloc_slice_fill_iter(arr.iter().map(|x| value_to_arena(x, arena)));
            ArenaValue::Array(items)
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
            ArenaValue::Object(pairs)
        }
    }
}

/// Build a fresh `ArenaValue` mirroring the source variant. Used to
/// reborrow into an arena slice (e.g., when constructing an Object from
/// arena-resident field values). Most variants are Copy-shaped; only
/// DateTime/Duration require Clone.
#[inline]
pub(crate) fn reborrow_arena_value<'a>(av: &ArenaValue<'a>) -> ArenaValue<'a> {
    match av {
        ArenaValue::Null => ArenaValue::Null,
        ArenaValue::Bool(b) => ArenaValue::Bool(*b),
        ArenaValue::Number(n) => ArenaValue::Number(*n),
        ArenaValue::String(s) => ArenaValue::String(s),
        ArenaValue::Array(items) => ArenaValue::Array(items),
        ArenaValue::Object(pairs) => ArenaValue::Object(pairs),
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(dt) => ArenaValue::DateTime(dt.clone()),
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(d) => ArenaValue::Duration(d.clone()),
    }
}

/// Render an `ArenaValue` as a `Cow<Value>` for use with `&Value`-based
/// boundary helpers. Always owned (allocates a fresh `Value`) since arena
/// values are not borrow-compatible with `serde_json::Value`. Used only at
/// the API boundary (input/output conversion); no in-loop callers.
#[inline]
pub(crate) fn arena_to_value_cow<'a>(v: &'a ArenaValue<'a>) -> Cow<'a, Value> {
    Cow::Owned(arena_to_value(v))
}
