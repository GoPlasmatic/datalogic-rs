//! Tests for configuration options

#![cfg(feature = "serde_json")]

use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{
    DivisionByZeroHandling, Engine, EvaluationConfig, NanHandling, NumericCoercionConfig,
    TruthyEvaluator,
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_nan_handling_throw_error() {
    let engine = Engine::new();
    let logic = json!({"+": [1, "not_a_number"]});
    let result = engine.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_nan_handling_ignore_value() {
    let config = EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(3)); // 1 + 2, ignoring "not_a_number"
}

#[test]
fn test_nan_handling_coerce_to_zero() {
    let config =
        EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::CoerceToZero);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(3)); // 1 + 0 + 2
}

#[test]
fn test_nan_handling_return_null() {
    let config = EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::ReturnNull);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn test_numeric_coercion_default() {
    let engine = Engine::new();

    let logic = json!({"+": ["", 5]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(5));

    let logic = json!({"+": [true, false, 3]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(4));

    let logic = json!({"+": [null, 10]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(10));
}

#[test]
fn test_numeric_coercion_strict() {
    let config = EvaluationConfig::default()
        .with_arithmetic_nan_handling(NanHandling::IgnoreValue)
        .with_numeric_coercion(
            NumericCoercionConfig::default()
                .with_empty_string_to_zero(false)
                .with_null_to_zero(false)
                .with_bool_to_number(false)
                .with_reject_non_numeric(true),
        );
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"+": ["", 5]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(5));

    let logic = json!({"+": [true, 3]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(3));

    let logic = json!({"+": [null, 10]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(10));
}

#[test]
fn test_loose_equality_errors_default() {
    let engine = Engine::new();
    let logic = json!({"==": [[], 5]});
    let result = engine.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_loose_equality_errors_disabled() {
    let config = EvaluationConfig::default().with_loose_equality_errors(false);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"==": [[], 5]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_safe_arithmetic_preset() {
    let engine = Engine::builder()
        .with_config(EvaluationConfig::safe_arithmetic())
        .build();

    let logic = json!({"+": [1, "not_a_number", 2, [3, 4], 5]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(8));

    let logic = json!({"==": [[], "string"]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_strict_preset() {
    let engine = Engine::builder()
        .with_config(EvaluationConfig::strict())
        .build();

    let logic = json!({"+": [true, 5]});
    let result = engine.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());

    let logic = json!({"+": [null, 5]});
    let result = engine.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_thread_safety() {
    let config = EvaluationConfig::safe_arithmetic();
    let engine = Arc::new(Engine::builder().with_config(config).build());

    let logic = Arc::new(json!({"+": [{"var": "a"}, {"var": "b"}]}));

    let mut handles = vec![];
    for i in 0..4 {
        let engine = Arc::clone(&engine);
        let logic = Arc::clone(&logic);

        let handle = std::thread::spawn(move || {
            let data = json!({"a": i * 10, "b": i});
            engine
                .eval_into::<serde_json::Value, _, _>(&*logic, &data)
                .unwrap()
        });

        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results[0], json!(0));
    assert_eq!(results[1], json!(11));
    assert_eq!(results[2], json!(22));
    assert_eq!(results[3], json!(33));
}

#[test]
fn test_runtime_config_change() {
    let logic = json!({"+": [1, "not_a_number"]});

    let engine1 = Engine::new();
    let result = engine1.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());

    let engine2 = Engine::builder()
        .with_config(
            EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue),
        )
        .build();
    let result = engine2
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(1));
}

#[test]
fn test_subtraction_with_config() {
    let config = EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"-": [10, "invalid", 3]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(7));
}

#[test]
fn test_multiplication_with_config() {
    let config =
        EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::CoerceToZero);
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({"*": [2, "invalid", 3]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(6));
}

#[test]
fn test_comparison_with_config() {
    let config = EvaluationConfig::default().with_numeric_coercion(
        NumericCoercionConfig::default()
            .with_empty_string_to_zero(false)
            .with_null_to_zero(false),
    );
    let engine = Engine::builder().with_config(config).build();

    let logic = json!({">": [true, false]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!(true));

    let logic = json!({">": ["", -1]});
    let result = engine.eval_into::<serde_json::Value, _, _>(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_truthy_evaluator_javascript() {
    let engine = Engine::new();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": ["", "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [[], "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [{}, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [null, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [false, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [[1], "truthy", "falsy"]}), json!("truthy")),
    ];

    let empty_obj_test = json!({"if": [{"var": "obj"}, "truthy", "falsy"]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&empty_obj_test, &json!({"obj": {}}))
        .unwrap();
    assert_eq!(
        result,
        json!("falsy"),
        "Empty object should be falsy in JavaScript mode"
    );

    let result = engine
        .eval_into::<serde_json::Value, _, _>(&empty_obj_test, &json!({"obj": {"a": 1}}))
        .unwrap();
    assert_eq!(
        result,
        json!("truthy"),
        "Non-empty object should be truthy in JavaScript mode"
    );

    for (logic, expected) in test_cases {
        let result = engine
            .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
            .unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_strict_boolean() {
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
    let engine = Engine::builder().with_config(config).build();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["", "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [[], "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [{}, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [null, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [false, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("truthy")),
    ];

    for (logic, expected) in test_cases {
        let result = engine
            .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
            .unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_custom() {
    let custom_evaluator = Arc::new(|value: &OwnedDataValue| -> bool {
        if let Some(n) = value.as_i64() {
            n % 2 == 0
        } else {
            false
        }
    });

    let config = EvaluationConfig::default()
        .with_truthy_evaluator(TruthyEvaluator::Custom(custom_evaluator));
    let engine = Engine::builder().with_config(config).build();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [2, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [3, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [[], "truthy", "falsy"]}), json!("falsy")),
    ];

    for (logic, expected) in test_cases {
        let result = engine
            .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
            .unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_custom_constructor() {
    // Same predicate as test_truthy_evaluator_custom, built via the
    // ergonomic `TruthyEvaluator::custom` constructor instead of
    // `TruthyEvaluator::Custom(Arc::new(...))`.
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::custom(
        |value: &OwnedDataValue| value.as_i64().map(|n| n % 2 == 0).unwrap_or(false),
    ));
    let engine = Engine::builder().with_config(config).build();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [2, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("falsy")),
    ];

    for (logic, expected) in test_cases {
        let result = engine
            .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
            .unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_in_logical_operators() {
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
    let engine = Engine::builder().with_config(config).build();

    let test_cases = vec![
        (json!({"and": [0, "result"]}), json!("result")),
        (json!({"and": [false, "result"]}), json!(false)),
        (json!({"or": [0, "result"]}), json!(0)),
        (json!({"or": [false, "result"]}), json!("result")),
        (json!({"!": [0]}), json!(false)),
        (json!({"!": [false]}), json!(true)),
        (json!({"!!": [0]}), json!(true)),
        (json!({"!!": [false]}), json!(false)),
    ];

    for (logic, expected) in test_cases {
        let result = engine
            .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
            .unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn debug_format_covers_all_truthy_variants() {
    // Each variant must produce a stable, lossless Debug rendering so
    // EvaluationConfig can derive Debug without choking on the Custom
    // closure variant.
    let variants: Vec<(TruthyEvaluator, &str)> = vec![
        (TruthyEvaluator::JavaScript, "JavaScript"),
        (TruthyEvaluator::Python, "Python"),
        (TruthyEvaluator::StrictBoolean, "StrictBoolean"),
        (
            TruthyEvaluator::Custom(Arc::new(|_: &OwnedDataValue| true)),
            "Custom(<fn>)",
        ),
    ];
    for (variant, expected) in variants {
        assert_eq!(format!("{:?}", variant), expected);
    }
}

#[test]
fn debug_format_works_for_evaluation_config() {
    // Custom truthy evaluator must not block EvaluationConfig::Debug.
    let config = EvaluationConfig::default()
        .with_truthy_evaluator(TruthyEvaluator::Custom(Arc::new(|_| false)));
    let rendered = format!("{:?}", config);
    assert!(rendered.contains("EvaluationConfig"));
    assert!(rendered.contains("Custom(<fn>)"));
}

#[test]
fn test_evaluation_config_fluent_setters() {
    // Chained setters compose the same config that struct-update syntax would.
    let config = EvaluationConfig::default()
        .with_arithmetic_nan_handling(NanHandling::IgnoreValue)
        .with_division_by_zero(DivisionByZeroHandling::ReturnNull)
        .with_loose_equality_errors(false)
        .with_truthy_evaluator(TruthyEvaluator::StrictBoolean)
        .with_numeric_coercion(
            NumericCoercionConfig::default()
                .with_empty_string_to_zero(false)
                .with_null_to_zero(false)
                .with_bool_to_number(false)
                .with_reject_non_numeric(true),
        )
        .with_max_recursion_depth(64);

    assert_eq!(config.arithmetic_nan_handling, NanHandling::IgnoreValue);
    assert_eq!(config.division_by_zero, DivisionByZeroHandling::ReturnNull);
    assert!(!config.loose_equality_errors);
    assert!(matches!(
        config.truthy_evaluator,
        TruthyEvaluator::StrictBoolean
    ));
    assert!(config.numeric_coercion.reject_non_numeric);
    assert_eq!(config.max_recursion_depth, 64);

    // Engine-level smoke check: the chained config drives evaluation as
    // expected — `IgnoreValue` lets arithmetic skip the bad operand.
    let engine = Engine::builder().with_config(config).build();
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&json!({"+": [1, "skip", 2]}), &json!({}))
        .unwrap();
    assert_eq!(result, json!(3));
}

#[test]
fn test_reject_non_numeric_alone_changes_behavior() {
    // With only `reject_non_numeric` set (all other coercion flags at their
    // permissive defaults and NaN handling at the default ThrowError), the
    // fabricated coercions must turn into errors.
    let config = EvaluationConfig::default()
        .with_numeric_coercion(NumericCoercionConfig::default().with_reject_non_numeric(true));
    let engine = Engine::builder().with_config(config).build();

    // Baseline (no flag): "" coerces to 0, so "" + 1 == 1.
    let base = Engine::new();
    assert_eq!(
        base.eval_into::<serde_json::Value, _, _>(&json!({"+": ["", 1]}), &json!({}))
            .unwrap()
            .as_f64(),
        Some(1.0)
    );

    // With the flag alone: empty string / null / bool all become type errors.
    for rule in [
        json!({"+": ["", 1]}),
        json!({"+": [null, 1]}),
        json!({"+": [true, 1]}),
    ] {
        assert!(
            engine
                .eval_into::<serde_json::Value, _, _>(&rule, &json!({}))
                .is_err(),
            "expected {rule} to error under reject_non_numeric"
        );
    }

    // Real numbers and numeric-looking strings still coerce.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"+": ["5", 1]}), &json!({}))
            .unwrap()
            .as_f64(),
        Some(6.0)
    );
}

#[test]
fn test_div_by_zero_fold_and_variadic_honor_config() {
    // Float operands take the configurable div-by-zero path in every arity,
    // not just the 2-arg form.
    let engine = Engine::builder()
        .with_config(
            EvaluationConfig::default().with_division_by_zero(DivisionByZeroHandling::ReturnNull),
        )
        .build();

    // Variadic (3+ args), fractional dividend → float path → ReturnNull.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [10.5, 3.0, 0]}), &json!({}))
            .unwrap(),
        json!(null)
    );
    // 1-arg array fold, fractional dividend → float path → ReturnNull.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [[10.5, 0]]}), &json!({}))
            .unwrap(),
        json!(null)
    );

    // Integer operands always error regardless of config (carve-out); this
    // preserves the JSONLogic conformance behavior for {"/": [8, 2, 0]}.
    assert!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [8, 2, 0]}), &json!({}))
            .is_err()
    );
    assert!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [[10, 0]]}), &json!({}))
            .is_err()
    );
}

#[test]
fn test_div_by_zero_fold_matches_two_arg_for_numeric_string_dividend() {
    // A numeric-*string* dividend coerces to a whole f64 (7.0) but is not an
    // integer `DataValue`. The 2-arg path decides the int/int carve-out on the
    // original value (`as_i64()` is None for a string), so it takes the
    // configurable float path. The fold and variadic forms must agree, rather
    // than treating `7.0.fract() == 0` as an integer and hard-erroring.
    //
    // The dividend is fed through `var` so it survives as a runtime string:
    // a bare `"7"` literal in an arithmetic position is pre-converted to the
    // number 7 at compile time, which would take the int/int carve-out and
    // mask the asymmetry this test guards.
    let engine = Engine::builder()
        .with_config(
            EvaluationConfig::default().with_division_by_zero(DivisionByZeroHandling::ReturnNull),
        )
        .build();

    let str_dividend = json!({"x": "7"});
    let int_dividend = json!({"x": 7});

    // 2-arg reference behavior: string dividend → float path → ReturnNull.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [{"var": "x"}, 0]}), &str_dividend)
            .unwrap(),
        json!(null)
    );
    // 1-arg array fold must match the 2-arg reference.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(
                &json!({"/": [[{"var": "x"}, 0]]}),
                &str_dividend
            )
            .unwrap(),
        json!(null)
    );
    // Variadic (3+ args): string first operand → float path on the first step.
    assert_eq!(
        engine
            .eval_into::<serde_json::Value, _, _>(
                &json!({"/": [{"var": "x"}, 0, 1]}),
                &str_dividend
            )
            .unwrap(),
        json!(null)
    );

    // The genuine int/int carve-out is unchanged: a real integer dividend still
    // errors regardless of config, in both fold and variadic forms.
    assert!(
        engine
            .eval_into::<serde_json::Value, _, _>(&json!({"/": [{"var": "x"}, 0]}), &int_dividend)
            .is_err()
    );
    assert!(
        engine
            .eval_into::<serde_json::Value, _, _>(
                &json!({"/": [[{"var": "x"}, 0]]}),
                &int_dividend
            )
            .is_err()
    );
    assert!(
        engine
            .eval_into::<serde_json::Value, _, _>(
                &json!({"/": [{"var": "x"}, 0, 1]}),
                &int_dividend
            )
            .is_err()
    );
}
