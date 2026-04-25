//! Integration tests for the `ArenaOperator` trait — zero-clone custom operators.

use bumpalo::Bump;
use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Result};
use serde_json::json;

/// Doubles the first numeric argument. Returns a fresh arena-allocated number.
struct DoubleArena;
impl ArenaOperator for DoubleArena {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(ArenaValue::from_f64(n * 2.0)))
    }
}

/// Concatenates string args directly into the arena.
struct CatArena;
impl ArenaOperator for CatArena {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        let mut buf = bumpalo::collections::String::new_in(arena);
        for av in args {
            if let Some(s) = av.as_str() {
                buf.push_str(s);
            }
        }
        Ok(arena.alloc(ArenaValue::String(buf.into_bump_str())))
    }
}

#[test]
fn arena_operator_double_at_root() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("double".into(), Box::new(DoubleArena));

    let compiled = engine.compile(&json!({"double": 21})).unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn arena_operator_inside_filter() {
    // The whole point of ArenaOperator: zero-clone use inside iteration.
    let mut engine = DataLogic::new();
    engine.add_arena_operator("double".into(), Box::new(DoubleArena));

    let compiled = engine
        .compile(&json!({"map": [{"var": "xs"}, {"double": {"var": ""}}]}))
        .unwrap();
    let result = engine
        .evaluate_ref(&compiled, &json!({"xs": [1, 2, 3, 4]}))
        .unwrap();
    assert_eq!(result, json!([2, 4, 6, 8]));
}

#[test]
fn arena_operator_string_result() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("xcat".into(), Box::new(CatArena));

    let compiled = engine.compile(&json!({"xcat": ["he", "ll", "o"]})).unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
    assert_eq!(result, json!("hello"));
}

#[test]
fn arena_operator_takes_precedence_over_legacy() {
    use datalogic_rs::{ContextStack, Evaluator, Operator};
    use serde_json::Value;

    struct LegacyDouble;
    impl Operator for LegacyDouble {
        fn evaluate(
            &self,
            args: &[Value],
            _context: &mut ContextStack,
            _evaluator: &dyn Evaluator,
        ) -> Result<Value> {
            // Wrong intentionally — to prove the arena form runs instead.
            let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!(n * 99.0))
        }
    }

    let mut engine = DataLogic::new();
    engine.add_operator("double".into(), Box::new(LegacyDouble));
    engine.add_arena_operator("double".into(), Box::new(DoubleArena));

    let compiled = engine.compile(&json!({"double": 21})).unwrap();
    let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
    assert_eq!(result, json!(42), "arena form should win, not legacy");
}

#[test]
fn arena_only_operator_works_in_value_mode_too() {
    // Even when value-mode dispatch is taken (e.g., trivial rule that
    // never enters arena dispatch), an arena-only operator must still
    // work via the synthesis bridge.
    let mut engine = DataLogic::new();
    engine.add_arena_operator("double".into(), Box::new(DoubleArena));

    // Plain `evaluate` form — Arc input.
    use std::sync::Arc;
    let compiled = engine.compile(&json!({"double": 7})).unwrap();
    let result = engine
        .evaluate(&compiled, Arc::new(json!({})))
        .unwrap();
    assert_eq!(result, json!(14));
}

#[test]
fn evaluate_ref_var_inside_filter_bridge_object_input() {
    // Object input forces filter to bridge to value-mode (ResolvedInput::Bridge).
    // Inside that bridge, var lookups need to see the input — exercises that
    // bridges synthesize their own context from actx.root_input().
    let engine = DataLogic::new();
    let logic = serde_json::json!({"filter": [{"var": "items"}, {">": [{"var": ""}, 2]}]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine
        .evaluate_ref(&compiled, &serde_json::json!({"items": [1, 2, 3, 4, 5]}))
        .unwrap();
    assert_eq!(result, serde_json::json!([3, 4, 5]));
}

#[test]
fn evaluate_ref_legacy_custom_op_inside_arena_iter() {
    // Legacy custom op invoked INSIDE map (arena dispatch). The op reads
    // context.current().data() — must see the iter item, not the engine
    // placeholder.
    use datalogic_rs::{ContextStack, Evaluator, Operator};
    use serde_json::Value;

    struct DoubleLegacy;
    impl Operator for DoubleLegacy {
        fn evaluate(
            &self,
            args: &[Value],
            context: &mut ContextStack,
            evaluator: &dyn Evaluator,
        ) -> Result<Value> {
            // Legacy contract: args are unevaluated; call evaluator to resolve.
            let v = evaluator.evaluate(&args[0], context)?;
            let n = v.as_i64().unwrap_or(0);
            Ok(serde_json::json!(n * 2))
        }
    }

    let mut engine = DataLogic::new();
    engine.add_operator("double_legacy".into(), Box::new(DoubleLegacy));

    let compiled = engine
        .compile(&serde_json::json!({
            "map": [{"var": "xs"}, {"double_legacy": {"var": ""}}]
        }))
        .unwrap();
    let result = engine
        .evaluate_ref(&compiled, &serde_json::json!({"xs": [1, 2, 3]}))
        .unwrap();
    assert_eq!(result, serde_json::json!([2, 4, 6]));
}

#[test]
fn evaluate_ref_legacy_custom_op() {
    // Legacy `Operator` trait dispatch from evaluate_ref. The legacy op needs
    // a real ContextStack with the input data.
    use datalogic_rs::{ContextStack, Evaluator, Operator};
    use serde_json::Value;

    struct ReadField;
    impl Operator for ReadField {
        fn evaluate(
            &self,
            args: &[Value],
            context: &mut ContextStack,
            _evaluator: &dyn Evaluator,
        ) -> Result<Value> {
            let key = args.first().and_then(|v| v.as_str()).unwrap_or("");
            // Read from context's root data — must reflect the caller's input.
            let frame = context.current();
            let data = frame.data();
            Ok(data
                .get(key)
                .cloned()
                .unwrap_or(Value::Null))
        }
    }

    let mut engine = DataLogic::new();
    engine.add_operator("read_field".into(), Box::new(ReadField));

    let compiled = engine
        .compile(&serde_json::json!({"read_field": "name"}))
        .unwrap();
    let result = engine
        .evaluate_ref(&compiled, &serde_json::json!({"name": "Alice"}))
        .unwrap();
    assert_eq!(result, serde_json::json!("Alice"));
}

#[test]
fn evaluate_ref_legacy_op_reading_context_in_arena_dispatch() {
    // A legacy op reads context.current().data() — verifies the placeholder
    // ContextStack in evaluate_via_arena_ref is properly substituted at the
    // bridge boundary so the op sees the caller's input.
    use datalogic_rs::{ContextStack, Evaluator, Operator};
    use serde_json::Value;

    struct ReadRoot;
    impl Operator for ReadRoot {
        fn evaluate(
            &self,
            args: &[Value],
            context: &mut ContextStack,
            _evaluator: &dyn Evaluator,
        ) -> Result<Value> {
            let key = args.first().and_then(|v| v.as_str()).unwrap_or("");
            // Read the FIELD from the current context. In a filter body the
            // "current" frame is the iter item — but the args here are the
            // root path string, not the iter item.
            let frame = context.current();
            let data = frame.data();
            Ok(data.get(key).cloned().unwrap_or(Value::Null))
        }
    }

    let mut engine = DataLogic::new();
    engine.add_operator("read_root".into(), Box::new(ReadRoot));

    // Rule that triggers arena dispatch: filter with simple var input.
    // The body uses a legacy custom op that reads context.current().data().
    // Inside filter iteration, current() is the iter item — so args[0] read
    // against it should find the 'tag' field of each item.
    let compiled = engine
        .compile(&serde_json::json!({
            "filter": [{"var": "items"}, {"read_root": "active"}]
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
    // Filter keeps items where {"read_root": "active"} is truthy.
    assert_eq!(
        result,
        serde_json::json!([
            {"id": 1, "active": true},
            {"id": 3, "active": true}
        ])
    );
}

#[test]
fn arena_operator_with_input_ref() {
    // Custom op consumes an InputRef arg (var lookup against root).
    let mut engine = DataLogic::new();
    engine.add_arena_operator("double".into(), Box::new(DoubleArena));

    let compiled = engine.compile(&json!({"double": {"var": "n"}})).unwrap();
    let result = engine
        .evaluate_ref(&compiled, &json!({"n": 5}))
        .unwrap();
    assert_eq!(result, json!(10));
}
