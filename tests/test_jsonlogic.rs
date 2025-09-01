use datalogic_rs::{DataLogic, LogicError};
use serde_json::{Value as JsonValue, json};
use std::env;
use std::fs;
use std::path::Path;

type TestResult<T> = Result<T, String>;

#[derive(Debug)]
struct TestCase {
    description: String,
    rule: JsonValue,
    data: Option<JsonValue>,
    result: Option<JsonValue>,
    error: Option<JsonValue>,
    preserve_structure: Option<bool>,
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
            let result = obj.get("result").cloned();
            let error = obj.get("error").cloned();
            let preserve_structure = obj
                .get("preserve_structure")
                .map(|v| v.as_bool().unwrap_or(false));

            test_cases.push(TestCase {
                description,
                rule,
                data,
                result,
                error,
                preserve_structure,
            });
        }
    }

    test_cases
}

fn run_test_case(test_case: &TestCase) -> TestResult<()> {
    // Create a DataLogic instance which manages the arena and parsers
    let dl = if test_case.preserve_structure == Some(true) {
        DataLogic::with_preserve_structure()
    } else {
        DataLogic::new()
    };

    // Parse the rule using DataLogic's parse_logic method
    let rule_str = test_case.rule.to_string();

    let rule_logic = match dl.parse_logic(&rule_str) {
        Ok(logic) => logic,
        Err(e) => {
            // If we expect an error, check if it's the right type
            if let Some(expected_error) = &test_case.error
                && let Some(error_obj) = expected_error.as_object()
                && let Some(error_type) = error_obj.get("type")
            {
                if error_type.as_str() == Some("NaN") && e.to_string().contains("NaN") {
                    return Ok(());
                } else if error_type.as_str() == Some("Unknown Operator")
                    && let LogicError::OperatorNotFoundError { operator: _ } = e
                {
                    return Ok(());
                }
            }
            return Err(format!("Failed to parse rule: {e}"));
        }
    };

    // Parse the data (or use empty object if not provided)
    let empty_json = json!({});
    let data_json = test_case.data.as_ref().unwrap_or(&empty_json);

    // Use DataLogic to parse the data
    let data = match dl.parse_data(&data_json.to_string()) {
        Ok(data) => data,
        Err(e) => return Err(format!("Failed to parse data: {e}")),
    };

    // Evaluate the rule using DataLogic's evaluate method
    let result = match dl.evaluate(&rule_logic, data) {
        Ok(value) => value,
        Err(e) => {
            // If we expect an error, check if it's the right type
            if let Some(expected_error) = &test_case.error
                && let Some(error_obj) = expected_error.as_object()
                && let Some(error_type) = error_obj.get("type")
            {
                if error_type.as_str() == Some("NaN") {
                    if let LogicError::NaNError = e {
                        return Ok(());
                    } else if let LogicError::ThrownError { r#type } = &e
                        && r#type == "NaN"
                    {
                        return Ok(());
                    }
                } else if error_type.as_str() == Some("Invalid Arguments") {
                    if let LogicError::InvalidArgumentsError = e {
                        return Ok(());
                    }
                } else if error_type.as_str() == Some("Unknown Operator") {
                    if let LogicError::OperatorNotFoundError { operator: _ } = e {
                        return Ok(());
                    }
                } else if let LogicError::ThrownError { r#type } = &e
                    && let Some(expected_type) = error_type.as_str()
                    && expected_type == r#type
                {
                    return Ok(());
                }
            }
            return Err(format!("Failed to evaluate rule: {e}"));
        }
    };

    // If we expected an error but didn't get one, that's a failure
    if test_case.error.is_some() {
        return Err(format!("Expected an error but got result: {result}"));
    }

    // If a specific result is expected
    if let Some(expected_result) = &test_case.result {
        // Convert the expected result to DataValue for comparison
        let expected = match dl.parse_data(&expected_result.to_string()) {
            Ok(value) => value,
            Err(e) => return Err(format!("Failed to parse expected result: {e}")),
        };

        // Compare the results
        if result.equals(expected) {
            Ok(())
        } else {
            Err(format!("Test failed: expected {expected}, got {result}"))
        }
    } else {
        // No specific result expected
        Ok(())
    }
}

fn run_test_suite(test_file_path: &Path) -> (usize, usize) {
    println!("Running tests from: {}", test_file_path.display());

    // Read and parse the test file
    let json_str = match fs::read_to_string(test_file_path) {
        Ok(content) => content,
        Err(e) => {
            println!(
                "ERROR: Failed to read test file {}: {}",
                test_file_path.display(),
                e
            );
            return (0, 0);
        }
    };

    let test_cases = parse_test_cases(&json_str);
    println!("  Running {} test cases", test_cases.len());

    let mut passed = 0;
    let mut failed = 0;
    let empty_json = json!({});

    for (i, test_case) in test_cases.iter().enumerate() {
        match run_test_case(test_case) {
            Ok(_) => {
                passed += 1;
                println!("  ✅ {}: {}", i + 1, test_case.description);
            }
            Err(e) => {
                failed += 1;
                println!("  ❌ {}: {}", i + 1, test_case.description);
                println!("     Error: {e}");
                println!("     Rule: {}", test_case.rule);
                println!(
                    "     Data: {}",
                    test_case.data.as_ref().unwrap_or(&empty_json)
                );
            }
        }
    }

    println!("  Results: {passed} passed, {failed} failed");

    (passed, failed)
}

// Replace the main function with test functions
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_test_files() -> Vec<PathBuf> {
        // Check if a specific test file is specified via environment variable
        if let Ok(test_file) = env::var("JSONLOGIC_TEST_FILE") {
            return vec![PathBuf::from(test_file)];
        }

        // Check if we should run a specific test suite from command line arguments
        let args: Vec<String> = env::args().collect();
        if args.len() > 1 {
            // If test file path is provided as a command-line argument
            let arg = &args[1];
            if Path::new(arg).exists() {
                return vec![PathBuf::from(arg)];
            }
        }

        // Default: Run all test files from the index
        let index_file = PathBuf::from("tests/suites/index.json");
        if index_file.exists()
            && let Ok(content) = fs::read_to_string(&index_file)
        {
            let json: JsonValue = serde_json::from_str(&content).unwrap_or_else(|_| json!([]));
            if let Some(arr) = json.as_array() {
                return arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| PathBuf::from(format!("tests/suites/{s}")))
                    .collect();
            }
        }

        // Fallback: Just run the compatible.json tests
        vec![PathBuf::from("tests/suites/compatible.json")]
    }

    #[test]
    fn test_jsonlogic() {
        let test_files = get_test_files();

        let mut total_passed = 0;
        let mut total_failed = 0;

        for test_file in test_files {
            let (passed, failed) = run_test_suite(&test_file);
            total_passed += passed;
            total_failed += failed;
        }

        println!("\nTotal Results: {total_passed} passed, {total_failed} failed");

        assert_eq!(total_failed, 0, "{total_failed} tests failed");
    }
}
