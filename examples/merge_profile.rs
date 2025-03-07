use datalogic_rs::arena::DataArena;
use datalogic_rs::{evaluate, parse_json};
use datalogic_rs::value::DataValue;
use serde_json::json;
use std::time::Instant;

fn main() {
    // Number of iterations for benchmarking
    let iterations = 1_000_000;
    
    // Create test data
    let arena = DataArena::new();
    let data = DataValue::null();
    
    // Define test cases
    let test_cases = [
        // Empty arrays
        (json!({"merge": [[], [], []]}), "empty arrays"),
        // Small arrays
        (json!({"merge": [[1, 2], [3, 4], [5, 6]]}), "small arrays"),
        // Large arrays
        (json!({"merge": [
            create_array(100),
            create_array(100),
            create_array(100),
            create_array(100),
            create_array(100)
        ]}), "large arrays"),
        // Mixed types
        (json!({"merge": [
            [1, 2, 3],
            "single value",
            [4, 5, 6],
            true,
            [7, 8, 9]
        ]}), "mixed types"),
        // Nested arrays
        (json!({"merge": [
            [1, [2, 3]],
            [[4, 5], 6],
            [7, [8, 9]]
        ]}), "nested arrays"),
    ];
    
    println!("Running {} iterations for each test case:", iterations);
    
    // Run benchmarks
    for (rule_json, name) in &test_cases {
        // Parse rule
        let rule_token = parse_json(rule_json, &arena).unwrap();
        
        // Benchmark
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = evaluate(rule_token, &data, &arena).unwrap();
        }
        
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
        let ops_per_sec = 1_000_000_000.0 / ns_per_op;
        
        println!("{}: {:?} ({:.2} ns/op, {:.2} ops/sec)",
                 name, elapsed, ns_per_op, ops_per_sec);
    }
}

// Helper function to create an array of specified size
fn create_array(size: usize) -> serde_json::Value {
    let mut array = Vec::with_capacity(size);
    for i in 0..size {
        array.push(json!(i));
    }
    json!(array)
} 