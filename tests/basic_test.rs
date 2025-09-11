use datalogic_rs::DataLogic;
use serde_json::json;
use std::borrow::Cow;

#[test]
fn test_basic_equality() {
    let engine = DataLogic::new();

    // Test {"==": [1, 1]}
    let logic = json!({"==": [1, 1]});
    let data = json!({});

    let compiled = engine.compile(Cow::Borrowed(&logic)).unwrap();
    let result = engine.evaluate_owned(&compiled, data).unwrap();

    assert_eq!(result, json!(true));
}

#[test]
fn test_variable_access() {
    let engine = DataLogic::new();

    // Test {"var": "name"}
    let logic = json!({"var": "name"});
    let data = json!({"name": "Alice"});

    let compiled = engine.compile(Cow::Borrowed(&logic)).unwrap();
    let result = engine.evaluate_owned(&compiled, data).unwrap();

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
    let compiled = engine.compile(Cow::Borrowed(&logic)).unwrap();
    let result1 = engine.evaluate_owned(&compiled, data1).unwrap();
    assert_eq!(result1, json!("hot"));

    let data2 = json!({"temp": 50});
    let result2 = engine.evaluate_owned(&compiled, data2).unwrap();
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
    let compiled = engine.compile(Cow::Borrowed(&logic)).unwrap();
    let result = engine.evaluate_owned(&compiled, data).unwrap();

    assert_eq!(result, json!([1, 3, 5]));
}
