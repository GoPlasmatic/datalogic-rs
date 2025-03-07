use datalogic_rs::arena::DataArena;
use datalogic_rs::logic::{evaluate, parse_json};
use datalogic_rs::value::{DataValue, FromJson};
use std::time::Instant;
use serde_json::json;

fn main() {
    // Number of iterations for benchmarking
    let iterations = 1_000_000;
    
    // Create test data
    let data_json = json!({
        "a": 1,
        "b": 2,
        "c": 3,
        "d": {
            "e": 4,
            "f": 5
        },
        "g": [6, 7, 8],
        "h": null
    });
    
    // Convert to DataValue
    let arena = DataArena::new();
    let data = DataValue::from_json(&data_json, &arena);
    
    // Define test cases
    let test_cases = [
        // Simple missing - all present
        (r#"{"missing": ["a", "b", "c"]}"#, "all present"),
        // Simple missing - some missing
        (r#"{"missing": ["a", "x", "y"]}"#, "some missing"),
        // Simple missing - all missing
        (r#"{"missing": ["x", "y", "z"]}"#, "all missing"),
        // Nested property
        (r#"{"missing": ["d.e", "d.f", "d.z"]}"#, "nested properties"),
        // Array index
        (r#"{"missing": ["g.0", "g.1", "g.5"]}"#, "array indices"),
        // Missing_some - enough present
        (r#"{"missing_some": [2, ["a", "b", "x", "y"]]}"#, "missing_some enough"),
        // Missing_some - not enough present
        (r#"{"missing_some": [3, ["a", "b", "x", "y"]]}"#, "missing_some not enough"),
    ];
    
    println!("Running {} iterations for each test case:", iterations);
    
    // Run benchmarks
    for (rule_str, name) in &test_cases {
        // Parse rule
        let rule_json: serde_json::Value = serde_json::from_str(rule_str).unwrap();
        let rule_token = parse_json(&rule_json, &arena).unwrap();
        
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