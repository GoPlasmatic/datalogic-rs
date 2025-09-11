use datalogic_rs::DataLogic;
use serde_json::Value;
use std::borrow::Cow;
use std::fs;
use std::time::Instant;

fn main() {
    // Load test cases from JSON file
    let response =
        fs::read_to_string("tests/suites/compatible.json").expect("Failed to read test cases file");

    let json_data: Vec<Value> =
        serde_json::from_str(&response).expect("Failed to parse test cases");

    // Create engine instance
    let engine = DataLogic::new();

    // Extract and compile test cases
    let mut test_cases = Vec::new();
    for entry in json_data {
        // Skip string entries (comments)
        if entry.is_string() {
            continue;
        }

        if let Value::Object(test_case) = entry {
            // Get rule and data
            if let Some(logic) = test_case.get("rule") {
                let data = test_case.get("data").cloned().unwrap_or(Value::Null);

                // Compile the logic once
                if let Ok(compiled) = engine.compile(Cow::Borrowed(logic)) {
                    test_cases.push((compiled, data));
                }
            }
        }
    }

    let iterations = 100000u32; // Reasonable number of iterations for benchmarking
    println!(
        "Running {} iterations for {} test cases",
        iterations,
        test_cases.len()
    );

    // Warm-up run
    for (compiled_logic, data) in &test_cases {
        let _ = engine.evaluate_owned(compiled_logic, data.clone());
    }

    let start = Instant::now();

    // Run benchmark
    for (compiled_logic, data) in &test_cases {
        for _ in 0..iterations {
            let _ = engine.evaluate_owned(compiled_logic, data.clone());
        }
    }

    let duration = start.elapsed();
    let total_operations = iterations * test_cases.len() as u32;
    let avg_iteration_time = duration / total_operations;

    println!("\n=== Benchmark Results ===");
    println!("Total time: {duration:?}");
    println!("Total operations: {}", total_operations);
    println!("Average operation time: {avg_iteration_time:?}");
    println!(
        "Operations per second: {:.0}",
        total_operations as f64 / duration.as_secs_f64()
    );

    // Calculate throughput
    let throughput_mb =
        (total_operations as f64 * 1000.0) / (1024.0 * 1024.0) / duration.as_secs_f64();
    println!("Estimated throughput: {:.2} MB/s", throughput_mb);
}
