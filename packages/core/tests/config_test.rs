//! Tests for configuration options

#![cfg(feature = "compat")]

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
    let result = engine.evaluate_serde(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_nan_handling_ignore_value() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::IgnoreValue,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(3)); // 1 + 2, ignoring "not_a_number"
}

#[test]
fn test_nan_handling_coerce_to_zero() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::CoerceToZero,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(3)); // 1 + 0 + 2
}

#[test]
fn test_nan_handling_return_null() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::ReturnNull,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn test_numeric_coercion_default() {
    let engine = Engine::new();

    let logic = json!({"+": ["", 5]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(5));

    let logic = json!({"+": [true, false, 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(4));

    let logic = json!({"+": [null, 10]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(10));
}

#[test]
fn test_numeric_coercion_strict() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::IgnoreValue,
        numeric_coercion: NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: false,
            strict_numeric: true,
            undefined_to_zero: false,
        },
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": ["", 5]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(5));

    let logic = json!({"+": [true, 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(3));

    let logic = json!({"+": [null, 10]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(10));
}

#[test]
fn test_loose_equality_errors_default() {
    let engine = Engine::new();
    let logic = json!({"==": [[], 5]});
    let result = engine.evaluate_serde(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_loose_equality_errors_disabled() {
    let config = EvaluationConfig {
        loose_equality_errors: false,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"==": [[], 5]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_safe_arithmetic_preset() {
    let engine = Engine::builder()
        .config(EvaluationConfig::safe_arithmetic())
        .build();

    let logic = json!({"+": [1, "not_a_number", 2, [3, 4], 5]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(8));

    let logic = json!({"==": [[], "string"]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_strict_preset() {
    let engine = Engine::builder().config(EvaluationConfig::strict()).build();

    let logic = json!({"+": [true, 5]});
    let result = engine.evaluate_serde(&logic, &json!({}));
    assert!(result.is_err());

    let logic = json!({"+": [null, 5]});
    let result = engine.evaluate_serde(&logic, &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_thread_safety() {
    let config = EvaluationConfig::safe_arithmetic();
    let engine = Arc::new(Engine::builder().config(config).build());

    let logic = Arc::new(json!({"+": [{"var": "a"}, {"var": "b"}]}));

    let mut handles = vec![];
    for i in 0..4 {
        let engine = Arc::clone(&engine);
        let logic = Arc::clone(&logic);

        let handle = std::thread::spawn(move || {
            let data = json!({"a": i * 10, "b": i});
            engine.evaluate_serde(&logic, &data).unwrap()
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
    let result = engine1.evaluate_serde(&logic, &json!({}));
    assert!(result.is_err());

    let engine2 = Engine::builder()
        .config(EvaluationConfig {
            arithmetic_nan_handling: NanHandling::IgnoreValue,
            ..Default::default()
        })
        .build();
    let result = engine2.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(1));
}

#[test]
fn test_subtraction_with_config() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::IgnoreValue,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"-": [10, "invalid", 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(7));
}

#[test]
fn test_multiplication_with_config() {
    let config = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::CoerceToZero,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({"*": [2, "invalid", 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(6));
}

#[test]
fn test_comparison_with_config() {
    let config = EvaluationConfig {
        numeric_coercion: NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: true,
            strict_numeric: false,
            undefined_to_zero: false,
        },
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let logic = json!({">": [true, false]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(true));

    let logic = json!({">": ["", -1]});
    let result = engine.evaluate_serde(&logic, &json!({}));
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
        .evaluate_serde(&empty_obj_test, &json!({"obj": {}}))
        .unwrap();
    assert_eq!(
        result,
        json!("falsy"),
        "Empty object should be falsy in JavaScript mode"
    );

    let result = engine
        .evaluate_serde(&empty_obj_test, &json!({"obj": {"a": 1}}))
        .unwrap();
    assert_eq!(
        result,
        json!("truthy"),
        "Non-empty object should be truthy in JavaScript mode"
    );

    for (logic, expected) in test_cases {
        let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_strict_boolean() {
    let config = EvaluationConfig {
        truthy_evaluator: TruthyEvaluator::StrictBoolean,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

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
        let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
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

    let config = EvaluationConfig {
        truthy_evaluator: TruthyEvaluator::Custom(custom_evaluator),
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [2, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [3, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [[], "truthy", "falsy"]}), json!("falsy")),
    ];

    for (logic, expected) in test_cases {
        let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_custom_constructor() {
    // Same predicate as test_truthy_evaluator_custom, built via the
    // ergonomic `TruthyEvaluator::custom` constructor instead of
    // `TruthyEvaluator::Custom(Arc::new(...))`.
    let config = EvaluationConfig {
        truthy_evaluator: TruthyEvaluator::custom(|value: &OwnedDataValue| {
            value.as_i64().map(|n| n % 2 == 0).unwrap_or(false)
        }),
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [2, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("falsy")),
    ];

    for (logic, expected) in test_cases {
        let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_in_logical_operators() {
    let config = EvaluationConfig {
        truthy_evaluator: TruthyEvaluator::StrictBoolean,
        ..Default::default()
    };
    let engine = Engine::builder().config(config).build();

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
        let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
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
    let config = EvaluationConfig {
        truthy_evaluator: TruthyEvaluator::Custom(Arc::new(|_| false)),
        ..Default::default()
    };
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
        .with_numeric_coercion(NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: false,
            strict_numeric: true,
            undefined_to_zero: false,
        })
        .with_max_recursion_depth(64);

    assert_eq!(config.arithmetic_nan_handling, NanHandling::IgnoreValue);
    assert_eq!(config.division_by_zero, DivisionByZeroHandling::ReturnNull);
    assert!(!config.loose_equality_errors);
    assert!(matches!(
        config.truthy_evaluator,
        TruthyEvaluator::StrictBoolean
    ));
    assert!(config.numeric_coercion.strict_numeric);
    assert_eq!(config.max_recursion_depth, 64);

    // Engine-level smoke check: the chained config drives evaluation as
    // expected — `IgnoreValue` lets arithmetic skip the bad operand.
    let engine = Engine::builder().config(config).build();
    let result = engine
        .evaluate_serde(&json!({"+": [1, "skip", 2]}), &json!({}))
        .unwrap();
    assert_eq!(result, json!(3));
}
