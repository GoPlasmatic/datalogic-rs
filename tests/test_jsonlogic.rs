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

        // Check if this test expects an error or a result
        let expects_error = test_obj.contains_key("error");
        let expected_error = test_obj.get("error");
        let expected_result = test_obj.get("result");

        if !expects_error && expected_result.is_none() {
            panic!("Test case {} missing 'result' or 'error'", index);
        }

        // Compile and evaluate
        match engine.compile(rule) {
            Ok(compiled) => match engine.evaluate_owned(&compiled, data.clone()) {
                Ok(result) => {
                    if expects_error {
                        println!("✗ Test {}: {}", index, description);
                        println!("  Expected error: {:?}", expected_error);
                        println!("  Got result:     {:?}", result);
                        failed += 1;
                    } else if let Some(expected) = expected_result {
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
                }
                Err(e) => {
                    if expects_error {
                        // Check if the error matches expected error
                        if let Some(expected_error_obj) = expected_error {
                            // Extract the error type from the thrown error
                            if let datalogic_rs::Error::Thrown(thrown_value) = &e {
                                if thrown_value == expected_error_obj {
                                    println!(
                                        "✓ Test {}: {} (error as expected)",
                                        index, description
                                    );
                                    passed += 1;
                                } else {
                                    println!("✗ Test {}: {}", index, description);
                                    println!("  Expected error: {:?}", expected_error_obj);
                                    println!("  Got error:      {:?}", thrown_value);
                                    failed += 1;
                                }
                            } else if let datalogic_rs::Error::InvalidArguments(msg) = &e {
                                // Check if it's an InvalidArguments error
                                let error_obj = serde_json::json!({"type": msg});
                                if &error_obj == expected_error_obj {
                                    println!(
                                        "✓ Test {}: {} (error as expected)",
                                        index, description
                                    );
                                    passed += 1;
                                } else {
                                    println!("✗ Test {}: {}", index, description);
                                    println!("  Expected error: {:?}", expected_error_obj);
                                    println!("  Got error:      {:?}", error_obj);
                                    failed += 1;
                                }
                            } else {
                                println!("✗ Test {}: {}", index, description);
                                println!("  Expected error: {:?}", expected_error_obj);
                                println!("  Got error:      {:?}", e);
                                failed += 1;
                            }
                        } else {
                            println!("✓ Test {}: {} (error as expected)", index, description);
                            passed += 1;
                        }
                    } else {
                        println!(
                            "✗ Test {}: {} - Unexpected evaluation error: {}",
                            index, description, e
                        );
                        failed += 1;
                    }
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
    println!("Results: {} passed, {} failed", passed, failed);
    println!("========================================");

    if failed > 0 {
        panic!("Some tests failed!");
    }
}
