#![cfg(feature = "serde_json")]

use datalogic_rs::Engine;
use serde_json::json;

#[test]
fn test_basic_equality() {
    let engine = Engine::new();
    let logic = json!({"==": [1, 1]});
    let data = json!({});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &data)
        .unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_variable_access() {
    let engine = Engine::new();
    let logic = json!({"var": "name"});
    let data = json!({"name": "Alice"});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &data)
        .unwrap();
    assert_eq!(result, json!("Alice"));
}

#[test]
fn test_if_then_else() {
    let engine = Engine::new();
    let logic = json!({
        "if": [
            {"==": [{"var": "temp"}, 100]},
            "hot",
            "cold"
        ]
    });

    let result1 = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({"temp": 100}))
        .unwrap();
    assert_eq!(result1, json!("hot"));

    let result2 = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({"temp": 50}))
        .unwrap();
    assert_eq!(result2, json!("cold"));
}

// `val` and the metadata-hint `index` form are part of the ext-control extension.
#[cfg(feature = "ext-control")]
#[test]
fn test_map_with_context() {
    let engine = Engine::new();
    let logic = json!({
        "map": [
            [1, 2, 3],
            {"+": [{"val": []}, {"val": [[0], "index"]}]}
        ]
    });
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!([1, 3, 5]));
}

#[cfg(feature = "datetime")]
#[test]
fn test_now_operator() {
    use chrono::DateTime;
    let engine = Engine::new();

    let logic = json!({"now": []});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();

    assert!(result.is_string(), "Now operator should return a string");

    if let serde_json::Value::String(datetime_str) = &result {
        let parsed = DateTime::parse_from_rfc3339(datetime_str);
        assert!(
            parsed.is_ok(),
            "Now operator should return valid ISO datetime format"
        );
    }

    std::thread::sleep(std::time::Duration::from_millis(10));
    let result2 = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert!(
        result2.is_string(),
        "Second call to now should also return a string"
    );
}

#[test]
fn test_evaluate_json_value_api() {
    let engine = Engine::new();
    let logic = json!({"+": [{"var": "a"}, {"var": "b"}]});
    let data = json!({"a": 3, "b": 4});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &data)
        .unwrap();
    assert_eq!(result, json!(7));
}

#[cfg(feature = "ext-string")]
#[test]
fn test_evaluate_with_arena_dispatch() {
    let engine = Engine::new();
    // A rule that triggers arena dispatch (filter + length).
    let logic = json!({"length": {"filter": [{"var": "items"}, {">": [{"var": ""}, 2]}]}});
    let data = json!({"items": [1, 2, 3, 4, 5]});
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &data)
        .unwrap();
    assert_eq!(result, json!(3));
}
