use datalogic_rs::arena::DataArena;
use datalogic_rs::value::DataValue;
use datalogic_rs::parse_str;
use datalogic_rs::evaluate;
use datalogic_rs::value::FromJson;
use datalogic_rs::Token;
use serde_json::json;
use std::time::{Duration, Instant};

fn main() {
    let iterations = 100_000;
    
    // Create test data
    let arena = DataArena::new();
    
    // Test data for reduce operator
    let data_json = json!({
        "numbers": [1, 2, 3, 4, 5],
        "objects": [
            {"value": 1, "name": "one"},
            {"value": 2, "name": "two"},
            {"value": 3, "name": "three"},
            {"value": 4, "name": "four"},
            {"value": 5, "name": "five"},
        ]
    });
    let data = DataValue::from_json(&data_json, &arena);
    
    // Test rules with different initial values
    
    // Sum with initial value 0 (traditional)
    let sum_rule_0 = parse_str(r#"{"reduce":[{"var":"numbers"}, {"+":[{"var":"current"}, {"var":"accumulator"}]}, 0]}"#, &arena).unwrap();
    
    // Sum with initial value 10
    let sum_rule_10 = parse_str(r#"{"reduce":[{"var":"numbers"}, {"+":[{"var":"current"}, {"var":"accumulator"}]}, 10]}"#, &arena).unwrap();
    
    // Product with initial value 1 (traditional)
    let product_rule_1 = parse_str(r#"{"reduce":[{"var":"numbers"}, {"*":[{"var":"current"}, {"var":"accumulator"}]}, 1]}"#, &arena).unwrap();
    
    // Product with initial value 2
    let product_rule_2 = parse_str(r#"{"reduce":[{"var":"numbers"}, {"*":[{"var":"current"}, {"var":"accumulator"}]}, 2]}"#, &arena).unwrap();
    
    // Run and verify results
    println!("Testing reduce operator with different initial values:");
    
    // Sum with initial 0
    let sum_0_result = evaluate(sum_rule_0, &data, &arena).unwrap();
    println!("Sum with initial 0: {:?}", sum_0_result);
    
    // Sum with initial 10
    let sum_10_result = evaluate(sum_rule_10, &data, &arena).unwrap();
    println!("Sum with initial 10: {:?}", sum_10_result);
    
    // Product with initial 1
    let product_1_result = evaluate(product_rule_1, &data, &arena).unwrap();
    println!("Product with initial 1: {:?}", product_1_result);
    
    // Product with initial 2
    let product_2_result = evaluate(product_rule_2, &data, &arena).unwrap();
    println!("Product with initial 2: {:?}", product_2_result);
    
    // Profile each operation
    println!("\nPerformance measurements:");
    profile_operator("Sum with initial 0", &sum_rule_0, &data, &arena, iterations);
    profile_operator("Sum with initial 10", &sum_rule_10, &data, &arena, iterations);
    profile_operator("Product with initial 1", &product_rule_1, &data, &arena, iterations);
    profile_operator("Product with initial 2", &product_rule_2, &data, &arena, iterations);
}

fn profile_operator(name: &str, rule: &Token, data: &DataValue, arena: &DataArena, iterations: usize) {
    // Warm up
    for _ in 0..1000 {
        let _ = evaluate(rule, data, arena).unwrap();
    }
    
    // Measure
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _ = evaluate(rule, data, arena).unwrap();
    }
    
    let duration = start.elapsed();
    
    // Report
    println!(
        "{}: {:?} for {} iterations ({:.2} ns/op, {:.2} ops/sec)",
        name,
        duration,
        iterations,
        duration_to_ns(duration) / iterations as f64,
        iterations as f64 / duration.as_secs_f64(),
    );
}

fn duration_to_ns(duration: Duration) -> f64 {
    duration.as_secs() as f64 * 1_000_000_000.0 + duration.subsec_nanos() as f64
} 