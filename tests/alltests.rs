use datalogic_rs::{JsonLogic, Rule};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn run_jsonlogic_test(logic: &Value, data: &Value, expected: &Value, error: &Value) -> Result<(), ()> {
    let rule = Rule::from_value(&logic).unwrap();
    
    match JsonLogic::apply(&rule, data) {
        Ok(result) => {
            if result == *expected {
                return Ok(());
            }
        
            match (&result, expected) {
                (Value::Object(got_obj), Value::Object(exp_obj)) => {
                    if got_obj == exp_obj {
                        return Ok(());
                    }
                },
                _ => {}
            }

            println!("Got: {}", result);
            println!("Expected: {}", expected);
            Err(())
        },
        Err(e) => {
            if let Value::Object(error_data) = error {
                if let Some(t) = error_data.get("type") {
                    // Convert error to string and normalize
                    let error_str = e.to_string();
                    let normalized_error = if error_str.starts_with('{') {
                        // Try to parse as JSON
                        if let Ok(Value::Object(map)) = serde_json::from_str(&error_str) {
                            // Extract "type" field if present
                            map.get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&error_str)
                                .to_string()
                        } else {
                            error_str.trim_matches('"').trim().to_string()
                        }
                    } else {
                        error_str.trim_matches('"').trim().to_string()
                    };

                    let expected_str = t.to_string().trim_matches('"').trim().to_string();
                    
                    if normalized_error == expected_str {
                        return Ok(());
                    }
                    println!("Got error: {}", normalized_error);
                    println!("Expected error: {}", expected_str);
                }
            } else {
                println!("Got unexpected error: {}", e);
            }
            Err(())
        }
    }
}

fn run_jsonlogic_test_suite(source: &str) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    // println!("Loading tests from: {}", source);
    let content = fs::read_to_string(source)?;
    
    let json_data: Vec<Value> = serde_json::from_str(&content)?;
    // println!("Parsed {} test cases", json_data.len());

    let mut total_tests = 0;
    let mut passed_tests = 0;
    
    for (index, entry) in json_data.iter().enumerate() {
        // if let Value::String(title) = entry {
        //     let current_section = title.clone();
        //     println!("Testing section: {}", current_section);
        //     continue;
        // } else
        if let Value::Object(test_case) = entry {
            // println!("Running test: {}", test_case.get("description").unwrap());
            let description = test_case.get("description").unwrap();
            let logic = test_case.get("rule").unwrap();
            let data = test_case.get("data").unwrap_or(&Value::Null);
            let expected = test_case.get("result").unwrap_or(&Value::Null);
            let error_type = test_case.get("error").unwrap_or(&Value::Null);
            total_tests += 1;
            match run_jsonlogic_test(logic, data, expected, error_type) {
                Ok(_) => {
                    passed_tests += 1;
                },
                Err(_) => {
                    println!("Test {} failed: {}", index, description);
                }
            }
            
        }
    }
    
    Ok((passed_tests, total_tests))
}

fn extract_filename(url: &str) -> &str {
    url.split('/').last().unwrap_or(url)
}

#[test]
fn test_jsonlogic_all_test_suites() {
    let test_sources = vec![
        "tests/suites/additional.json",
        "tests/suites/arithmetic/divide.extra.json",
        "tests/suites/arithmetic/divide.json",
        "tests/suites/arithmetic/minus.extra.json",
        "tests/suites/arithmetic/minus.json",
        "tests/suites/arithmetic/modulo.extra.json",
        "tests/suites/arithmetic/modulo.json",
        "tests/suites/arithmetic/multiply.extra.json",
        "tests/suites/arithmetic/multiply.json",
        "tests/suites/arithmetic/plus.extra.json",
        "tests/suites/plus.json",
        "tests/suites/chained.json",
        "tests/suites/coalesce.json",
        "tests/suites/comparison/greaterThan.json",
        "tests/suites/comparison/greaterThanEquals.json",
        "tests/suites/comparison/lessThan.json",
        "tests/suites/comparison/lessThanEquals.json",
        "tests/suites/comparison/softEquals.json",
        "tests/suites/comparison/softNotEquals.json",
        "tests/suites/comparison/strictEquals.json",
        "tests/suites/comparison/strictNotEquals.json",
        "tests/suites/compatible.json",
        "tests/suites/control/and.json",
        "tests/suites/control/if.json",
        "tests/suites/control/or.json",
        "tests/suites/exists.json",
        "tests/suites/iterators.extra.json",
        "tests/suites/scopes.json",
        "tests/suites/throw.json",
        "tests/suites/truthiness.json",
        "tests/suites/try.json",
        "tests/suites/val-compat.json",
        "tests/suites/val.json",
    ];



    let mut overall_passed = 0;
    let mut overall_total = 0;

    for source in &test_sources {
        let name = if source.starts_with("http") {
            extract_filename(source)
        } else {
            Path::new(source).file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(source)
        };

        match run_jsonlogic_test_suite(source) {
            Ok((passed, total)) => {
                print!("Results for {} ", name);
                println!("Passed: {}/{} tests {}", passed, total, if passed == total { "✅" } else { "❌" });
                overall_passed += passed;
                overall_total += total;
            },
            Err(e) => println!("Failed to run tests for {} ({}): {}", name, source, e),
        }
    }

    println!("\nOverall Results:");
    println!("Total Passed: {}/{} tests", overall_passed, overall_total);
    
    // Only assert if we actually ran some tests
    assert!(overall_total > 0, "No tests were run!");
    assert!(overall_passed == overall_total, "Some tests failed");
}
