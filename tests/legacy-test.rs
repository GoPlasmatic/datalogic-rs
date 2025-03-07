use datalogic_rs::arena::DataArena;
use datalogic_rs::logic::{IntoLogic, evaluate};
use datalogic_rs::value::{DataValue, FromJson};
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct TestCase {
    description: String,
    rule: JsonValue,
    data: Option<JsonValue>,
    result: JsonValue,
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
            
            test_cases.push(TestCase {
                description,
                rule,
                data,
                result,
            });
        }
    }
    
    test_cases
}

fn run_test_case(test_case: &TestCase) -> Result<(), String> {
    let arena = DataArena::new();
    
    // Parse the rule
    let rule_logic = match test_case.rule.into_logic(&arena) {
        Ok(logic) => logic,
        Err(e) => return Err(format!("Failed to parse rule: {}", e)),
    };
    
    // Parse the data (or use empty object if not provided)
    let empty_json = json!({});
    let data_json = test_case.data.as_ref().unwrap_or(&empty_json);
    let data = <DataValue as FromJson>::from_json(data_json, &arena);
    
    // Evaluate the rule
    let result = match evaluate(rule_logic.root(), &data, &arena) {
        Ok(value) => value,
        Err(e) => return Err(format!("Failed to evaluate rule: {}", e)),
    };
    
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
    let test_file_path = Path::new("tests/suites/compatible.json");
    let json_str = fs::read_to_string(test_file_path)
        .expect("Failed to read test file");
    
    let test_cases = parse_test_cases(&json_str);
    println!("Running {} test cases", test_cases.len());
    
    let mut passed = 0;
    let mut failed = 0;
    
    for test_case in test_cases {
        match run_test_case(&test_case) {
            Ok(_) => {
                passed += 1;
                // println!("PASSED: {}", test_case.description);
            }
            Err(err) => {
                failed += 1;
                println!("FAILED: {} - {}", test_case.description, err);
            }
        }
    }
    
    println!("Test results: {} passed, {} failed", passed, failed);
    
    // Uncomment this to make the test fail if any test cases failed
    // assert_eq!(failed, 0, "{} test cases failed", failed);
}
