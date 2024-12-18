use datalogic_rs::*;
use serde_json::json;

fn main() {
    let logic = json!({
        "map": [[5, 6, 7], { "+": [{ "var": [] }, 1] }]
      });
    
    let data = json!({
        "user": {
            "age": 21,
            "name": "John"
        }
    });

    let rule = Rule::from_value(&logic).unwrap();
    println!("Rule: {:?}", rule);
    let result = JsonLogic::apply(&rule, &data).unwrap();
    println!("Result: {}", result);
}