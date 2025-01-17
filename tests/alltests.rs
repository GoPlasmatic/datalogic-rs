use datalogic_rs::{JsonLogic, Rule};
use serde_json::Value;
use reqwest::blocking::get;
use std::fs;
use std::path::Path;

fn load_test_cases(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    if source.starts_with("http") {
        Ok(get(source)?.text()?)
    } else {
        Ok(fs::read_to_string(source)?)
    }
}

fn run_jsonlogic_test(logic: &Value, data: &Value, expected: &Value) -> Result<(), ()> {
    let rule = Rule::from_value(&logic).unwrap();

    match JsonLogic::apply(&rule, data) {
        Ok(result) => {
            if result == *expected {
                Ok(())
            } else {
                println!("Rule: {}", logic);
                println!("Data: {}", data);
                println!("Expected: {}", expected);
                println!("Got: {}", result);
                Err(())
            }
        },
        Err(e) => {
            println!("Error: {}", e);
            println!("Rule: {}", logic);
            println!("Data: {}", data);
            println!("Expected: {}", expected);
            Err(())
        }
    }
}

fn run_jsonlogic_test_suite(source: &str) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    println!("\nLoading tests from: {}", source);
    
    let content = load_test_cases(source)?;
    println!("Received content of length: {}", content.len());
    
    let json_data: Vec<Value> = serde_json::from_str(&content)?;
    println!("Parsed {} test cases", json_data.len());

    let mut current_section = String::new();
    let mut total_tests = 0;
    let mut passed_tests = 0;
    
    for (index, entry) in json_data.iter().enumerate() {
        if let Value::String(title) = entry {
            current_section = title.clone();
            println!("Testing section: {}", current_section);
            continue;
        } else if let Value::Array(test_case) = entry {
            if test_case.len() != 3 {
                println!("Skipping malformed test case {}: {:?}", index, test_case);
                continue;
            }

            total_tests += 1;
            let logic = &test_case[0];
            let data = &test_case[1];
            let expected = &test_case[2];
            match run_jsonlogic_test(logic, data, expected) {
                Ok(_) => {
                    passed_tests += 1;
                },
                Err(_) => {
                    println!("Test {} failed", index);
                }
            }
        } else if let Value::Object(test_case) = entry {
            let description = test_case.get("description").unwrap();
            let logic = test_case.get("rule").unwrap();
            let data = test_case.get("data").unwrap();
            let expected = test_case.get("result").unwrap();
            total_tests += 1;
            println!("Testing: {}", description);
            match run_jsonlogic_test(logic, data, expected) {
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
        // Remote URLs
        "https://jsonlogic.com/tests.json",
        "tests/leaf-coercion-proposal.json",
        "tests/add-operator.json",
        "tests/lessthan-operator.json"
        // Local file
        // "tests/custom_tests.json",  // Add your local test file path here
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

        println!("\nTesting suite: {}", name);
        
        match run_jsonlogic_test_suite(source) {
            Ok((passed, total)) => {
                println!("\nResults for {} ({})", name, source);
                println!("Passed: {}/{} tests", passed, total);
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
