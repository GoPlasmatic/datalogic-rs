use datalogic_rs::arena::DataArena;
use datalogic_rs::value::DataValue;
use datalogic_rs::parse_json;
use datalogic_rs::evaluate;
use datalogic_rs::value::FromJson;
use datalogic_rs::Token;
use serde_json::json;
use std::time::{Duration, Instant};

fn main() {
    let iterations = 1_000_000;
    
    // Create test data
    let arena = DataArena::new();
    
    // Test data for array operators
    let data_json = json!({
        "numbers": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        "objects": [
            {"value": 1, "name": "one"},
            {"value": 2, "name": "two"},
            {"value": 3, "name": "three"},
            {"value": 4, "name": "four"},
            {"value": 5, "name": "five"},
        ],
        "mixed": [1, "two", 3.5, true, false, null, {"key": "value"}]
    });
    let data = DataValue::from_json(&data_json, &arena);
    
    // Test rules
    let all_rule_json: serde_json::Value = serde_json::from_str(r#"{"all":[{"var":"numbers"}, {">=":[{"var":""}, 1]}]}"#).unwrap();
    let all_rule = parse_json(&all_rule_json, &arena).unwrap();
    
    let some_rule_json: serde_json::Value = serde_json::from_str(r#"{"some":[{"var":"numbers"}, {">":[{"var":""}, 5]}]}"#).unwrap();
    let some_rule = parse_json(&some_rule_json, &arena).unwrap();
    
    let none_rule_json: serde_json::Value = serde_json::from_str(r#"{"none":[{"var":"numbers"}, {">":[{"var":""}, 10]}]}"#).unwrap();
    let none_rule = parse_json(&none_rule_json, &arena).unwrap();
    
    // Profile each operator
    profile_operator("all", &all_rule, &data, &arena, iterations);
    profile_operator("some", &some_rule, &data, &arena, iterations);
    profile_operator("none", &none_rule, &data, &arena, iterations);
    
    // More complex cases
    let complex_all_json: serde_json::Value = serde_json::from_str(r#"{"all":[{"var":"objects"}, {">":[{"var":"value"}, 0]}]}"#).unwrap();
    let complex_all = parse_json(&complex_all_json, &arena).unwrap();
    
    let complex_some_json: serde_json::Value = serde_json::from_str(r#"{"some":[{"var":"objects"}, {"==":[{"var":"name"}, "three"]}]}"#).unwrap();
    let complex_some = parse_json(&complex_some_json, &arena).unwrap();
    
    let complex_none_json: serde_json::Value = serde_json::from_str(r#"{"none":[{"var":"objects"}, {">":[{"var":"value"}, 10]}]}"#).unwrap();
    let complex_none = parse_json(&complex_none_json, &arena).unwrap();
    
    profile_operator("complex all", &complex_all, &data, &arena, iterations);
    profile_operator("complex some", &complex_some, &data, &arena, iterations);
    profile_operator("complex none", &complex_none, &data, &arena, iterations);

    // Test cases for merge operator
    let test_cases = [
        ("empty arrays", json!({"merge": [[], [], []]})),
        ("small arrays", json!({"merge": [[1, 2], [3, 4], [5, 6]]})),
        ("large arrays", json!({"merge": [
            array(100),
            array(100),
            array(100),
            array(100),
            array(100)
        ]})),
        ("mixed types", json!({"merge": [
            [1, 2, 3],
            "single value",
            [4, 5, 6],
            true,
            [7, 8, 9]
        ]})),
        ("nested arrays", json!({"merge": [
            [1, [2, 3]],
            [[4, 5], 6],
            [7, [8, 9]]
        ]})),
    ];
    
    // Parse the test cases
    let mut parsed_cases = Vec::new();
    for (name, rule) in &test_cases {
        let token = parse_json(rule, &arena).unwrap();
        parsed_cases.push((*name, token));
    }
    
    // Create data for testing
    let data = DataValue::null();
    
    println!("Running {} iterations for each test case", iterations);
    
    // Run benchmarks
    for (name, token) in parsed_cases {
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = evaluate(token, &data, &arena);
        }
        
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
        let ops_per_sec = 1_000_000_000.0 / ns_per_op;
        
        println!(
            "{}: {:.2} ms ({:.2} ns/op, {:.2} ops/sec)",
            name,
            elapsed.as_millis(),
            ns_per_op,
            ops_per_sec
        );
    }
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

// Helper function to create an array of specified size
fn array(size: usize) -> serde_json::Value {
    let mut array = Vec::with_capacity(size);
    for i in 0..size {
        array.push(json!(i));
    }
    json!(array)
} 