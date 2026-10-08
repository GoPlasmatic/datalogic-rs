//! A rule compiled on one engine and evaluated on another follows the
//! evaluating engine's settings, even where compiling folded a constant
//! subexpression. Folding evaluates under the compiling engine's settings
//! (number coercion, NaN and division handling, loose equality,
//! truthiness), so before 5.8.0 a folded `{"/": [1.5, 0]}` kept the
//! compiling engine's answer on an engine configured to return `null`,
//! while the same division computed at runtime returned `null`.

use std::sync::Arc;

use datalogic_rs::{
    DivisionByZeroHandling, Engine, EvaluationConfig, NanHandling, NumericCoercionConfig,
    TruthyEvaluator,
};

fn engine_with(config: EvaluationConfig) -> Engine {
    Engine::builder().with_config(config).build()
}

fn returning_null_on_zero() -> Engine {
    engine_with(
        EvaluationConfig::default().with_division_by_zero(DivisionByZeroHandling::ReturnNull),
    )
}

#[test]
fn a_folded_division_follows_the_evaluating_engine() {
    let compiling = Engine::new();
    let logic = compiling.compile(r#"{"/": [1.5, 0]}"#).unwrap();
    assert!(logic.is_static(), "the division should fold to a constant");

    // The compiling engine saturates.
    assert_eq!(
        compiling.session().eval_str(&logic, "null").unwrap(),
        "1.7976931348623157e308"
    );
    // An engine set to return null returns null, as for the same
    // division computed at runtime.
    let nulling = returning_null_on_zero();
    assert_eq!(nulling.session().eval_str(&logic, "null").unwrap(), "null");
    let dynamic = compiling.compile(r#"{"/": [{"var": "x"}, 0]}"#).unwrap();
    assert_eq!(
        nulling
            .session()
            .eval_str(&dynamic, r#"{"x": 1.5}"#)
            .unwrap(),
        "null"
    );
    // An engine set to throw throws.
    let throwing = engine_with(
        EvaluationConfig::default().with_division_by_zero(DivisionByZeroHandling::ThrowError),
    );
    assert!(throwing.session().eval_str(&logic, "null").is_err());
}

#[test]
fn a_dead_branch_follows_the_evaluating_engines_truthiness() {
    // JavaScript truthiness: 0 is falsy, so compiling drops the "a" branch.
    let compiling = Engine::new();
    let logic = compiling.compile(r#"{"if": [0, "a", "b"]}"#).unwrap();
    assert_eq!(
        compiling.session().eval_str(&logic, "null").unwrap(),
        r#""b""#
    );

    // Strict boolean truthiness: 0 is truthy.
    let strict = engine_with(
        EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean),
    );
    assert_eq!(strict.session().eval_str(&logic, "null").unwrap(), r#""a""#);
}

#[test]
fn a_constant_folded_under_lenient_nan_handling_still_errors_on_a_strict_engine() {
    let lenient = engine_with(
        EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::ReturnNull),
    );
    let logic = lenient.compile(r#"{"+": [1, "x"]}"#).unwrap();
    assert_eq!(lenient.session().eval_str(&logic, "null").unwrap(), "null");

    // The default engine throws on a non-numeric operand.
    let strict = Engine::new();
    assert!(strict.session().eval_str(&logic, "null").is_err());
}

#[test]
fn a_constant_folded_under_lenient_coercion_follows_a_stricter_engine() {
    // By default "" counts as 0.
    let compiling = Engine::new();
    let logic = compiling.compile(r#"{"+": [1, ""]}"#).unwrap();
    assert_eq!(compiling.session().eval_str(&logic, "null").unwrap(), "1");

    let strict =
        engine_with(EvaluationConfig::default().with_numeric_coercion(
            NumericCoercionConfig::default().with_empty_string_to_zero(false),
        ));
    assert!(strict.session().eval_str(&logic, "null").is_err());
}

#[test]
fn a_folded_branch_inside_a_dynamic_rule_follows_the_evaluating_engine() {
    let compiling = Engine::new();
    let logic = compiling
        .compile(r#"{"if": [{"var": "flag"}, {"/": [1.5, 0]}, {"var": "fallback"}]}"#)
        .unwrap();
    let nulling = returning_null_on_zero();
    assert_eq!(
        nulling
            .session()
            .eval_str(&logic, r#"{"flag": true, "fallback": 7}"#)
            .unwrap(),
        "null"
    );
    assert_eq!(
        nulling
            .session()
            .eval_str(&logic, r#"{"flag": false, "fallback": 7}"#)
            .unwrap(),
        "7"
    );
}

#[test]
fn an_engine_with_the_same_settings_runs_the_rule_as_compiled() {
    let compiling = Engine::new();
    let logic = compiling.compile(r#"{"/": [1.5, 0]}"#).unwrap();
    let alike = Engine::new();
    assert!(!logic.compiled_on(&alike));
    assert_eq!(
        alike.session().eval_str(&logic, "null").unwrap(),
        "1.7976931348623157e308"
    );
}

#[test]
fn every_evaluation_and_thread_sees_its_own_engines_answer() {
    let logic = Arc::new(Engine::new().compile(r#"{"/": [1.5, 0]}"#).unwrap());
    let nulling = Arc::new(returning_null_on_zero());
    let saturating = Arc::new(Engine::new());

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let logic = Arc::clone(&logic);
            let engine = if i % 2 == 0 {
                Arc::clone(&nulling)
            } else {
                Arc::clone(&saturating)
            };
            std::thread::spawn(move || {
                let expected = if i % 2 == 0 {
                    "null"
                } else {
                    "1.7976931348623157e308"
                };
                let mut session = engine.session();
                for _ in 0..200 {
                    assert_eq!(session.eval_str(&logic, "null").unwrap(), expected);
                    session.reset();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn several_differently_configured_engines_each_get_their_own_answer() {
    let logic = Engine::new().compile(r#"{"/": [-1.5, 0]}"#).unwrap();
    let cases = [
        (DivisionByZeroHandling::ReturnNull, Some("null")),
        (DivisionByZeroHandling::ThrowError, None),
        (DivisionByZeroHandling::ReturnInfinity, Some("null")),
    ];
    // Twice over, so the second round reads the compiles the first made.
    for _ in 0..2 {
        for (handling, expected) in &cases {
            let engine =
                engine_with(EvaluationConfig::default().with_division_by_zero(handling.clone()));
            let got = engine.session().eval_str(&logic, "null");
            match expected {
                Some(want) => assert_eq!(got.unwrap(), *want, "{handling:?}"),
                None => assert!(got.is_err(), "{handling:?}"),
            }
        }
    }
}

#[cfg(feature = "budget")]
#[test]
fn metered_evaluation_follows_the_evaluating_engine() {
    let logic = Engine::new().compile(r#"{"/": [1.5, 0]}"#).unwrap();
    let nulling = returning_null_on_zero();
    let arena = bumpalo::Bump::new();
    let metered = nulling
        .evaluate_metered(&logic, "null", &arena, 1_000)
        .unwrap();
    assert!(metered.value.is_null());
}

#[cfg(feature = "trace")]
#[test]
fn a_trace_follows_the_evaluating_engine() {
    let logic = Engine::new().compile(r#"{"/": [1.5, 0]}"#).unwrap();
    let nulling = returning_null_on_zero();
    let run = nulling.trace().eval(&logic, "null");
    assert_eq!(
        run.result.unwrap(),
        datalogic_rs::datavalue::OwnedDataValue::Null
    );
}

#[cfg(feature = "templating")]
#[test]
fn a_template_compiled_on_one_engine_is_compiled_again_as_a_template() {
    let compiling = Engine::new();
    let logic = compiling
        .compile_template(r#"{"ratio": {"/": [1.5, 0]}, "name": {"var": "n"}}"#)
        .unwrap();
    let nulling = returning_null_on_zero();
    assert_eq!(
        nulling.session().eval_str(&logic, r#"{"n": "x"}"#).unwrap(),
        r#"{"ratio":null,"name":"x"}"#
    );
}
