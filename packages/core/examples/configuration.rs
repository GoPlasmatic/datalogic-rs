//! Example demonstrating the configuration system in Engine.
//!
//! Uses the v5 string-based one-shot API ([`Engine::evaluate_str`]) — no
//! `serde_json::Value` boundary, no `compat` feature required.

use datalogic_rs::{Engine, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator};

fn main() {
    println!("Engine Configuration Examples\n");

    // 1. Default behavior (JavaScript-style truthiness)
    let default_engine = Engine::new();
    demonstrate_truthiness("Default (JavaScript)", &default_engine);

    // 2. Python-style truthiness
    let python_engine = Engine::builder()
        .config(EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::Python))
        .build();
    demonstrate_truthiness("Python", &python_engine);

    // 3. Strict boolean truthiness
    let strict_engine = Engine::builder()
        .config(EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::StrictBoolean))
        .build();
    demonstrate_truthiness("Strict Boolean", &strict_engine);

    // 4. NaN handling examples
    println!("\n=== NaN Handling Examples ===\n");
    demonstrate_nan_handling();

    // 5. Numeric coercion examples
    println!("\n=== Numeric Coercion Examples ===\n");
    demonstrate_numeric_coercion();
}

fn demonstrate_truthiness(name: &str, engine: &Engine) {
    println!("=== {} Truthiness ===", name);

    let test_values = [
        ("0", "0"),
        ("-1", "-1"),
        ("1", "1"),
        (r#""""#, "empty string"),
        (r#""text""#, "\"text\""),
        ("false", "false"),
        ("null", "null"),
        ("[]", "[]"),
        ("[1]", "[1]"),
    ];

    let logic = r#"{"!!": [{"var": "value"}]}"#;
    for (value_json, description) in test_values {
        let data = format!(r#"{{"value": {}}}"#, value_json);
        let result = engine.evaluate_str(logic, &data).unwrap();
        let label = if result == "true" { "truthy" } else { "falsy" };
        println!("  {} is {}", description, label);
    }
    println!();
}

fn demonstrate_nan_handling() {
    let logic = r#"{"+": ["hello", 1]}"#;
    let data = "{}";

    // Default: throw error
    let default_engine = Engine::new();
    let result = default_engine.evaluate_str(logic, data);
    println!("Default (ThrowError): {:?}", result);

    // Ignore non-numeric values
    let ignore_engine = Engine::builder()
        .config(EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue))
        .build();
    let result = ignore_engine.evaluate_str(logic, data).unwrap();
    println!("IgnoreValue: {} (non-numeric ignored)", result);

    // Return null
    let null_engine = Engine::builder()
        .config(EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::ReturnNull))
        .build();
    let result = null_engine.evaluate_str(logic, data).unwrap();
    println!("ReturnNull: {}", result);

    // Coerce to zero
    let coerce_engine = Engine::builder()
        .config(EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::CoerceToZero))
        .build();
    let result = coerce_engine.evaluate_str(logic, data).unwrap();
    println!("CoerceToZero: {} (\"hello\" treated as 0)", result);
}

fn demonstrate_numeric_coercion() {
    let test_cases = [
        (r#"{"+": ["", 1]}"#, "\"\" + 1"),
        (r#"{"+": [true, 1]}"#, "true + 1"),
        (r#"{"+": [null, 5]}"#, "null + 5"),
    ];

    // Default: coercion enabled
    let default_engine = Engine::new();
    println!("Default (Coercion Enabled):");
    for (logic, desc) in &test_cases {
        let result = default_engine.evaluate_str(logic, "{}").unwrap();
        println!("  {} = {}", desc, result);
    }

    // Strict: no coercion (return null on NaN instead of erroring)
    let strict_engine = Engine::builder()
        .config(
            EvaluationConfig::default()
                .with_arithmetic_nan_handling(NanHandling::ReturnNull)
                .with_numeric_coercion(
                    NumericCoercionConfig::default()
                        .with_empty_string_to_zero(false)
                        .with_null_to_zero(false)
                        .with_bool_to_number(false)
                        .with_reject_non_numeric(true),
                ),
        )
        .build();

    println!("\nStrict (No Coercion):");
    for (logic, desc) in &test_cases {
        let result = strict_engine.evaluate_str(logic, "{}").unwrap();
        println!("  {} = {}", desc, result);
    }
}
