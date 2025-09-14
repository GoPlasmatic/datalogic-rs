use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_arc_data_sharing() {
    let engine = DataLogic::new();

    // Create a large data object wrapped in Arc
    let data = Arc::new(json!({
        "user": {
            "name": "Alice",
            "age": 30,
            "active": true
        },
        "items": [1, 2, 3, 4, 5]
    }));

    // Test 1: Simple variable access
    let logic1 = json!({"var": "user.name"});
    let compiled1 = engine.compile(&logic1).unwrap();
    let result1 = engine.evaluate(&compiled1, data.clone()).unwrap();
    assert_eq!(result1, json!("Alice"));

    // Test 2: Array map operation (uses nested contexts)
    let logic2 = json!({
        "map": [
            {"var": "items"},
            {"+": [{"var": ""}, 10]}
        ]
    });
    let compiled2 = engine.compile(&logic2).unwrap();
    let result2 = engine.evaluate(&compiled2, data.clone()).unwrap();
    assert_eq!(result2, json!([11, 12, 13, 14, 15]));

    // Test 3: Complex nested operation
    let logic3 = json!({
        "if": [
            {"var": "user.active"},
            {"filter": [
                {"var": "items"},
                {">": [{"var": ""}, 2]}
            ]},
            []
        ]
    });
    let compiled3 = engine.compile(&logic3).unwrap();
    let result3 = engine.evaluate(&compiled3, data.clone()).unwrap();
    assert_eq!(result3, json!([3, 4, 5]));

    // Verify the Arc is still valid and unchanged
    assert_eq!(Arc::strong_count(&data), 1);
    assert_eq!(
        *data,
        json!({
            "user": {
                "name": "Alice",
                "age": 30,
                "active": true
            },
            "items": [1, 2, 3, 4, 5]
        })
    );
}
