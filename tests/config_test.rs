//! Tests for configuration options

#![cfg(feature = "compat")]

use datalogic_rs::{Engine, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator};
use serde_json::{Value, json};
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
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(3)); // 1 + 2, ignoring "not_a_number"
}

#[test]
fn test_nan_handling_coerce_to_zero() {
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let engine = Engine::builder().config(config).build();

    let logic = json!({"+": [1, "not_a_number", 2]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(3)); // 1 + 0 + 2
}

#[test]
fn test_nan_handling_return_null() {
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::ReturnNull);
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
    let config = EvaluationConfig::default()
        .with_nan_handling(NanHandling::IgnoreValue)
        .with_numeric_coercion(NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: false,
            strict_numeric: true,
            undefined_to_zero: false,
        });
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
    let config = EvaluationConfig::default().with_loose_equality_errors(false);
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
        .config(EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue))
        .build();
    let result = engine2.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(1));
}

#[test]
fn test_subtraction_with_config() {
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let engine = Engine::builder().config(config).build();

    let logic = json!({"-": [10, "invalid", 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(7));
}

#[test]
fn test_multiplication_with_config() {
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let engine = Engine::builder().config(config).build();

    let logic = json!({"*": [2, "invalid", 3]});
    let result = engine.evaluate_serde(&logic, &json!({})).unwrap();
    assert_eq!(result, json!(6));
}

#[test]
fn test_comparison_with_config() {
    let config = EvaluationConfig::default().with_numeric_coercion(NumericCoercionConfig {
        empty_string_to_zero: false,
        null_to_zero: false,
        bool_to_number: true,
        strict_numeric: false,
        undefined_to_zero: false,
    });
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
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
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
    let custom_evaluator = Arc::new(|value: &Value| -> bool {
        if let Some(n) = value.as_i64() {
            n % 2 == 0
        } else {
            false
        }
    });

    let config = EvaluationConfig::default()
        .with_truthy_evaluator(TruthyEvaluator::Custom(custom_evaluator));
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
fn test_truthy_in_logical_operators() {
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
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
