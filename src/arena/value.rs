//! `ArenaValue` — internal mirror of `serde_json::Value` for arena allocation.
//!
//! Lifetime `'a` is tied to a `bumpalo::Bump` that lives for the duration of
//! one `evaluate()` call. Converted to/from owned `Value` only at API boundaries
//! (input ingestion, output return, custom operator bridges).

use bumpalo::Bump;
use serde_json::Value;

/// Internal value type for arena-mode evaluation. Mirrors `serde_json::Value`
/// but uses arena-allocated slices and string slices in place of `Vec`/`String`/
/// `BTreeMap` — directly attacking the heap traffic that dominates the v4 profile.
///
/// **Variants**:
/// - `Null` / `Bool` / `Number` — inline (no arena allocation)
/// - `String` — UTF-8 bytes allocated in the arena
/// - `Array` — slice of `ArenaValue` allocated in the arena
/// - `Object` — sorted slice of `(key, value)` pairs (binary-search lookup)
/// - `InputRef` — borrow into the caller's `Arc<Value>` tree without copying.
///   Used by `var` lookups so input data never gets cloned during evaluation.
#[derive(Debug)]
pub(crate) enum ArenaValue<'a> {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(&'a str),
    Array(&'a [ArenaValue<'a>]),
    Object(&'a [(&'a str, ArenaValue<'a>)]),
    InputRef(&'a Value),
}

impl<'a> ArenaValue<'a> {
    /// Truthiness check matching the JavaScript-default semantics used by the
    /// existing `is_truthy` helper. Replicated here to avoid a `to_value` round
    /// trip during predicate evaluation. Reserved for Phase 4 when predicates
    /// can run in arena mode.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn is_truthy_default(&self) -> bool {
        match self {
            ArenaValue::Null => false,
            ArenaValue::Bool(b) => *b,
            ArenaValue::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
            ArenaValue::String(s) => !s.is_empty(),
            ArenaValue::Array(items) => !items.is_empty(),
            ArenaValue::Object(pairs) => !pairs.is_empty(),
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
        ArenaValue::Number(n) => Value::Number(n.clone()),
        ArenaValue::String(s) => Value::String((*s).to_string()),
        ArenaValue::Array(items) => {
            Value::Array(items.iter().map(arena_to_value).collect())
        }
        ArenaValue::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in *pairs {
                map.insert((*k).to_string(), arena_to_value(v));
            }
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
/// recursively converted. A future optimization could store top-level
/// owned values as `InputRef` to a longer-lived storage — out of POC scope.
pub(crate) fn value_to_arena<'a>(v: &Value, arena: &'a Bump) -> ArenaValue<'a> {
    match v {
        Value::Null => ArenaValue::Null,
        Value::Bool(b) => ArenaValue::Bool(*b),
        Value::Number(n) => ArenaValue::Number(n.clone()),
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
}
