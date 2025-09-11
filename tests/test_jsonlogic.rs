use datalogic_rs::DataLogic;
use serde_json::{Value, json};

use std::env;
use std::fs;
use std::path::Path;

#[test]
fn test_jsonlogic() {
    // Get test file from environment variable, or run all tests from index.json
    let test_file = env::var("JSONLOGIC_TEST_FILE");

    let engine = DataLogic::new();
    let mut total_passed = 0;
    let mut total_failed = 0;

    match test_file {
        Ok(file) => {
            // Run single test file
            println!("Running tests from: {}", file);
            let (passed, failed) = run_test_file(&engine, &file);
            total_passed += passed;
            total_failed += failed;
        }
        Err(_) => {
            // Run all tests from index.json
            println!("No JSONLOGIC_TEST_FILE specified, running all tests from index.json\n");

            let index_path = "tests/suites/index.json";
            let index_contents = fs::read_to_string(index_path).expect("Failed to read index.json");

            let index: Vec<String> =
                serde_json::from_str(&index_contents).expect("Failed to parse index.json");

            for test_file in index {
                let test_path = format!("tests/suites/{}", test_file);

                // Check if file exists
                if !Path::new(&test_path).exists() {
                    println!("WARNING: Skipping {} (file not found)\n", test_file);
                    continue;
                }

                println!("\n=== Running tests from: {} ===", test_file);
                let (passed, failed) = run_test_file(&engine, &test_path);
                total_passed += passed;
                total_failed += failed;

                println!("  Results: {} passed, {} failed", passed, failed);
            }
        }
    }

    println!("\n========================================");
    println!(
        "TOTAL RESULTS: {} passed, {} failed",
        total_passed, total_failed
    );
    println!("========================================");

    if total_failed > 0 {
        panic!("Some tests failed!");
    }
}

fn run_test_file(engine: &DataLogic, test_file: &str) -> (usize, usize) {
    // Read and parse test file
    let contents = fs::read_to_string(test_file)
        .unwrap_or_else(|_| panic!("Failed to read test file: {}", test_file));

    let test_cases: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|_| panic!("Failed to parse JSON from: {}", test_file));

    let test_array = test_cases
        .as_array()
        .expect("Test file should contain an array of test cases");

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

    (passed, failed)
}
