//! `ArenaValue` — internal mirror of `serde_json::Value` for arena allocation.
//!
//! Lifetime `'a` is tied to a `bumpalo::Bump` that lives for the duration of
//! one `evaluate()` call. Converted to/from owned `Value` only at API boundaries
//! (input ingestion, output return, custom operator bridges).
//!
//! Submodules:
//! - [`conversion`] — `value_to_arena`, `arena_to_value`, `reborrow`.
//! - [`coercion`] — numeric coercion (config-aware + default-rules).
//! - [`lookup`] — micro-optimised `key_eq` + object field lookup.
//! - [`traversal`] — path traversal by `PathSegment` slice or dot-string.
//! - [`strings`] — arena string rendering + truthiness.

mod coercion;
mod conversion;
mod lookup;
mod strings;
mod traversal;

#[cfg(any(test, feature = "datetime"))]
pub(crate) use coercion::coerce_arena_to_number;
pub(crate) use coercion::{coerce_arena_to_number_cfg, try_coerce_arena_to_integer_cfg};
pub use conversion::value_to_arena;
pub(crate) use conversion::{arena_to_value, arena_to_value_cow, reborrow_arena_value};
#[cfg(feature = "ext-control")]
pub(crate) use lookup::arena_object_lookup_field;
pub(crate) use strings::{is_truthy_arena, to_string_arena};
#[cfg(feature = "ext-control")]
pub(crate) use traversal::arena_apply_path_element;
pub(crate) use traversal::{
    arena_access_path_str_ref, arena_path_exists_segments, arena_path_exists_str,
    arena_traverse_segments,
};

use bumpalo::Bump;

use crate::value::NumberValue;

#[cfg(feature = "datetime")]
use crate::datetime::{DataDateTime, DataDuration};

/// Arena-allocated mirror of [`serde_json::Value`].
///
/// Lifetime `'a` is tied to a [`bumpalo::Bump`] that lives for the
/// duration of one [`crate::DataLogic::evaluate_ref`] call. Composite
/// variants (`String`, `Array`, `Object`) hold arena-allocated slices
/// instead of `Vec` / `BTreeMap` / heap `String` — that's the key
/// allocation win over the public [`serde_json::Value`] type.
///
/// Most users implementing [`crate::ArenaOperator`] will read inputs via
/// the accessors (`as_f64`, `as_str`, `as_bool`, `is_null`, …) and
/// construct results via the `from_*` constructors, only reaching into
/// the variants directly for advanced cases.
///
/// ## Variants
///
/// - `Null` / `Bool` — inline.
/// - `Number` — wraps [`NumberValue`], distinguishing `Integer(i64)` from
///   `Float(f64)` natively (vs. the opaque `serde_json::Number`).
/// - `String` — UTF-8 bytes allocated in the arena.
/// - `Array` — slice of `ArenaValue` allocated in the arena.
/// - `Object` — slice of `(key, value)` pairs allocated in the arena.
/// - `DateTime` / `Duration` — chrono-backed values, inline. Boundary
///   representation in `serde_json::Value` is `{"datetime": "..."}` /
///   `{"timestamp": "..."}` matching the existing helper contract.
///
/// `Clone` is derived because compiled-time literal precomputation stores a
/// `Box<ArenaValue<'static>>` inside `CompiledNode::Value`, and the enclosing
/// `CompiledNode` derives `Clone`. The clone is shallow for the common
/// primitive variants (Null/Bool/Number are Copy-shaped); slice and
/// string variants clone the reference, not the bytes; only DateTime/Duration
/// pay an actual Clone cost — all rare on the hot path.
#[derive(Debug, Clone)]
pub enum ArenaValue<'a> {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number with native Integer/Float distinction.
    Number(NumberValue),
    /// JSON string allocated in the arena.
    String(&'a str),
    /// JSON array of arena-allocated values.
    Array(&'a [ArenaValue<'a>]),
    /// JSON object as arena-allocated `(key, value)` pairs.
    Object(&'a [(&'a str, ArenaValue<'a>)]),
    /// Chrono-backed datetime (feature `datetime`).
    #[cfg(feature = "datetime")]
    DateTime(DataDateTime),
    /// Chrono-backed duration (feature `datetime`).
    #[cfg(feature = "datetime")]
    Duration(DataDuration),
}

impl<'a> ArenaValue<'a> {
    // ---- Constructors ----

    /// Null literal.
    #[inline]
    pub fn null() -> Self {
        ArenaValue::Null
    }

    /// Boolean literal.
    #[inline]
    pub fn bool(b: bool) -> Self {
        ArenaValue::Bool(b)
    }

    /// Numeric literal from i64 (no allocation).
    #[inline]
    pub fn from_i64(i: i64) -> Self {
        ArenaValue::Number(NumberValue::from_i64(i))
    }

    /// Numeric literal from f64. Whole-valued floats within i64 range
    /// collapse to the integer fast path automatically.
    #[inline]
    pub fn from_f64(f: f64) -> Self {
        ArenaValue::Number(NumberValue::from_f64(f))
    }

    /// String literal — allocates the bytes in the arena.
    #[inline]
    pub fn from_str(s: &str, arena: &'a Bump) -> Self {
        ArenaValue::String(arena.alloc_str(s))
    }

    // ---- Accessors ----

    /// Returns true if this value is `Null`.
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, ArenaValue::Null)
    }

    /// Extract a boolean if this value is a `Bool`.
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ArenaValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract an `i64` if this value is integer-valued.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ArenaValue::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    /// Extract an `f64` if this value is numeric.
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ArenaValue::Number(n) => Some(n.as_f64()),
            _ => None,
        }
    }

    /// Extract a string slice if this value is a string.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ArenaValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Truthiness check matching JavaScript-default semantics. For
    /// config-aware truthiness use [`is_truthy_arena`].
    #[inline]
    pub(crate) fn is_truthy_default(&self) -> bool {
        match self {
            ArenaValue::Null => false,
            ArenaValue::Bool(b) => *b,
            ArenaValue::Number(n) => !n.is_zero() && !n.is_nan(),
            ArenaValue::String(s) => !s.is_empty(),
            ArenaValue::Array(items) => !items.is_empty(),
            ArenaValue::Object(pairs) => !pairs.is_empty(),
            #[cfg(feature = "datetime")]
            ArenaValue::DateTime(_) | ArenaValue::Duration(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn round_trip(input: Value) {
        let arena = Bump::new();
        let av = value_to_arena(&input, &arena);
        let back = arena_to_value(&av);
        assert_eq!(input, back, "round-trip mismatch");
    }

    #[test]
    fn round_trip_primitives() {
        round_trip(json!(null));
        round_trip(json!(true));
        round_trip(json!(false));
        round_trip(json!(0));
        round_trip(json!(-1));
        round_trip(json!(1.5));
        round_trip(json!(""));
        round_trip(json!("hello"));
        round_trip(json!("with unicode: 日本語 🎉"));
    }

    #[test]
    fn round_trip_arrays() {
        round_trip(json!([]));
        round_trip(json!([1, 2, 3]));
        round_trip(json!([null, true, "x", 1.5, [1, 2]]));
    }

    #[test]
    fn round_trip_objects() {
        round_trip(json!({}));
        round_trip(json!({"a": 1}));
        round_trip(json!({"a": 1, "b": "two", "c": [1, 2, 3]}));
        round_trip(json!({"nested": {"deep": {"value": 42}}}));
    }

    #[test]
    fn round_trip_mixed() {
        round_trip(json!({
            "users": [
                {"id": 1, "name": "Alice", "active": true},
                {"id": 2, "name": "Bob", "active": false}
            ],
            "count": 2,
            "metadata": null
        }));
    }

    #[test]
    fn truthiness_matches_default() {
        let arena = Bump::new();
        assert!(!value_to_arena(&json!(null), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(false), &arena).is_truthy_default());
        assert!(value_to_arena(&json!(true), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(0), &arena).is_truthy_default());
        assert!(value_to_arena(&json!(1), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(""), &arena).is_truthy_default());
        assert!(value_to_arena(&json!("x"), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!([]), &arena).is_truthy_default());
        assert!(value_to_arena(&json!([1]), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!({}), &arena).is_truthy_default());
        assert!(value_to_arena(&json!({"a": 1}), &arena).is_truthy_default());
    }

    #[test]
    fn number_int_path_preserved_through_arena() {
        // Round-tripping `42` must keep it an Integer in NumberValue, and
        // serde_json::Number::from(42) must round-trip back as integer-shaped.
        let v = json!(42);
        let arena = Bump::new();
        let av = value_to_arena(&v, &arena);
        if let ArenaValue::Number(n) = &av {
            assert!(n.is_integer(), "42 should be Integer, got {:?}", n);
            assert_eq!(n.as_i64(), Some(42));
        } else {
            panic!("expected Number variant, got {:?}", av);
        }
        let back = arena_to_value(&av);
        assert_eq!(v, back);
    }

    #[test]
    #[cfg(feature = "datetime")]
    fn datetime_arena_round_trip_via_object() {
        // Boundary: Value::Object {"datetime": "..."} ↔ ArenaValue::DateTime
        // via explicit construction (we don't auto-parse on value_to_arena).
        let dt = DataDateTime::parse("2024-01-15T10:30:00Z").expect("parse");
        let av = ArenaValue::DateTime(dt.clone());
        let back = arena_to_value(&av);
        let map = back.as_object().expect("object");
        assert!(map.contains_key("datetime"));
        let s = map
            .get("datetime")
            .and_then(|v| v.as_str())
            .expect("string");
        let dt2 = DataDateTime::parse(s).expect("re-parse");
        assert_eq!(dt, dt2);
    }

    #[test]
    fn coerce_arena_to_number_basics() {
        let arena = Bump::new();
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(42), &arena)),
            Some(42.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(true), &arena)),
            Some(1.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(false), &arena)),
            Some(0.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(null), &arena)),
            Some(0.0)
        );
        let parsed: f64 = "3.14".parse().unwrap();
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!("3.14"), &arena)),
            Some(parsed)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(""), &arena)),
            Some(0.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!([5]), &arena)),
            Some(5.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!([1, 2]), &arena)),
            None
        );
    }
}
