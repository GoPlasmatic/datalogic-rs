use datalogic_rs::JsonLogic;
use serde_json::json;

fn main() {
    let logic = JsonLogic::new();
    
    let rule = json!({
        "==": [
            {"var": "user.age"},
            21
        ]
    });
    
    let data = json!({
        "user": {
            "age": 21,
            "name": "John"
        }
    });

    let result = logic.apply(&rule, &data).unwrap();
    println!("Is user 21? {}", result);  // Prints: Is user 21? true
}