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
