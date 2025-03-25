use datalogic_rs::*;
use serde_json::Value;
use std::fs;
use std::time::Instant;

fn main() {
    // Load test cases from JSON file
    let response =
        fs::read_to_string("tests/suites/compatible.json").expect("Failed to read test cases file");

    let json_data: Vec<Value> =
        serde_json::from_str(&response).expect("Failed to parse test cases");

    // Instance for parsing
    let parse_logic = DataLogic::new();

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
                let data = test_case.get("data").unwrap_or(&Value::Null);
                let data_value = parse_logic.parse_data(data.to_string().as_str()).unwrap();

                // Use JsonLogicParser to parse the rule
                let rule_json_str = logic.to_string();
                if let Ok(rule) = parse_logic.parse_logic(&rule_json_str, None) {
                    test_cases.push((rule.clone(), data_value.clone()));
                }
            }
        }
    }

    let iterations = 1e5 as u32; // Reduced iterations to avoid OOM
    println!(
        "Running {} iterations for {} test cases",
        iterations,
        test_cases.len()
    );
    let start = Instant::now();

    // Separate instance for evaluation
    let mut eval_logic = DataLogic::new();

    // Run benchmark
    for (rule, data_value) in &test_cases {
        for _ in 0..iterations {
            let _ = eval_logic.evaluate(rule, data_value);
        }
        eval_logic.reset_arena();
    }

    let duration = start.elapsed();
    println!("Memory usage: {:?}", eval_logic.arena().memory_usage());

    let avg_iteration_time = duration / (iterations * test_cases.len() as u32);

    println!("Total time: {:?}", duration);
    println!("Average iteration time: {:?}", avg_iteration_time);
    println!(
        "Iterations per second: {:.2}",
        (iterations * test_cases.len() as u32) as f64 / duration.as_secs_f64()
    );
}
