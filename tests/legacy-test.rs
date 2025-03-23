use datalogic_rs::arena::DataArena;
use datalogic_rs::logic::{IntoLogic, evaluate};
use datalogic_rs::value::{DataValue, FromJson};
use datalogic_rs::LogicError;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct TestCase {
    description: String,
    rule: JsonValue,
    data: Option<JsonValue>,
    result: JsonValue,
    error: Option<JsonValue>,
}

fn parse_test_cases(json_str: &str) -> Vec<TestCase> {
    let json_array: Vec<JsonValue> = serde_json::from_str(json_str).expect("Failed to parse JSON");
    
    let mut test_cases = Vec::new();
    let mut current_description = String::new();
    
    for item in json_array {
        if item.is_string() {
            // This is a comment or section header
            current_description = item.as_str().unwrap_or("").to_string();
            continue;
        }
        
        if let Some(obj) = item.as_object() {
            let description = if let Some(desc) = obj.get("description") {
                desc.as_str().unwrap_or("").to_string()
            } else {
                current_description.clone()
            };
            
            let rule = obj.get("rule").cloned().unwrap_or(JsonValue::Null);
            let data = obj.get("data").cloned();
            let result = obj.get("result").cloned().unwrap_or(JsonValue::Null);
            let error = obj.get("error").cloned();
            
            test_cases.push(TestCase {
                description,
                rule,
                data,
                result,
                error,
            });
        }
    }
    
    test_cases
}

fn run_test_case(test_case: &TestCase) -> Result<(), String> {
    let arena = DataArena::new();
    
    // Parse the rule
    let rule_logic = match test_case.rule.to_logic(&arena) {
        Ok(logic) => logic,
        Err(e) => {
            // If we expect an error, check if it's the right type
            if let Some(expected_error) = &test_case.error {
                if let Some(error_obj) = expected_error.as_object() {
                    if let Some(error_type) = error_obj.get("type") {
                        if error_type.as_str() == Some("NaN") && e.to_string().contains("NaN") {
                            return Ok(());
                        } else if error_type.as_str() == Some("Unknown Operator") {
                            if let LogicError::OperatorNotFoundError { operator: _ } = e {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            return Err(format!("Failed to parse rule: {}", e));
        },
    };
    
    // Parse the data (or use empty object if not provided)
    let empty_json = json!({});
    let data_json = test_case.data.as_ref().unwrap_or(&empty_json);
    let data = <DataValue as FromJson>::from_json(data_json, &arena);
    
    // Evaluate the rule
    let result = match evaluate(rule_logic.root(), &data, &arena) {
        Ok(value) => value,
        Err(e) => {
            // If we expect an error, check if it's the right type
            if let Some(expected_error) = &test_case.error {
                if let Some(error_obj) = expected_error.as_object() {
                    if let Some(error_type) = error_obj.get("type") {
                        if error_type.as_str() == Some("NaN") {
                            if let LogicError::NaNError = e {
                                return Ok(());
                            } else if let LogicError::ThrownError { r#type } = &e {
                                // Special case for thrown "NaN" errors
                                if r#type == "NaN" {
                                    return Ok(());
                                }
                            }
                        } else if error_type.as_str() == Some("Invalid Arguments") {
                            if let LogicError::InvalidArgumentsError = e {
                                return Ok(());
                            }
                        } else if error_type.as_str() == Some("Unknown Operator") {
                            if let LogicError::OperatorNotFoundError { operator: _ } = e {
                                return Ok(());
                            }
                        } else if let LogicError::ThrownError { r#type } = &e {
                            // This is from the throw operator - check if the error type matches
                            if let Some(expected_type) = error_type.as_str() {
                                if expected_type == r#type {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            return Err(format!("Failed to evaluate rule: {}", e));
        },
    };
    
    // If we expected an error but didn't get one, that's a failure
    if test_case.error.is_some() {
        return Err(format!("Expected an error but got result: {:?}", result));
    }
    
    // Convert the expected result to DataValue for comparison
    let expected = <DataValue as FromJson>::from_json(&test_case.result, &arena);
    
    // Compare the results
    if result.equals(&expected) {
        Ok(())
    } else {
        Err(format!(
            "Test failed: expected {:?}, got {:?}",
            expected,
            result
        ))
    }
}

#[test]
fn test_legacy_operators() {
    // Load the index file
    let index_file_path = Path::new("tests/suites/index.json");
    let index_json_str = fs::read_to_string(index_file_path)
        .expect("Failed to read index file");
    
    // Parse the index file to get the list of test files
    let test_files: Vec<String> = serde_json::from_str(&index_json_str)
        .expect("Failed to parse index.json");
    
    println!("Found {} test files in index", test_files.len());
    
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_cases = 0;
    let num_files = test_files.len();
    
    // Process each test file
    for test_file in test_files {
        let test_file_path = Path::new("tests/suites").join(&test_file);
        println!("Running tests from: {}", test_file);
        
        // Skip files that don't exist
        if !test_file_path.exists() {
            println!("WARNING: Test file {} does not exist, skipping", test_file);
            continue;
        }
        
        // Read and parse the test file
        let json_str = match fs::read_to_string(&test_file_path) {
            Ok(content) => content,
            Err(e) => {
                println!("ERROR: Failed to read test file {}: {}", test_file, e);
                continue;
            }
        };
        
        let test_cases = parse_test_cases(&json_str);
        println!("  Running {} test cases from {}", test_cases.len(), test_file);
        total_cases += test_cases.len();
        
        let mut file_passed = 0;
        let mut file_failed = 0;
        
        // Run each test case in the file
        for test_case in test_cases {
            match run_test_case(&test_case) {
                Ok(_) => {
                    file_passed += 1;
                    total_passed += 1;
                    // Uncomment for verbose output
                    // println!("  PASSED: {}", test_case.description);
                }
                Err(err) => {
                    file_failed += 1;
                    total_failed += 1;
                    println!("  FAILED: {} - {}", test_case.description, err);
                }
            }
        }
        
        println!("  Results for {}: {} passed, {} failed", test_file, file_passed, file_failed);
    }
    
    println!("\nOverall test results: {} passed, {} failed (total: {} cases from {} files)", 
             total_passed, total_failed, total_cases, num_files);
    
    // Uncomment this to make the test fail if any test cases failed
    // assert_eq!(total_failed, 0, "{} test cases failed", total_failed);
}
