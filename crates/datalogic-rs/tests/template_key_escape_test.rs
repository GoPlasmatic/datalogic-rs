//! Integration tests for [`EngineBuilder::with_template_key_escape`] — the
//! opt-in prefix that lets a template emit an object key which would
//! otherwise be swallowed as an operator invocation.
//!
//! The input/output semantics live in the data-driven suite
//! (`tests/suites/template-key-escape.json`). This file covers what the JSON
//! harness can't express: coverage generated from the engine's own operator
//! table, custom-operator interaction, the `to_json` round-trip invariant,
//! constant folding, multi-byte escape chars, and the feature gates.

#![cfg(all(feature = "templating", feature = "serde_json"))]

use bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};
use serde_json::{Map, Value, json};

/// Engine with templating on and `$` as the escape prefix — the shape most
/// tests here want.
fn escaped_engine() -> Engine {
    Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .build()
}

/// Build the single-key rule `{"<key>": <value>}` programmatically, so keys
/// that are awkward to embed in a JSON string literal (`$+`, `$===`, `$??`)
/// need no quoting dance.
fn single_key_rule(key: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert(key.to_string(), value);
    Value::Object(map)
}

fn eval(engine: &Engine, rule: &Value, data: &Value) -> Value {
    let compiled = engine
        .compile(rule)
        .unwrap_or_else(|e| panic!("compile failed for {rule}: {e}"));
    engine
        .session()
        .eval_into::<Value, _>(&compiled, data)
        .unwrap_or_else(|e| panic!("eval failed for {rule}: {e}"))
}

// ============================================================
// Generated coverage over the engine's own operator table
// ============================================================

/// Every built-in operator name must be reachable as a literal output key
/// once escaped. Driven by `Engine::builtin_operator_names()` rather than a
/// hand-written list, so the test cannot drift as operators are added, and
/// so it covers the punctuation names (`+`, `==`, `===`, `!`, `!!`, `??`,
/// `>=`, …) that are easy to forget.
#[test]
fn every_builtin_operator_name_is_escapable() {
    let engine = escaped_engine();
    let names: Vec<&'static str> = engine.builtin_operator_names().collect();
    // The loop below is meaningful at any feature set — it covers whatever
    // this build compiled in. The size guard only pins that a *full* build
    // really does surface the whole table, so it cannot silently shrink.
    if cfg!(all(
        feature = "datetime",
        feature = "error-handling",
        feature = "ext-array",
        feature = "ext-control",
        feature = "ext-math",
        feature = "ext-object",
        feature = "ext-string",
        feature = "flagd",
    )) {
        assert!(
            names.len() > 50,
            "expected the full builtin table, got {} names",
            names.len()
        );
    } else {
        assert!(!names.is_empty(), "the baseline table is never empty");
    }

    for name in names {
        let rule = single_key_rule(&format!("${name}"), json!(1));
        let expected = single_key_rule(name, json!(1));
        assert_eq!(
            eval(&engine, &rule, &Value::Null),
            expected,
            "escaping `${name}` should emit the literal key `{name}`"
        );
    }
}

/// The same names, doubled, must emit a single literal sigil rather than
/// stripping twice or resolving as an operator.
#[test]
fn every_builtin_operator_name_survives_doubling() {
    let engine = escaped_engine();
    for name in engine.builtin_operator_names() {
        let rule = single_key_rule(&format!("$${name}"), json!(1));
        let expected = single_key_rule(&format!("${name}"), json!(1));
        assert_eq!(
            eval(&engine, &rule, &Value::Null),
            expected,
            "`$${name}` should emit the literal key `${name}`"
        );
    }
}

/// Without the escape configured, those same names stay operators (or fail
/// on arity). This is the backward-compatibility half of the pair above:
/// the escape must be the only thing that changes key resolution.
#[test]
fn builtin_names_are_unchanged_without_the_escape() {
    let engine = Engine::builder().with_templating(true).build();
    for name in engine.builtin_operator_names() {
        let rule = single_key_rule(&format!("${name}"), json!(1));
        let expected = single_key_rule(&format!("${name}"), json!(1));
        assert_eq!(
            eval(&engine, &rule, &Value::Null),
            expected,
            "`${name}` must pass through verbatim when no escape is set"
        );
    }
}

// ============================================================
// Custom operators
// ============================================================

/// Returns the fixed string "custom" so a dispatched call is unmistakable.
struct Tag;
impl CustomOperator for Tag {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(DataValue::String("custom")))
    }
}

#[test]
fn escape_bypasses_a_registered_custom_operator() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .add_operator("tag", Tag)
        .build();

    // Unescaped: the custom operator dispatches.
    assert_eq!(
        eval(&engine, &json!({"tag": [1]}), &Value::Null),
        json!("custom")
    );

    // Escaped: a literal field, so the custom operator is never reached.
    // This is what pins the escape check ahead of the `has_custom_operator`
    // lookup in the templating branch of the compile walker.
    assert_eq!(
        eval(&engine, &json!({"$tag": [1]}), &Value::Null),
        json!({"tag": [1]})
    );
}

/// A custom operator whose *name* starts with the escape char becomes
/// unreachable while the escape is configured. Documented behaviour: the
/// escape wins, and such an operator should be renamed or the engine given
/// a different escape char.
#[test]
fn escape_wins_over_a_sigil_named_custom_operator() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .add_operator("$tag", Tag)
        .build();

    assert_eq!(
        eval(&engine, &json!({"$tag": [1]}), &Value::Null),
        json!({"tag": [1]}),
        "the escape must take precedence over a custom operator named `$tag`"
    );

    // Without the escape the very same engine shape reaches the operator.
    let plain = Engine::builder()
        .with_templating(true)
        .add_operator("$tag", Tag)
        .build();
    assert_eq!(
        eval(&plain, &json!({"$tag": [1]}), &Value::Null),
        json!("custom")
    );
}

// ============================================================
// `to_json` round-trip invariant
// ============================================================

/// `node_serialize` promises that re-parsing `to_json()` yields logic that
/// evaluates identically. Escaped keys must therefore serialise back in
/// their *source* form: emitting a bare `type` would re-parse as the `type`
/// operator. This is the reason the escape is applied at evaluation time
/// rather than being stripped during compilation.
#[test]
fn to_json_preserves_the_escape() {
    let engine = escaped_engine();
    let data = json!({"x": 1});

    for rule in [
        json!({"$type": {"var": "x"}}),
        json!({"$$type": 1}),
        json!({"$type": 1, "plain": 2}),
        json!({"outer": {"$type": {"var": "x"}}}),
        json!({"if": [true, {"$type": 1}, 2]}),
    ] {
        let compiled = engine.compile(&rule).unwrap();
        let round_tripped = compiled.to_json();

        assert!(
            round_tripped.contains("$type"),
            "to_json() dropped the escape for {rule}: {round_tripped}"
        );

        // The real invariant: recompiling the serialised form evaluates the
        // same way.
        let recompiled = engine
            .compile(&round_tripped)
            .unwrap_or_else(|e| panic!("re-compiling {round_tripped} failed: {e}"));
        assert_eq!(
            engine
                .session()
                .eval_into::<Value, _>(&recompiled, &data)
                .unwrap(),
            eval(&engine, &rule, &data),
            "round-trip changed the result for {rule}"
        );
    }
}

/// The serialised key is exactly the source key, not merely "contains a
/// sigil somewhere".
#[test]
fn to_json_emits_the_exact_source_key() {
    let engine = escaped_engine();
    let compiled = engine.compile(&json!({"$type": {"var": "x"}})).unwrap();
    let parsed: Value = serde_json::from_str(&compiled.to_json()).unwrap();
    let keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["$type"]);
}

/// `compile -> to_json -> compile -> to_json` is a fixed point.
#[test]
fn to_json_is_stable_across_recompilation() {
    let engine = escaped_engine();
    let first = engine
        .compile(&json!({"$type": {"var": "x"}, "$map": 2}))
        .unwrap()
        .to_json();
    let second = engine.compile(&first).unwrap().to_json();
    assert_eq!(first, second);
}

// ============================================================
// Constant folding
// ============================================================

/// A fully static escaped object nested in a static parent must not be
/// folded into an object literal: the fold would bake in the *stripped* key
/// and `to_json` would then emit a rule that re-parses as an operator call.
#[test]
fn escaped_objects_are_not_folded_into_literals() {
    let engine = escaped_engine();
    let rule = json!({"if": [true, {"$type": 1}, 2]});

    let compiled = engine.compile(&rule).unwrap();
    let serialised = compiled.to_json();
    assert!(
        serialised.contains("$type"),
        "constant folding erased the escape: {serialised}"
    );

    assert_eq!(
        engine
            .session()
            .eval_into::<Value, _>(&compiled, &Value::Null)
            .unwrap(),
        json!({"type": 1})
    );
}

/// Same rule, folding explicitly disabled: identical result, so the
/// behaviour does not depend on the optimiser being on.
#[test]
fn folding_disabled_gives_the_same_result() {
    let folding_off = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .with_constant_folding(false)
        .build();

    let rule = json!({"if": [true, {"$type": 1}, 2]});
    assert_eq!(
        eval(&folding_off, &rule, &Value::Null),
        eval(&escaped_engine(), &rule, &Value::Null)
    );
}

/// A fully static escaped object at the top level still evaluates through
/// the strip.
#[test]
fn static_top_level_escaped_object() {
    let engine = escaped_engine();
    assert_eq!(
        eval(&engine, &json!({"$type": 1}), &Value::Null),
        json!({"type": 1})
    );
}

// ============================================================
// Multi-byte escape characters
// ============================================================

/// The escape is a `char`, so it may be multi-byte. Guards against a naive
/// `&key[1..]` implementation, which would panic on a char boundary.
#[test]
fn multi_byte_escape_char() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('§')
        .build();

    assert_eq!(
        eval(&engine, &json!({"§type": 1}), &Value::Null),
        json!({"type": 1})
    );
    assert_eq!(
        eval(&engine, &json!({"§§type": 1}), &Value::Null),
        json!({"§type": 1})
    );
    // The escape char alone strips to the empty key.
    assert_eq!(
        eval(&engine, &json!({"§": 1}), &Value::Null),
        json!({"": 1})
    );
    // A different sigil is left alone.
    assert_eq!(
        eval(&engine, &json!({"$type": 1}), &Value::Null),
        json!({"$type": 1})
    );
}

/// An emoji escape char (4 UTF-8 bytes) for good measure.
#[test]
fn four_byte_escape_char() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('🔑')
        .build();

    assert_eq!(
        eval(&engine, &json!({"🔑type": 1}), &Value::Null),
        json!({"type": 1})
    );
    assert_eq!(
        eval(&engine, &json!({"🔑🔑type": 1}), &Value::Null),
        json!({"🔑type": 1})
    );
}

// ============================================================
// Defaults and plumbing
// ============================================================

#[test]
fn default_engine_has_no_escape() {
    // `Engine::new()` and a bare builder must both leave `$` keys alone.
    for engine in [Engine::new(), Engine::builder().build()] {
        // Templating is off on both, so a `$` key is an unknown operator.
        let compiled = engine.compile(&json!({"$type": 1}));
        let is_err = match compiled {
            Err(_) => true,
            Ok(logic) => engine
                .session()
                .eval_into::<Value, _>(&logic, &Value::Null)
                .is_err(),
        };
        assert!(is_err, "`$type` must stay an unknown operator by default");
    }
}

#[test]
fn templating_without_escape_is_unchanged() {
    let engine = Engine::builder().with_templating(true).build();
    assert_eq!(
        eval(&engine, &json!({"$a": 1, "b": 2}), &Value::Null),
        json!({"$a": 1, "b": 2})
    );
    assert_eq!(
        eval(&engine, &json!({"$type": 1}), &Value::Null),
        json!({"$type": 1})
    );
}

#[test]
fn debug_reports_the_escape() {
    let rendered = format!("{:?}", escaped_engine());
    assert!(
        rendered.contains("template_key_escape"),
        "Engine Debug should surface the setting: {rendered}"
    );
    assert!(
        rendered.contains('$'),
        "expected the escape char: {rendered}"
    );
}

/// The escape only means something in templating mode: without it every
/// single-key object is an operator invocation, so there is nothing to
/// escape into and the setting must be inert.
#[test]
fn escape_is_inert_without_templating() {
    let engine = Engine::builder().with_template_key_escape('$').build();

    // Unknown operator, exactly as it would be with no escape configured.
    let compiled = engine.compile(&json!({"$type": 1}));
    let is_err = match compiled {
        Err(_) => true,
        Ok(logic) => engine
            .session()
            .eval_into::<Value, _>(&logic, &Value::Null)
            .is_err(),
    };
    assert!(is_err);

    // Operator dispatch is untouched. Uses a baseline operator rather than a
    // feature-gated one so the check runs in every build.
    assert_eq!(
        eval(&engine, &json!({"+": [{"var": "x"}, 1]}), &json!({"x": 1})),
        json!(2)
    );
}

// ============================================================
// Depth guard
// ============================================================

/// Escaped keys go through a different compile branch than plain operator
/// invocations, so re-check that the nesting guard still fires there rather
/// than overflowing the stack.
#[test]
fn deeply_nested_escaped_objects_hit_the_depth_guard() {
    let engine = escaped_engine();

    let mut deep = json!(1);
    for _ in 0..300 {
        deep = json!({ "$a": deep });
    }
    assert!(
        engine.compile(&deep).is_err(),
        "expected deep nesting of escaped keys to be rejected at compile time"
    );

    let mut shallow = json!(1);
    for _ in 0..10 {
        shallow = json!({ "$a": shallow });
    }
    assert!(engine.compile(&shallow).is_ok());
}

// ============================================================
// Trace
// ============================================================

/// The trace tree shows the rule *as written*, so an escaped key appears in
/// its source form. That is the right thing for a debugger: it mirrors what
/// the author typed rather than the evaluated output key.
#[cfg(feature = "trace")]
#[test]
fn trace_shows_the_source_key() {
    let engine = escaped_engine();
    let run = engine
        .trace()
        .eval_into::<Value, _, _>(r#"{"$type": {"var": "x"}}"#, r#"{"x": 1}"#);

    assert_eq!(run.result.as_ref().unwrap(), &json!({"type": 1}));
    assert!(
        run.expression_tree.expression.contains("$type"),
        "trace should show the source key: {}",
        run.expression_tree.expression
    );
    assert!(!run.steps.is_empty());
}
