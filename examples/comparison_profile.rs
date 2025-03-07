use std::time::Instant;

use datalogic_rs::arena::DataArena;
use datalogic_rs::evaluate;
use datalogic_rs::parse_json;
use datalogic_rs::value::{DataValue, FromJson};
use serde_json::json;

fn main() {
    let iterations = 1_000_000;
    println!("Running {} iterations for each test case:", iterations);

    // Create a data arena for allocating strings
    let arena = DataArena::new();
    
    // Create test data
    let data_json = json!({
        "a": 5,
        "b": 3,
        "c": "5",
        "d": "3",
        "e": true,
        "f": false,
        "g": null,
        "h": [1, 2, 3],
        "i": {"x": 1, "y": 2}
    });
    let data = DataValue::from_json(&data_json, &arena);

    // Define test cases
    let test_cases = [
        // Equal (==)
        ("equal_numbers", r#"{"==":[5, 5]}"#),
        ("equal_string_number", r#"{"==":[5, "5"]}"#),
        ("equal_vars", r#"{"==":["var:a", "var:a"]}"#),
        ("equal_var_string", r#"{"==":["var:a", "var:c"]}"#),
        ("equal_different_types", r#"{"==":["var:h", "var:i"]}"#),
        
        // Strict Equal (===)
        ("strict_equal_numbers", r#"{"===":[5, 5]}"#),
        ("strict_equal_string_number", r#"{"===":[5, "5"]}"#),
        ("strict_equal_vars", r#"{"===":["var:a", "var:a"]}"#),
        ("strict_equal_var_string", r#"{"===":["var:a", "var:c"]}"#),
        
        // Not Equal (!=)
        ("not_equal_numbers", r#"{"!=":[5, 3]}"#),
        ("not_equal_string_number", r#"{"!=":[5, "3"]}"#),
        ("not_equal_vars", r#"{"!=":["var:a", "var:b"]}"#),
        ("not_equal_var_string", r#"{"!=":["var:a", "var:d"]}"#),
        
        // Strict Not Equal (!==)
        ("strict_not_equal_numbers", r#"{"!==":[5, 3]}"#),
        ("strict_not_equal_string_number", r#"{"!==":[5, "5"]}"#),
        ("strict_not_equal_vars", r#"{"!==":["var:a", "var:b"]}"#),
        ("strict_not_equal_var_string", r#"{"!==":["var:a", "var:c"]}"#),
        
        // Greater Than (>)
        ("greater_than_simple", r#"{">":[5, 3]}"#),
        ("greater_than_equal", r#"{">":[5, 5]}"#),
        ("greater_than_string_number", r#"{">":[5, "3"]}"#),
        ("greater_than_vars", r#"{">":[{"var":"a"}, {"var":"b"}]}"#),
        ("greater_than_var_string", r#"{">":[{"var":"a"}, {"var":"d"}]}"#),
        ("greater_than_multiple", r#"{">":[5, 4, 3, 2, 1]}"#),
        
        // Greater Than or Equal (>=)
        ("greater_than_equal_simple", r#"{">=":[5, 3]}"#),
        ("greater_than_equal_equal", r#"{">=":[5, 5]}"#),
        ("greater_than_equal_string_number", r#"{">=":[5, "3"]}"#),
        ("greater_than_equal_vars", r#"{">=":[{"var":"a"}, {"var":"b"}]}"#),
        ("greater_than_equal_var_string", r#"{">=":[{"var":"a"}, {"var":"d"}]}"#),
        ("greater_than_equal_multiple", r#"{">=":[5, 5, 4, 3, 2]}"#),
        
        // Less Than (<)
        ("less_than_simple", r#"{"<":[3, 5]}"#),
        ("less_than_equal", r#"{"<":[5, 5]}"#),
        ("less_than_string_number", r#"{"<":["3", 5]}"#),
        ("less_than_vars", r#"{"<":[{"var":"b"}, {"var":"a"}]}"#),
        ("less_than_var_string", r#"{"<":[{"var":"d"}, {"var":"a"}]}"#),
        ("less_than_multiple", r#"{"<":[1, 2, 3, 4, 5]}"#),
        
        // Less Than or Equal (<=)
        ("less_than_equal_simple", r#"{"<=":[3, 5]}"#),
        ("less_than_equal_equal", r#"{"<=":[5, 5]}"#),
        ("less_than_equal_string_number", r#"{"<=":[3, "5"]}"#),
        ("less_than_equal_vars", r#"{"<=":[{"var":"b"}, {"var":"a"}]}"#),
        ("less_than_equal_var_string", r#"{"<=":[{"var":"d"}, {"var":"a"}]}"#),
        ("less_than_equal_multiple", r#"{"<=":[1, 1, 2, 3, 4]}"#),
    ];

    // Run benchmarks
    for (name, json_str) in &test_cases {
        let json_value = serde_json::from_str(json_str).unwrap();
        let expr = parse_json(&json_value, &arena).unwrap();
        
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = evaluate(&expr, &data, &arena).unwrap();
        }
        let elapsed = start.elapsed();
        
        let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
        let ops_per_sec = 1_000_000_000.0 / ns_per_op;
        
        println!("{}: {}ms ({:.2} ns/op, {:.2} ops/sec)",
                 name, elapsed.as_secs_f64() * 1000.0, ns_per_op, ops_per_sec);
    }
} 