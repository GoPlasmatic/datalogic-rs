use datalogic_rs::arena::DataArena;
use datalogic_rs::logic::{evaluate, parse_json};
use datalogic_rs::value::{DataValue, FromJson};
use serde_json::json;

fn main() {
    // Create test data
    let data_json = json!({ "hello" : 1 });
    
    // Convert to DataValue
    let arena = DataArena::new();
    let data = DataValue::from_json(&data_json, &arena);
    
    // Parse rule
    let rule_json: serde_json::Value = serde_json::from_str(r#"{ "exists": "hello" }"#).unwrap();
    let rule_token = parse_json(&rule_json, &arena).unwrap();
    println!("rule_token: {:?}", rule_token);
            
    let result = evaluate(rule_token, &data, &arena).unwrap();
    println!("result: {:?}", result);
} 