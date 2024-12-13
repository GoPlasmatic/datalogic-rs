use datalogic_rs::*;
use serde_json::json;

fn main() {
    // Test both escaped dot and regular dot navigation
    let test_cases = vec![
        // Test 1: Escaped dot should look up exact key
        (
            json!({"var": "hello\\.world"}),
            json!({"hello": {"world": "i'm here!"}, "hello.world": "ups!"}),
            "ups!"
        ),
        // Test 2: Regular dot should navigate nested object
        (
            json!({"var": "hello.world"}),
            json!({"hello": {"world": "i'm here!"}, "hello.world": "ups!"}),
            "i'm here!"
        )
    ];

    for (logic, data, expected) in test_cases {
        let rule = Rule::from_value(&logic).unwrap();
        let result = JsonLogic::apply(&rule, &data).unwrap();
        println!("Rule: {}", logic);
        println!("Result: {}", result);
        println!("Expected: {}", expected);
        println!("---");
    }
}