//! Integration tests for the `CustomOperator` trait — zero-clone custom operators.

#![cfg(feature = "compat")]
#![allow(deprecated)]

use bumpalo::Bump;
use datalogic_rs::compat::LegacyApi;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{ArenaExt, CustomOperator, DataValue, Engine, Result};
use serde_json::json;

/// Doubles the first numeric argument. Returns a fresh arena-allocated number.
struct DoubleArena;
impl CustomOperator for DoubleArena {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

/// Concatenates string args directly into the arena.
struct CatArena;
impl CustomOperator for CatArena {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let mut buf = bumpalo::collections::String::new_in(arena);
        for av in args {
            if let Some(s) = av.as_str() {
                buf.push_str(s);
            }
        }
        Ok(arena.alloc(DataValue::String(buf.into_bump_str())))
    }
}

#[test]
fn arena_operator_double_at_root() {
    let engine = Engine::builder()
        .add_operator("double", DoubleArena)
        .build();

    let compiled = engine.compile_serde_value(&json!({"double": 21})).unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn arena_operator_inside_filter() {
    // The whole point of Operator: zero-clone use inside iteration.
    let engine = Engine::builder()
        .add_operator("double", DoubleArena)
        .build();

    let compiled = engine
        .compile_serde_value(&json!({"map": [{"var": "xs"}, {"double": {"var": ""}}]}))
        .unwrap();
    let result = engine
        .evaluate_ref(&compiled, &json!({"xs": [1, 2, 3, 4]}))
        .unwrap();
    assert_eq!(result, json!([2, 4, 6, 8]));
}

#[test]
fn arena_operator_string_result() {
    let engine = Engine::builder().add_operator("xcat", CatArena).build();

    let compiled = engine
        .compile_serde_value(&json!({"xcat": ["he", "ll", "o"]}))
        .unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
    assert_eq!(result, json!("hello"));
}

#[test]
fn evaluate_ref_var_inside_filter_bridge_object_input() {
    // Object input forces filter to bridge to value-mode (ResolvedInput::Bridge).
    // Inside that bridge, var lookups need to see the input — exercises that
    // bridges synthesize their own context from ctx.root_input().
    let engine = Engine::new();
    let logic = serde_json::json!({"filter": [{"var": "items"}, {">": [{"var": ""}, 2]}]});
    let compiled = engine.compile_serde_value(&logic).unwrap();
    let result = engine
        .evaluate_ref(&compiled, &serde_json::json!({"items": [1, 2, 3, 4, 5]}))
        .unwrap();
    assert_eq!(result, serde_json::json!([3, 4, 5]));
}

#[test]
fn arena_operator_with_input_ref() {
    // Custom op consumes an InputRef arg (var lookup against root).
    let engine = Engine::builder()
        .add_operator("double", DoubleArena)
        .build();

    let compiled = engine
        .compile_serde_value(&json!({"double": {"var": "n"}}))
        .unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({"n": 5})).unwrap();
    assert_eq!(result, json!(10));
}

/// Custom op that reads an InputRef directly via context-aware var lookup.
/// Equivalent to the legacy "read_field" / "read_root" helpers; here it just
/// inspects its first arg, which the dispatcher already evaluated for us.
struct ReadField;
impl CustomOperator for ReadField {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        // The arg has already been evaluated through the var lookup;
        // we just hand it back. This proves InputRef args reach the op
        // without round-trips through `serde_json::Value`.
        let av = args
            .first()
            .copied()
            .unwrap_or_else(|| arena.alloc(DataValue::Null));
        Ok(av)
    }
}

#[test]
fn arena_operator_passthrough_input_ref() {
    let engine = Engine::builder()
        .add_operator("read_field", ReadField)
        .build();

    let compiled = engine
        .compile_serde_value(&serde_json::json!({"read_field": {"var": "name"}}))
        .unwrap();
    let result = engine
        .evaluate_ref(&compiled, &serde_json::json!({"name": "Alice"}))
        .unwrap();
    assert_eq!(result, serde_json::json!("Alice"));
}

/// Op that returns the iter item's "active" field — exercises that arena
/// custom ops invoked inside `filter` see the iter frame's data via their
/// pre-evaluated args.
struct ReadActiveField;
impl CustomOperator for ReadActiveField {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let av = args
            .first()
            .copied()
            .unwrap_or_else(|| arena.alloc(DataValue::Null));
        Ok(av)
    }
}

#[test]
fn arena_operator_inside_filter_reads_iter_item_field() {
    let engine = Engine::builder()
        .add_operator("identity", ReadActiveField)
        .build();

    // Filter passes each item to the predicate; the predicate calls
    // `identity` on `{"var": "active"}`, which the dispatcher resolves
    // against the iter frame.
    let compiled = engine
        .compile_serde_value(&serde_json::json!({
            "filter": [{"var": "items"}, {"identity": {"var": "active"}}]
        }))
        .unwrap();
    let result = engine
        .evaluate_ref(
            &compiled,
            &serde_json::json!({"items": [
                {"id": 1, "active": true},
                {"id": 2, "active": false},
                {"id": 3, "active": true}
            ]}),
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            {"id": 1, "active": true},
            {"id": 3, "active": true}
        ])
    );
}

/// Returns an `Object` with one key per `ArenaExt` helper, allowing one
/// custom-operator round-trip to exercise every method end-to-end.
struct ArenaExtSampler;
impl CustomOperator for ArenaExtSampler {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        // Build an Array via the helper, then nest it (and one of every
        // primitive helper) into an Object.
        let arr_items = [*arena.i64(7), *arena.f64(2.5), *arena.bool(true)];
        let arr = arena.array(&arr_items);

        let pairs: [(&str, DataValue<'_>); 6] = [
            ("null", *arena.null()),
            ("bool", *arena.bool(false)),
            ("small_int", *arena.i64(5)),   // singleton path
            ("big_int", *arena.i64(1_000)), // arena-allocated path
            ("float", *arena.f64(1.5)),
            ("string", *arena.string("hi")),
        ];
        let mut all_pairs = Vec::with_capacity(pairs.len() + 2);
        all_pairs.extend_from_slice(&pairs);
        all_pairs.push(("array", *arr));
        all_pairs.push(("empty_str", *arena.string(""))); // empty -> singleton
        Ok(arena.object(&all_pairs))
    }
}

#[test]
fn arena_ext_helpers_round_trip_through_custom_op() {
    let engine = Engine::builder()
        .add_operator("sample", ArenaExtSampler)
        .build();

    let result = engine
        .evaluate_json_value(&json!({"sample": []}), &json!({}))
        .unwrap();
    assert_eq!(
        result,
        json!({
            "null": null,
            "bool": false,
            "small_int": 5,
            "big_int": 1000,
            "float": 1.5,
            "string": "hi",
            "array": [7, 2.5, true],
            "empty_str": "",
        })
    );
}

#[test]
fn arena_ext_singletons_avoid_arena_writes() {
    // Repeated singleton lookups must be cheap and stable; running them
    // many times in the same scope should not blow the arena.
    let arena = Bump::new();
    for _ in 0..1_000 {
        let n = ArenaExt::null(&arena);
        assert!(matches!(n, DataValue::Null));
        let t = ArenaExt::bool(&arena, true);
        assert!(matches!(t, DataValue::Bool(true)));
        let zero = ArenaExt::i64(&arena, 0);
        assert_eq!(zero.as_i64(), Some(0));
        let empty = ArenaExt::string(&arena, "");
        assert_eq!(empty.as_str(), Some(""));
    }
    // Singletons should not have grown the arena meaningfully — the
    // exact bytes_allocated value is implementation-defined, but a
    // 1k-iteration loop hitting only singletons must stay well under
    // what 4 × 1000 fresh DataValue allocations would consume.
    assert!(arena.allocated_bytes() < 256);
}

#[test]
fn custom_operator_names_lists_registered_custom_ops() {
    let empty = Engine::new();
    assert_eq!(empty.custom_operator_names().count(), 0);

    let engine = Engine::builder()
        .add_operator("double", DoubleArena)
        .add_operator("xcat", CatArena)
        .build();

    let mut names: Vec<&str> = engine.custom_operator_names().collect();
    names.sort();
    assert_eq!(names, vec!["double", "xcat"]);
}
