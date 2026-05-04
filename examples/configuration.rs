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
    let strict_engine = DataLogic::builder().config(strict_config).build();
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
    let custom_engine = DataLogic::builder().config(custom_config).build();
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

    let logic = json!({"!!": [{"var": "value"}]});
    for (value, description) in test_values {
        let data = json!({"value": value});
        let result = engine.evaluate_value(&logic, &data).unwrap();
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
    let logic = json!({"+": ["hello", 1]});
    let data = json!({});

    // Default: throw error
    let default_engine = DataLogic::new();
    let result = default_engine.evaluate_value(&logic, &data);
    println!("Default (ThrowError): {:?}", result);

    // Ignore non-numeric values
    let ignore_config = EvaluationConfig::default().with_nan_handling(NanHandling::IgnoreValue);
    let ignore_engine = DataLogic::builder().config(ignore_config).build();
    let result = ignore_engine.evaluate_value(&logic, &data).unwrap();
    println!("IgnoreValue: {} (non-numeric ignored)", result);

    // Return null
    let null_config = EvaluationConfig::default().with_nan_handling(NanHandling::ReturnNull);
    let null_engine = DataLogic::builder().config(null_config).build();
    let result = null_engine.evaluate_value(&logic, &data).unwrap();
    println!("ReturnNull: {}", result);

    // Coerce to zero
    let coerce_config = EvaluationConfig::default().with_nan_handling(NanHandling::CoerceToZero);
    let coerce_engine = DataLogic::builder().config(coerce_config).build();
    let result = coerce_engine.evaluate_value(&logic, &data).unwrap();
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

    let data = json!({});
    println!("Default (Coercion Enabled):");
    for (logic, desc) in &test_cases {
        let result = default_engine.evaluate_value(logic, &data).unwrap();
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
    let strict_engine = DataLogic::builder().config(strict_config).build();

    println!("\nStrict (No Coercion):");
    for (logic, desc) in &test_cases {
        let result = strict_engine.evaluate_value(logic, &data).unwrap();
        println!("  {} = {}", desc, result);
    }
}
