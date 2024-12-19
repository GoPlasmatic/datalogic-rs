use datalogic_rs::*;
use serde_json::json;

fn main() {
    let logic = json!({
        "cat": {
          "map": [
            ["Jesse", "Jeremy", "Harishankar"],
            {
              "cat": [
                { "var": [] },
                ' '
              ]
            }
          ]
        }
      });
    
    let data = json!({
        "user": {
            "age": 21,
            "name": "John"
        }
    });
    println!("Logic: {:?}", logic);

    let rule = Rule::from_value(&logic).unwrap();
    println!("Rule: {:?}", rule);
    let result = JsonLogic::apply(&rule, &data).unwrap();
    println!("Result: {}", result);
}