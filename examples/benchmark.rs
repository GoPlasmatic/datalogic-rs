use datalogic_rs::*;
use serde_json::Value;
use std::time::Instant;
use std::fs;

fn main() {
    // Load test cases from JSON file
    let response = fs::read_to_string("tests/suites/compatible.json")
        .expect("Failed to read test cases file");
    
    let json_data: Vec<Value> = serde_json::from_str(&response)
        .expect("Failed to parse test cases");

    let arena = DataArena::new();

    // Extract rules and data (just store the JSON values)
    let mut test_cases = Vec::new();
    for entry in json_data {
        // Skip string entries (comments)
        if entry.is_string() {
            continue;
        }
        
        if let Value::Object(test_case) = entry {
            // Get rule and data
            if let Some(logic) = test_case.get("rule") {
                // For simple test cases, data might be missing
                let data = test_case.get("data").unwrap_or(&Value::Null);
                let data_value = DataValue::from_json(data, &arena);
                if let Ok(rule) = logic.into_logic(&arena) {
                    test_cases.push((rule.root().clone(), data_value.clone()));
                }
            }
        }
    }
    
    let iterations = 1e5 as u32; // Reduced iterations to avoid OOM
    println!("Running {} iterations for {} test cases", iterations, test_cases.len());
    let start = Instant::now();

    // Run benchmark
    for (rule, data_value) in &test_cases {
        for _ in 0..iterations {
            let _ = evaluate(rule, data_value, &arena);
        }
    }
    
    let duration = start.elapsed();
    println!("Memory usage: {:?}", arena.memory_usage());

    let avg_iteration_time = duration / (iterations * test_cases.len() as u32);
    
    println!("Total time: {:?}", duration);
    println!("Average iteration time: {:?}", avg_iteration_time);
    println!("Iterations per second: {:.2}", (iterations * test_cases.len() as u32) as f64 / duration.as_secs_f64());
} 