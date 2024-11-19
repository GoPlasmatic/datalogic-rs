use datalogic_rs::JsonLogic;
use serde_json::Value;
use reqwest::blocking::get;

#[test]
fn test_jsonlogic_official_test_suite() {
    let response = get("http://jsonlogic.com/tests.json")
        .expect("Failed to fetch test cases")
        .text()
        .expect("Failed to read response");

    let json_data: Vec<Value> = serde_json::from_str(&response)
        .expect("Failed to parse test cases");

    let logic = JsonLogic::new();
    let mut current_section = String::new();
    
    for (index, entry) in json_data.iter().enumerate() {
        // Update section title if entry is a string
        if let Value::String(title) = entry {
            current_section = title.clone();
            println!("\nTesting section: {}", current_section);
            continue;
        }

        // Process test case array
        if let Value::Array(test_case) = entry {
            if test_case.len() != 3 {
                println!("Skipping malformed test case {}: {:?}", index, test_case);
                continue;
            }

            let rule = &test_case[0];
            let data = &test_case[1];
            let expected = &test_case[2];

            let result = match logic.apply(rule, data) {
                Ok(r) => r,
                Err(e) => {
                    println!("Test {} failed with error: {}", index, e);
                    println!("Section: {}", current_section);
                    println!("Rule: {}", rule);
                    println!("Data: {}", data);
                    continue;
                }
            };

            assert_eq!(
                result,
                *expected,
                "\nSection: {}\nTest case {}: \nRule: {}\nData: {}\nExpected: {}\nGot: {}\n",
                current_section, index, rule, data, expected, result
            );
        }
    }
}