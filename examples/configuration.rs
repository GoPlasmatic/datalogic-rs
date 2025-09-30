//! Example demonstrating the configuration system in DataLogic

use datalogic_rs::{
    DataLogic, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
use serde_json::json;
use std::sync::Arc;

fn main() {
    println!("DataLogic Configuration Examples\n");

    // 1. Default behavior (JavaScript-style)
    let default_engine = DataLogic::new();
    demonstrate_truthiness("Default (JavaScript)", &default_engine);

    // 2. Strict boolean truthiness
    let strict_config =
        EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean);
    let strict_engine = DataLogic::with_config(strict_config);
    demonstrate_truthiness("Strict Boolean", &strict_engine);

    // 3. Custom truthiness - only positive numbers are truthy
    let custom_evaluator = Arc::new(|value: &serde_json::Value| -> bool {
        if let Some(n) = value.as_f64() {
            n > 0.0
        } else {
            false
        }
    });
    let custom_config = EvaluationConfig::default()
        .with_truthy_evaluator(TruthyEvaluator::Custom(custom_evaluator));
    let custom_engine = DataLogic::with_config(custom_config);
    demonstrate_truthiness("Custom (Positive Numbers)", &custom_engine);

    // 4. NaN handling examples
    println!("\n=== NaN Handling Examples ===\n");
    demonstrate_nan_handling();

    // 5. Numeric coercion examples
    println!("\n=== Numeric Coercion Examples ===\n");
    demonstrate_numeric_coercion();
}

fn demonstrate_truthiness(name: &str, engine: &DataLogic) {
    println!("=== {} Truthiness ===", name);

    let test_values = vec![
        (json!(0), "0"),
        (json!(-1), "-1"),
        (json!(1), "1"),
        (json!(""), "empty string"),
        (json!("text"), "\"text\""),
        (json!(false), "false"),
        (json!(null), "null"),
        (json!([]), "[]"),
        (json!([1]), "[1]"),
    ];

    for (value, description) in test_values {
        let logic = json!({"!!": [{"var": "value"}]});
        let compiled = engine.compile(&logic).unwrap();
        let data = json!({"value": value});
        let result = engine.evaluate_owned(&compiled, data).unwrap();
        println!(
            "  {} is {}",
            description,
            if result == json!(true) {
                "truthy"
            } else {
                "falsy"
            }
        );
    }
    println!();
}

fn demonstrate_nan_handling() {
    // Default: throw error
    let default_engine = DataLogic::new();
    let logic = json!({"+": ["hello", 1]});
    let compiled = default_engine.compile(&logic).unwrap();
    let result = default_engine.evaluate_owned(&compiled, json!({}));
    println!("Default (ThrowError): {:?}", result);

    // Ignore non-numeric values
    let ignore_config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let ignore_engine = DataLogic::with_config(ignore_config);
    let compiled = ignore_engine.compile(&logic).unwrap();
    let result = ignore_engine.evaluate_owned(&compiled, json!({})).unwrap();
    println!("IgnoreValue: {} (non-numeric ignored)", result);

    // Return null
    let null_config = EvaluationConfig::default().with_nan_handling(NanHandling::ReturnNull);
    let null_engine = DataLogic::with_config(null_config);
    let compiled = null_engine.compile(&logic).unwrap();
    let result = null_engine.evaluate_owned(&compiled, json!({})).unwrap();
    println!("ReturnNull: {}", result);

    // Coerce to zero
    let coerce_config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let coerce_engine = DataLogic::with_config(coerce_config);
    let compiled = coerce_engine.compile(&logic).unwrap();
    let result = coerce_engine.evaluate_owned(&compiled, json!({})).unwrap();
    println!("CoerceToZero: {} (\"hello\" treated as 0)", result);
}

fn demonstrate_numeric_coercion() {
    // Default: coercion enabled
    let default_engine = DataLogic::new();
    let test_cases = vec![
        (json!({"+": ["", 1]}), "\"\" + 1"),
        (json!({"+": [true, 1]}), "true + 1"),
        (json!({"+": [null, 5]}), "null + 5"),
    ];

    println!("Default (Coercion Enabled):");
    for (logic, desc) in &test_cases {
        let compiled = default_engine.compile(logic).unwrap();
        let result = default_engine.evaluate_owned(&compiled, json!({})).unwrap();
        println!("  {} = {}", desc, result);
    }

    // Strict: no coercion
    let strict_config = EvaluationConfig::default()
        .with_numeric_coercion(NumericCoercionConfig {
            empty_string_to_zero: false,
            null_to_zero: false,
            bool_to_number: false,
            strict_numeric: true,
            undefined_to_zero: false,
        })
        .with_nan_handling(NanHandling::ReturnNull); // Return null instead of error
    let strict_engine = DataLogic::with_config(strict_config);

    println!("\nStrict (No Coercion):");
    for (logic, desc) in &test_cases {
        let compiled = strict_engine.compile(logic).unwrap();
        let result = strict_engine.evaluate_owned(&compiled, json!({})).unwrap();
        println!("  {} = {}", desc, result);
    }
}
