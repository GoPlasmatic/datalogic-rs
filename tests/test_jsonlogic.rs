use datalogic_rs::DataLogic;
use serde_json::{Value, json};

use std::env;
use std::fs;

#[test]
fn test_jsonlogic() {
    // Get test file from environment variable
    let test_file = env::var("JSONLOGIC_TEST_FILE")
        .unwrap_or_else(|_| "tests/suites/compatible.json".to_string());

    println!("Running tests from: {}", test_file);

    // Read and parse test file
    let contents = fs::read_to_string(&test_file)
        .unwrap_or_else(|_| panic!("Failed to read test file: {}", test_file));

    let test_cases: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|_| panic!("Failed to parse JSON from: {}", test_file));

    let test_array = test_cases
        .as_array()
        .expect("Test file should contain an array of test cases");

    let engine = DataLogic::new();
    let mut passed = 0;
    let mut failed = 0;

    for (index, test_case) in test_array.iter().enumerate() {
        // Skip string entries (they're usually section headers)
        if test_case.is_string() {
            println!("\n{}", test_case.as_str().unwrap());
            continue;
        }

        let test_obj = test_case
            .as_object()
            .unwrap_or_else(|| panic!("Test case {} should be an object", index));

        let description = test_obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        let rule = test_obj
            .get("rule")
            .unwrap_or_else(|| panic!("Test case {} missing 'rule'", index));

        let data = test_obj.get("data").cloned().unwrap_or(json!({}));

        let expected = test_obj
            .get("result")
            .unwrap_or_else(|| panic!("Test case {} missing 'result'", index));

        // Compile and evaluate
        match engine.compile(rule) {
            Ok(compiled) => match engine.evaluate_owned(&compiled, data.clone()) {
                Ok(result) => {
                    if &result == expected {
                        println!("✓ Test {}: {}", index, description);
                        passed += 1;
                    } else {
                        println!("✗ Test {}: {}", index, description);
                        println!("  Expected: {:?}", expected);
                        println!("  Got:      {:?}", result);
                        failed += 1;
                    }
                }
                Err(e) => {
                    println!(
                        "✗ Test {}: {} - Evaluation error: {}",
                        index, description, e
                    );
                    failed += 1;
                }
            },
            Err(e) => {
                println!(
                    "✗ Test {}: {} - Compilation error: {}",
                    index, description, e
                );
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!(
        "Results: {} passed, {} failed",
        passed, failed
    );
    println!("========================================");

    if failed > 0 {
        panic!("Some tests failed!");
    }
}
