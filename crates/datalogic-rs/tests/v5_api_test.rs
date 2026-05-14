//! End-to-end smoke tests for the v5 public surface.
//!
//! Exercises the v5 entry points (`EngineBuilder`, `compile`, `evaluate`,
//! `eval_str`, `eval_into`).

use bumpalo::Bump;
use datalogic_rs::{DataValue, Engine};

#[test]
fn builder_default_engine() {
    let engine = Engine::builder().build();
    let result = engine.eval_str(r#"{"+": [1, 2, 3]}"#, "null").unwrap();
    assert_eq!(result, "6");
}

#[test]
fn evaluate_str_one_shot_with_variable() {
    let engine = Engine::new();
    let result = engine
        .eval_str(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#)
        .unwrap();
    assert_eq!(result, "\"Alice\"");
}

#[test]
fn evaluate_arena_path() {
    let engine = Engine::new();
    let compiled = engine.compile(r#"{">": [{"var": "n"}, 5]}"#).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str(r#"{"n": 42}"#, &arena).unwrap();
    let result = engine.evaluate(&compiled, data, &arena).unwrap();
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn compile_then_evaluate_str_round_trip() {
    let engine = Engine::new();
    let compiled = engine.compile(r#"{"==": [1, 1]}"#).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str("null", &arena).unwrap();
    let result = engine.evaluate(&compiled, data, &arena).unwrap();
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn compile_once_evaluate_many_arena_reuse() {
    let engine = Engine::new();
    let compiled = engine
        .compile(r#"{"if": [{">": [{"var": "score"}, 80]}, "pass", "fail"]}"#)
        .unwrap();

    let mut arena = Bump::new();
    for (input, expected) in [(r#"{"score": 95}"#, "pass"), (r#"{"score": 50}"#, "fail")] {
        let data = DataValue::from_str(input, &arena).unwrap();
        let result = engine.evaluate(&compiled, data, &arena).unwrap();
        assert_eq!(result.as_str(), Some(expected));
        arena.reset();
    }
}

#[test]
fn datavalue_object_returned_as_json_string() {
    let engine = Engine::new();
    let result = engine
        .eval_str(r#"{"merge": [[1, 2], [3, 4]]}"#, "null")
        .unwrap();
    assert_eq!(result, "[1,2,3,4]");
}

#[test]
fn engine_and_session_are_debug_printable() {
    // Engine carries a Box<dyn CustomOperator> map; Session carries a
    // bumpalo::Bump. Both need hand-rolled Debug impls so users can
    // `dbg!()` them without compile errors.
    let engine = Engine::new();
    let rendered = format!("{:?}", engine);
    assert!(rendered.contains("Engine"));
    assert!(rendered.contains("custom_operators"));

    let session = engine.session();
    let rendered = format!("{:?}", session);
    assert!(rendered.contains("Session"));
    assert!(rendered.contains("arena_allocated_bytes"));
}

#[test]
fn logic_clone_evaluates_independently() {
    // `Logic: Clone` lets callers stash an independently-owned copy without
    // wrapping in `Arc`. The clone must produce identical results.
    let engine = Engine::new();
    let original = engine.compile(r#"{"*": [{"var": "x"}, 2]}"#).unwrap();
    let cloned = original.clone();

    let arena = Bump::new();
    let data = DataValue::from_str(r#"{"x": 21}"#, &arena).unwrap();

    let r1 = engine.evaluate(&original, data, &arena).unwrap();
    let r2 = engine.evaluate(&cloned, data, &arena).unwrap();
    assert_eq!(r1.as_i64(), Some(42));
    assert_eq!(r2.as_i64(), Some(42));
}

#[test]
fn logic_to_json_round_trip_evaluates_identically() {
    // The reverse-compilation form should re-compile to a Logic that
    // evaluates to the same result. The string form may not be
    // byte-identical to the input (literals canonicalise, var paths use
    // the canonical "var"/"val" forms), so we compare evaluation outputs.
    let engine = Engine::new();
    let original_rule = r#"{">": [{"var": "score"}, 90]}"#;
    let compiled = engine.compile(original_rule).unwrap();

    let serialised = compiled.to_json();
    assert!(serialised.contains("var"));
    assert!(serialised.contains("score"));

    // Display calls to_json.
    assert_eq!(format!("{}", compiled), serialised);

    let recompiled = engine.compile(&serialised).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str(r#"{"score": 95}"#, &arena).unwrap();
    let r1 = engine.evaluate(&compiled, data, &arena).unwrap();
    let r2 = engine.evaluate(&recompiled, data, &arena).unwrap();
    assert_eq!(r1.as_bool(), r2.as_bool());
    assert_eq!(r1.as_bool(), Some(true));
}

#[test]
fn logic_to_json_handles_constant_folded_subtree() {
    // Constant-folded sub-expressions become literals in the round-trip.
    // The semantics stay identical, even though the string form differs.
    let engine = Engine::new();
    let compiled = engine.compile(r#"{"+": [1, {"+": [2, 3]}]}"#).unwrap();
    let serialised = compiled.to_json();
    let recompiled = engine.compile(&serialised).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str("null", &arena).unwrap();
    let result = engine.evaluate(&recompiled, data, &arena).unwrap();
    assert_eq!(result.as_i64(), Some(6));
}

/// `EngineBuilder::with_constant_folding(false)` keeps every operator in
/// the compiled tree. We can't observe the tree shape directly (it's
/// `pub(crate)`), but `Logic::to_json` reflects it — when folding is on
/// the inner `{"+": [2, 3]}` becomes the literal `5`; when folding is
/// off it stays as the `+` operator.
#[test]
fn with_constant_folding_off_preserves_operators() {
    let folded = Engine::builder().build();
    let folded_logic = folded.compile(r#"{"+": [1, {"+": [2, 3]}]}"#).unwrap();
    let folded_json = folded_logic.to_json();
    // Folded form replaces the inner `{"+": [2, 3]}` with the literal 5.
    assert!(
        !folded_json.contains(r#"{"+": [2"#),
        "expected folded form, got {folded_json}"
    );

    let unfolded = Engine::builder().with_constant_folding(false).build();
    let unfolded_logic = unfolded.compile(r#"{"+": [1, {"+": [2, 3]}]}"#).unwrap();
    let unfolded_json = unfolded_logic.to_json();
    // Unfolded form keeps both `+` operators visible in the round-trip.
    assert!(
        unfolded_json.contains(r#"{"+": [2"#),
        "expected unfolded form to retain inner operator, got {unfolded_json}"
    );

    // Both engines compute the same answer regardless of folding.
    let arena = Bump::new();
    let data = DataValue::from_str("null", &arena).unwrap();
    assert_eq!(
        folded
            .evaluate(&folded_logic, data, &arena)
            .unwrap()
            .as_i64(),
        Some(6),
    );
    let arena2 = Bump::new();
    let data2 = DataValue::from_str("null", &arena2).unwrap();
    assert_eq!(
        unfolded
            .evaluate(&unfolded_logic, data2, &arena2)
            .unwrap()
            .as_i64(),
        Some(6),
    );
}

#[cfg(feature = "serde_json")]
#[test]
fn evaluate_json_value_one_shot() {
    use serde_json::json;
    let engine = Engine::new();
    let logic = json!({"+": [{"var": "a"}, {"var": "b"}]});
    let data = json!({"a": 2, "b": 3});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &data)
        .unwrap();
    assert_eq!(result, json!(5));
}

/// `EvaluationConfig::max_recursion_depth` bails gracefully when a
/// `CustomOperator` re-enters the engine recursively. Without the cap,
/// this would blow the C call stack with no recoverable error; the
/// per-thread depth counter catches it and surfaces a
/// `ConfigurationError` instead.
#[test]
fn max_recursion_depth_catches_custom_operator_reentry() {
    use datalogic_rs::operator::EvalContext;
    use datalogic_rs::{CustomOperator, ErrorKind, EvaluationConfig, Result as DLResult};
    use std::sync::{Arc, OnceLock};

    /// A custom op that re-enters the engine on every call, recursing
    /// indefinitely. Holds an `OnceLock<Arc<Engine>>` so the engine can
    /// be set after `Engine::builder().build()` (chicken-and-egg with
    /// the engine instance owning the operator).
    struct Recurse {
        engine: Arc<OnceLock<Engine>>,
    }

    impl CustomOperator for Recurse {
        fn evaluate<'a>(
            &self,
            _args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            _arena: &'a Bump,
        ) -> DLResult<&'a DataValue<'a>> {
            // Pull the engine out of the OnceLock and re-enter; this is
            // the documented footgun the recursion cap defends against.
            let engine = self.engine.get().expect("engine wired up");
            // We don't care about the inner result — the cap fires
            // before we ever come back, so the inner Result is the
            // ConfigurationError that bubbles up.
            engine.eval_str(r#"{"recurse": []}"#, "null")?;
            // Unreachable in practice (the recurse always errors
            // before returning) but kept to satisfy the type.
            Err(datalogic_rs::Error::custom_message("unreachable"))
        }
    }

    // Build an engine with a tight cap so the test runs fast without
    // approaching real-stack territory. 4 is well below the default
    // 256 but enough to verify the cap fires.
    let engine_lock = Arc::new(OnceLock::new());
    let recurse = Recurse {
        engine: Arc::clone(&engine_lock),
    };

    let config = EvaluationConfig::default().with_max_recursion_depth(4);

    let engine = Engine::builder()
        .with_config(config)
        .add_operator("recurse", recurse)
        .build();

    engine_lock.set(engine).expect("set once");
    let engine = engine_lock.get().unwrap();

    // Top-level call. Each `recurse` invocation increments
    // `DISPATCH_DEPTH` once for the dispatcher and once for the inner
    // re-entered `evaluate_str` → at depth 4 we bail.
    let result = engine.eval_str(r#"{"recurse": []}"#, "null");
    let err = result.expect_err("recursion cap should fire");

    assert!(
        matches!(err.kind, ErrorKind::ConfigurationError(_)),
        "expected ConfigurationError, got: {:?}",
        err.kind,
    );
    assert!(
        err.to_string().contains("max recursion depth exceeded"),
        "error should name the cap; got: {err}",
    );
}

/// Built-in operators always win against a custom registration with the
/// same name. The compile path checks `op_name.parse::<OpCode>()` first
/// (`compile/walker.rs:79`) and routes through the built-in dispatcher
/// before consulting the engine's custom-operator registry — so a
/// `CustomOperator` registered as `"+"` is silently shadowed and never
/// reached at runtime.
///
/// This test pins that contract. If a future change wants custom
/// operators to override built-ins, that's a deliberate behaviour change
/// that would flip this test.
#[test]
fn builtin_shadows_custom_operator_with_same_name() {
    use datalogic_rs::operator::EvalContext;
    use datalogic_rs::{CustomOperator, Result as DLResult};

    /// A custom op named `"+"` that, if reached, would return -1 — chosen
    /// to be impossible from the real `+` operator on any natural-number
    /// input, so we can tell which path actually ran.
    struct AdditiveImposter;
    impl CustomOperator for AdditiveImposter {
        fn evaluate<'a>(
            &self,
            _args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            arena: &'a Bump,
        ) -> DLResult<&'a DataValue<'a>> {
            Ok(arena.alloc(DataValue::from_f64(-1.0)))
        }
    }

    let engine = Engine::builder()
        .add_operator("+", AdditiveImposter)
        .build();
    let result = engine.eval_str(r#"{"+": [1, 2]}"#, "null").unwrap();
    // Built-in `+` ran (3), not the imposter (-1).
    assert_eq!(result, "3");
}
