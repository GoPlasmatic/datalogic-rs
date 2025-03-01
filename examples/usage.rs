use datalogic_rs::*;
use serde_json::json;

fn main() {
    let logic = json!({ "preserve": [{ "a": 1 }, { "b": 2 }, { "a": 1, "b": 2 }] });
    println!("Logic: {:?}", logic);

    let data = json!({});

    let rule = Rule::from_value(&logic).unwrap();
    println!("Rule: {:#?}", rule);

    let result = JsonLogic::apply(&rule, &data).unwrap();
    println!("Result: {}", result);
}
