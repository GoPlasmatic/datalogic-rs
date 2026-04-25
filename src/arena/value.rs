//! `ArenaValue` — internal mirror of `serde_json::Value` for arena allocation.
//!
//! Lifetime `'a` is tied to a `bumpalo::Bump` that lives for the duration of
//! one `evaluate()` call. Converted to/from owned `Value` only at API boundaries
//! (input ingestion, output return, custom operator bridges).

use bumpalo::Bump;
use serde_json::Value;

use crate::value::NumberValue;

#[cfg(feature = "datetime")]
use crate::datetime::{DataDateTime, DataDuration};

/// Internal value type for arena-mode evaluation. Mirrors `serde_json::Value`
/// but uses arena-allocated slices and string slices in place of `Vec`/`String`/
/// `BTreeMap` — directly attacking the heap traffic that dominates the v4 profile.
///
/// **Variants**:
/// - `Null` / `Bool` / `Number` — inline (no arena allocation).
///   `Number` uses `crate::value::NumberValue` for native Integer/Float
///   distinction and overflow-aware arithmetic (vs. the opaque
///   `serde_json::Number`).
/// - `String` — UTF-8 bytes allocated in the arena.
/// - `Array` — slice of `ArenaValue` allocated in the arena.
/// - `Object` — sorted slice of `(key, value)` pairs (binary-search lookup).
/// - `DateTime` / `Duration` — chrono-backed values, inline (no arena alloc).
///   Boundary representation in `Value` is `{"datetime": "ISO"}` /
///   `{"timestamp": "..."}` matching the existing `extract_datetime_value`
///   contract in `src/operators/helpers.rs`.
/// - `InputRef` — borrow into the caller's `Arc<Value>` tree without copying.
///   Used by `var` lookups so input data never gets cloned during evaluation.
#[derive(Debug)]
pub(crate) enum ArenaValue<'a> {
    Null,
    Bool(bool),
    Number(NumberValue),
    String(&'a str),
    Array(&'a [ArenaValue<'a>]),
    Object(&'a [(&'a str, ArenaValue<'a>)]),
    #[cfg(feature = "datetime")]
    DateTime(DataDateTime),
    #[cfg(feature = "datetime")]
    Duration(DataDuration),
    InputRef(&'a Value),
}

impl<'a> ArenaValue<'a> {
    /// Truthiness check matching the JavaScript-default semantics used by the
    /// existing `is_truthy` helper. Replicated here to avoid a `to_value` round
    /// trip during predicate evaluation.
    #[inline]
    #[allow(dead_code)]
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
            ArenaValue::InputRef(v) => match v {
                Value::Null => false,
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
                Value::String(s) => !s.is_empty(),
                Value::Array(arr) => !arr.is_empty(),
                Value::Object(obj) => !obj.is_empty(),
            },
        }
    }
}

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
        ArenaValue::InputRef(v) => (*v).clone(),
    }
}

/// Promote an owned `serde_json::Value` into the arena. Used by the custom
/// operator bridge: a custom op returns owned `Value`, which we wrap in an
/// `ArenaValue` so the rest of the arena pipeline can consume it without
/// special-casing.
///
/// Strings and primitives are copied into the arena. Arrays/objects are
/// recursively converted. Datetime/Duration objects are NOT eagerly parsed
/// here — callers that need temporal semantics extract on demand (matches
/// the existing `extract_datetime_value` contract).
pub(crate) fn value_to_arena<'a>(v: &Value, arena: &'a Bump) -> ArenaValue<'a> {
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
            let pairs = arena.alloc_slice_fill_iter(
                obj.iter()
                    .map(|(k, v)| (arena.alloc_str(k.as_str()) as &str, value_to_arena(v, arena))),
            );
            ArenaValue::Object(pairs)
        }
    }
}

// =============================================================================
// Arena-aware helpers (Phase 0). These mirror the existing helpers in
// `src/operators/helpers.rs` and `src/value_helpers.rs` but operate on
// `&ArenaValue<'a>`, deferring to the underlying `&Value` only when the
// variant is `InputRef`.
// =============================================================================

/// Coerce an `ArenaValue` to f64 using the engine's coercion rules. Mirrors
/// `value_helpers::coerce_to_number` but lifts ArenaValue's native variants.
pub(crate) fn coerce_arena_to_number(v: &ArenaValue<'_>) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::Bool(true) => Some(1.0),
        ArenaValue::Bool(false) => Some(0.0),
        ArenaValue::Null => Some(0.0),
        ArenaValue::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        ArenaValue::Array(items) => match items.len() {
            0 => Some(0.0),
            1 => coerce_arena_to_number(&items[0]),
            _ => None,
        },
        ArenaValue::Object(_) => None,
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(_) | ArenaValue::Duration(_) => None,
        ArenaValue::InputRef(v) => coerce_value_to_number_default(v),
    }
}

/// Default JSON Logic numeric coercion against `&serde_json::Value`. Mirrors
/// `value_helpers::coerce_to_number` for the default config (bool→number,
/// null→0, empty string→0). Used when the arena value still holds an
/// `InputRef` to an input-side `Value`.
fn coerce_value_to_number_default(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        Value::Array(arr) => match arr.len() {
            0 => Some(0.0),
            1 => coerce_value_to_number_default(&arr[0]),
            _ => None,
        },
        Value::Object(_) => None,
    }
}

/// Compare two `ArenaValue`s using JSON Logic ordering rules: Null < Bool
/// < Number < String < Array < Object, with same-type comparison when
/// applicable. Mirrors `compare_values` in `src/operators/array.rs:1456`.
#[cfg(feature = "ext-array")]
pub(crate) fn compare_arena_values(
    a: &ArenaValue<'_>,
    b: &ArenaValue<'_>,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // Resolve InputRef once so the comparison logic can stay flat.
    let af = arena_value_to_f64_for_compare(a);
    let bf = arena_value_to_f64_for_compare(b);
    match (af, bf) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => {
            // Fall back to lexical string comparison for non-numeric values
            // (matches existing behavior for sort with mixed/string keys).
            let as_str = arena_value_to_string_lossy(a);
            let bs_str = arena_value_to_string_lossy(b);
            as_str.cmp(&bs_str)
        }
    }
}

#[cfg(feature = "ext-array")]
fn arena_value_to_f64_for_compare(v: &ArenaValue<'_>) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        ArenaValue::Null => None,
        ArenaValue::InputRef(Value::Number(n)) => n.as_f64(),
        ArenaValue::InputRef(Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        ArenaValue::InputRef(Value::Null) => None,
        _ => None,
    }
}

#[cfg(feature = "ext-array")]
fn arena_value_to_string_lossy(v: &ArenaValue<'_>) -> String {
    match v {
        ArenaValue::String(s) => (*s).to_string(),
        ArenaValue::InputRef(Value::String(s)) => s.clone(),
        ArenaValue::Null | ArenaValue::InputRef(Value::Null) => String::new(),
        _ => format!("{:?}", v),
    }
}

/// Render an `ArenaValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Mirrors `helpers::to_string_cow` but produces
/// arena-resident strings so string-building operators (cat, substr) can
/// chain without heap traffic.
#[allow(dead_code)] // wired up in Phase 6 (string ops migration)
pub(crate) fn to_string_arena<'a>(v: &ArenaValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        ArenaValue::String(s) => *s,
        ArenaValue::InputRef(Value::String(s)) => arena.alloc_str(s),
        ArenaValue::Null | ArenaValue::InputRef(Value::Null) => "",
        ArenaValue::Bool(true) | ArenaValue::InputRef(Value::Bool(true)) => "true",
        ArenaValue::Bool(false) | ArenaValue::InputRef(Value::Bool(false)) => "false",
        ArenaValue::Number(n) => arena.alloc_str(&n.to_string()),
        ArenaValue::InputRef(Value::Number(n)) => arena.alloc_str(&n.to_string()),
        // Composite types: serialize as JSON. Rare path; cost acceptable.
        other => arena.alloc_str(&arena_to_value(other).to_string()),
    }
}

/// Config-aware truthiness for `ArenaValue`. Mirrors `helpers::is_truthy`.
#[allow(dead_code)] // wired up in Phase 4 (control ops + collection bodies)
pub(crate) fn is_truthy_arena(v: &ArenaValue<'_>, engine: &crate::DataLogic) -> bool {
    use crate::config::TruthyEvaluator;
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => v.is_truthy_default(),
        TruthyEvaluator::StrictBoolean => match v {
            ArenaValue::Null => false,
            ArenaValue::Bool(b) => *b,
            ArenaValue::InputRef(Value::Null) => false,
            ArenaValue::InputRef(Value::Bool(b)) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(&arena_to_value(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn input_ref_round_trip() {
        let original = json!({"a": [1, 2, 3], "b": "x"});
        let av = ArenaValue::InputRef(&original);
        let back = arena_to_value(&av);
        assert_eq!(original, back);
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
        // back should be {"datetime": "2024-01-15T10:30:00Z"}
        let map = back.as_object().expect("object");
        assert!(map.contains_key("datetime"));
        // And re-parse from the boundary form gives the same DateTime back.
        let s = map.get("datetime").and_then(|v| v.as_str()).expect("string");
        let dt2 = DataDateTime::parse(s).expect("re-parse");
        assert_eq!(dt, dt2);
    }

    #[test]
    fn coerce_arena_to_number_basics() {
        let arena = Bump::new();
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!(42), &arena)), Some(42.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!(true), &arena)), Some(1.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!(false), &arena)), Some(0.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!(null), &arena)), Some(0.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!("3.14"), &arena)), Some(3.14));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!(""), &arena)), Some(0.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!([5]), &arena)), Some(5.0));
        assert_eq!(coerce_arena_to_number(&value_to_arena(&json!([1, 2]), &arena)), None);
    }
}
