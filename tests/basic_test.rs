use chrono::DateTime;
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_basic_equality() {
    let engine = DataLogic::new();

    // Test {"==": [1, 1]}
    let logic = json!({"==": [1, 1]});
    let data = json!({});

    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(data)).unwrap();

    assert_eq!(result, json!(true));
}

#[test]
fn test_variable_access() {
    let engine = DataLogic::new();

    // Test {"var": "name"}
    let logic = json!({"var": "name"});
    let data = json!({"name": "Alice"});

    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(data)).unwrap();

    assert_eq!(result, json!("Alice"));
}

#[test]
fn test_if_then_else() {
    let engine = DataLogic::new();

    // Test {"if": [{"==": [{"var": "temp"}, 100]}, "hot", "cold"]}
    let logic = json!({
        "if": [
            {"==": [{"var": "temp"}, 100]},
            "hot",
            "cold"
        ]
    });

    let data1 = json!({"temp": 100});
    let compiled = engine.compile(&logic).unwrap();
    let result1 = engine.evaluate(&compiled, Arc::new(data1)).unwrap();
    assert_eq!(result1, json!("hot"));

    let data2 = json!({"temp": 50});
    let result2 = engine.evaluate(&compiled, Arc::new(data2)).unwrap();
    assert_eq!(result2, json!("cold"));
}

#[test]
fn test_map_with_context() {
    let engine = DataLogic::new();

    // Test map that adds index to each element
    // {"map": [[1, 2, 3], {"+": [{"val": []}, {"val": [[-1], "index"]}]}]}
    let logic = json!({
        "map": [
            [1, 2, 3],
            {"+": [{"val": []}, {"val": [[0], "index"]}]}
        ]
    });

    let data = json!({});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(data)).unwrap();

    assert_eq!(result, json!([1, 3, 5]));
}

#[test]
fn test_now_operator() {
    let engine = DataLogic::new();

    // Test the now operator
    let logic = json!({"now": []});
    let data = json!({});

    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(data)).unwrap();

    // Verify it's a string
    assert!(result.is_string(), "Now operator should return a string");

    // Verify it's a valid ISO datetime format
    if let serde_json::Value::String(datetime_str) = &result {
        let parsed = DateTime::parse_from_rfc3339(datetime_str);
        assert!(
            parsed.is_ok(),
            "Now operator should return valid ISO datetime format"
        );
    }

    // Test that two calls return different times (with a small delay)
    std::thread::sleep(std::time::Duration::from_millis(10));
    let result2 = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();

    // Note: We can't guarantee they'll be different due to timing precision,
    // but both should be valid datetime strings
    assert!(
        result2.is_string(),
        "Second call to now should also return a string"
    );
}
