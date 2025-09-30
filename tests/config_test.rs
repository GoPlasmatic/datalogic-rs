//! Tests for configuration options

use datalogic_rs::{
    DataLogic, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn test_nan_handling_throw_error() {
    // Default behavior - throw error on NaN
    let engine = DataLogic::new();
    let logic = json!({"+": [1, "not_a_number"]});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({});
    let result = engine.evaluate_owned(&compiled, data);
    assert!(result.is_err());
}

#[test]
fn test_nan_handling_ignore_value() {
    // Configure to ignore non-numeric values
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let engine = DataLogic::with_config(config);

    let logic = json!({"+": [1, "not_a_number", 2]});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(3)); // 1 + 2, ignoring "not_a_number"
}

#[test]
fn test_nan_handling_coerce_to_zero() {
    // Configure to treat non-numeric as zero
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let engine = DataLogic::with_config(config);

    let logic = json!({"+": [1, "not_a_number", 2]});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(3)); // 1 + 0 + 2
}

#[test]
fn test_nan_handling_return_null() {
    // Configure to return null on non-numeric
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::ReturnNull);
    let engine = DataLogic::with_config(config);

    let logic = json!({"+": [1, "not_a_number", 2]});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(null));
}

#[test]
fn test_numeric_coercion_default() {
    // Default coercion behavior
    let engine = DataLogic::new();

    // Empty string to 0
    let logic = json!({"+": ["", 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(5)); // 0 + 5

    // Boolean to number
    let logic = json!({"+": [true, false, 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(4)); // 1 + 0 + 3

    // Null to 0
    let logic = json!({"+": [null, 10]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(10)); // 0 + 10
}

#[test]
fn test_numeric_coercion_strict() {
    // Strict numeric coercion
    let config = EvaluationConfig::default()
        .with_nan_handling(NanHandling::IgnoreValue)
        .with_numeric_coercion(NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: false,
            strict_numeric: true,
            undefined_to_zero: false,
        });
    let engine = DataLogic::with_config(config);

    // Empty string not coerced
    let logic = json!({"+": ["", 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(5)); // Ignored "" + 5

    // Boolean not coerced
    let logic = json!({"+": [true, 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(3)); // Ignored true + 3

    // Null not coerced
    let logic = json!({"+": [null, 10]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(10)); // Ignored null + 10
}

#[test]
fn test_loose_equality_errors_default() {
    // Default - throws errors for incompatible types
    let engine = DataLogic::new();
    let logic = json!({"==": [[], 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());
}

#[test]
fn test_loose_equality_errors_disabled() {
    // Disabled - returns false for incompatible types
    let config = EvaluationConfig::default().with_loose_equality_errors(false);
    let engine = DataLogic::with_config(config);

    let logic = json!({"==": [[], 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false)); // Array vs Number = false (no error)
}

#[test]
fn test_safe_arithmetic_preset() {
    // Use safe arithmetic preset
    let engine = DataLogic::with_config(EvaluationConfig::safe_arithmetic());

    // Non-numeric values ignored
    let logic = json!({"+": [1, "not_a_number", 2, [3, 4], 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(8)); // 1 + 2 + 5 (ignoring invalid values)

    // No equality errors
    let logic = json!({"==": [[], "string"]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_strict_preset() {
    // Use strict preset
    let engine = DataLogic::with_config(EvaluationConfig::strict());

    // Strict numeric parsing
    let logic = json!({"+": [true, 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err()); // Boolean not coerced in strict mode

    // Null not coerced to 0
    let logic = json!({"+": [null, 5]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());
}

#[test]
fn test_thread_safety() {
    // Verify compiled logic with config can be shared across threads
    let config = EvaluationConfig::safe_arithmetic();
    let engine = Arc::new(DataLogic::with_config(config));

    let logic = json!({"+": [{"var": "a"}, {"var": "b"}]});
    let compiled = Arc::new(engine.compile(&logic).unwrap());

    let mut handles = vec![];

    for i in 0..4 {
        let engine = Arc::clone(&engine);
        let compiled = Arc::clone(&compiled);

        let handle = std::thread::spawn(move || {
            let data = json!({"a": i * 10, "b": i});
            engine.evaluate_owned(&compiled, data).unwrap()
        });

        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results[0], json!(0)); // 0 + 0
    assert_eq!(results[1], json!(11)); // 10 + 1
    assert_eq!(results[2], json!(22)); // 20 + 2
    assert_eq!(results[3], json!(33)); // 30 + 3
}

#[test]
fn test_runtime_config_change() {
    // Test that compiled logic respects the engine's configuration at evaluation time
    let logic = json!({"+": [1, "not_a_number"]});

    // First engine with default config (throws error)
    let engine1 = DataLogic::new();
    let compiled1 = engine1.compile(&logic).unwrap();
    let result = engine1.evaluate_owned(&compiled1, json!({}));
    assert!(result.is_err());

    // Second engine with config to ignore non-numeric values
    let engine2 = DataLogic::with_config(
        EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue),
    );
    let compiled2 = engine2.compile(&logic).unwrap();
    let result = engine2.evaluate_owned(&compiled2, json!({})).unwrap();
    assert_eq!(result, json!(1)); // "not_a_number" ignored
}

#[test]
fn test_subtraction_with_config() {
    // Test subtraction with different NaN handling
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let engine = DataLogic::with_config(config);

    let logic = json!({"-": [10, "invalid", 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(7)); // 10 - 3 (ignoring "invalid")
}

#[test]
fn test_multiplication_with_config() {
    // Test multiplication with NaN handling
    let config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let engine = DataLogic::with_config(config);

    let logic = json!({"*": [2, "invalid", 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(6)); // 2 * 3 (treating "invalid" as identity 1 for multiplication)
}

#[test]
fn test_comparison_with_config() {
    // Test comparison with configurable coercion
    let config = EvaluationConfig::default().with_numeric_coercion(NumericCoercionConfig {
        empty_string_to_zero: false,
        null_to_zero: false,
        bool_to_number: true,
        strict_numeric: false,
        undefined_to_zero: false,
    });
    let engine = DataLogic::with_config(config);

    // Boolean still coerced to number for comparison
    let logic = json!({">": [true, false]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true)); // 1 > 0

    // Empty string not coerced to 0
    let logic = json!({">": ["", -1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err()); // Can't compare empty string to number
}

#[test]
fn test_truthy_evaluator_javascript() {
    // Test default JavaScript-style truthiness
    let engine = DataLogic::new();

    // Test with different values
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

    // Test empty object - using data from context
    let empty_obj_test = json!({"if": [{"var": "obj"}, "truthy", "falsy"]});
    let compiled = engine.compile(&empty_obj_test).unwrap();
    let result = engine
        .evaluate_owned(&compiled, json!({"obj": {}}))
        .unwrap();
    assert_eq!(
        result,
        json!("falsy"),
        "Empty object should be falsy in JavaScript mode"
    );

    // Test non-empty object
    let result = engine
        .evaluate_owned(&compiled, json!({"obj": {"a": 1}}))
        .unwrap();
    assert_eq!(
        result,
        json!("truthy"),
        "Non-empty object should be truthy in JavaScript mode"
    );

    for (logic, expected) in test_cases {
        let compiled = engine.compile(&logic).unwrap();
        let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_strict_boolean() {
    // Test strict boolean truthiness
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
    let engine = DataLogic::with_config(config);

    // Test with different values - only null and false are falsy
    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")), // 0 is truthy in strict mode
        (json!({"if": ["", "truthy", "falsy"]}), json!("truthy")), // empty string is truthy
        (json!({"if": [[], "truthy", "falsy"]}), json!("truthy")), // empty array is truthy
        (json!({"if": [{}, "truthy", "falsy"]}), json!("truthy")), // empty object is truthy
        (json!({"if": [null, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [false, "truthy", "falsy"]}), json!("falsy")),
        (json!({"if": [1, "truthy", "falsy"]}), json!("truthy")),
        (json!({"if": ["text", "truthy", "falsy"]}), json!("truthy")),
    ];

    for (logic, expected) in test_cases {
        let compiled = engine.compile(&logic).unwrap();
        let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_evaluator_custom() {
    use std::sync::Arc;

    // Test custom truthiness - only even numbers are truthy
    let custom_evaluator = Arc::new(|value: &Value| -> bool {
        if let Some(n) = value.as_i64() {
            n % 2 == 0
        } else {
            false
        }
    });

    let config = EvaluationConfig::default()
        .with_truthy_evaluator(TruthyEvaluator::Custom(custom_evaluator));
    let engine = DataLogic::with_config(config);

    // Test with different values
    let test_cases = vec![
        (json!({"if": [0, "truthy", "falsy"]}), json!("truthy")), // 0 is even
        (json!({"if": [1, "truthy", "falsy"]}), json!("falsy")),  // 1 is odd
        (json!({"if": [2, "truthy", "falsy"]}), json!("truthy")), // 2 is even
        (json!({"if": [3, "truthy", "falsy"]}), json!("falsy")),  // 3 is odd
        (json!({"if": ["text", "truthy", "falsy"]}), json!("falsy")), // not a number
        (json!({"if": [[], "truthy", "falsy"]}), json!("falsy")), // not a number
    ];

    for (logic, expected) in test_cases {
        let compiled = engine.compile(&logic).unwrap();
        let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}

#[test]
fn test_truthy_in_logical_operators() {
    // Test that logical operators also use the configured truthiness
    let config = EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
    let engine = DataLogic::with_config(config);

    // In strict boolean mode, 0 and empty strings are truthy
    let test_cases = vec![
        // AND operator
        (json!({"and": [0, "result"]}), json!("result")), // 0 is truthy, returns last value
        (json!({"and": [false, "result"]}), json!(false)), // false is falsy, returns false
        // OR operator
        (json!({"or": [0, "result"]}), json!(0)), // 0 is truthy, returns 0
        (json!({"or": [false, "result"]}), json!("result")), // false is falsy, returns "result"
        // NOT operator
        (json!({"!": [0]}), json!(false)), // 0 is truthy, NOT makes it false
        (json!({"!": [false]}), json!(true)), // false is falsy, NOT makes it true
        // Double NOT operator
        (json!({"!!": [0]}), json!(true)),      // 0 is truthy
        (json!({"!!": [false]}), json!(false)), // false is falsy
    ];

    for (logic, expected) in test_cases {
        let compiled = engine.compile(&logic).unwrap();
        let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
        assert_eq!(result, expected, "Failed for logic: {:?}", logic);
    }
}
