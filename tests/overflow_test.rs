use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_substr_overflow_protection() {
    let engine = DataLogic::new();

    // Test with i64::MAX start index
    let logic = json!({"substr": ["test", i64::MAX, 2]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!(""));

    // Test with i64::MIN start index
    let logic = json!({"substr": ["test", i64::MIN, 2]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!("te"));

    // Test with very large negative index
    let logic = json!({"substr": ["hello", -1000000000000i64, 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!("hel"));

    // Test with large positive length
    let logic = json!({"substr": ["hello", 0, i64::MAX]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!("hello"));

    // Test with negative length (JSONLogic behavior: acts like slice end position)
    let logic = json!({"substr": ["hello world", 6, -1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!("worl"));
}

#[test]
fn test_arithmetic_overflow_protection() {
    let engine = DataLogic::new();

    // Test addition overflow
    let logic = json!({"+": [i64::MAX, 1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should switch to float representation
    assert!(result.as_f64().is_some());
    assert_eq!(result.as_f64().unwrap(), (i64::MAX as f64) + 1.0);

    // Test subtraction underflow
    let logic = json!({"-": [i64::MIN, 1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should switch to float representation
    assert!(result.as_f64().is_some());

    // Test multiplication overflow
    let logic = json!({"*": [i64::MAX / 2, 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should switch to float representation
    assert!(result.as_f64().is_some());

    // Test division of i64::MIN by -1 (special overflow case)
    let logic = json!({"/": [i64::MIN, -1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should handle gracefully
    assert!(result.as_f64().is_some());
    assert_eq!(result.as_f64().unwrap(), -(i64::MIN as f64));

    // Test modulo of i64::MIN by -1 (special case)
    let logic = json!({"%": [i64::MIN, -1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn test_array_slice_overflow_protection() {
    let engine = DataLogic::new();

    // Test with large negative start index
    let logic = json!({"slice": [[1, 2, 3, 4, 5], i64::MIN, null]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([1, 2, 3, 4, 5]));

    // Test with large positive start index
    let logic = json!({"slice": [[1, 2, 3], i64::MAX, null]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([]));

    // Test with large step value
    let logic = json!({"slice": [[1, 2, 3, 4, 5], 0, null, i64::MAX]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([1])); // Should only get first element due to large step

    // Test with negative step and boundary conditions
    let logic = json!({"slice": [[1, 2, 3, 4, 5], -1, i64::MIN, -1]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([5, 4, 3, 2]));
}

#[test]
fn test_string_slice_overflow() {
    let engine = DataLogic::new();

    // Test slicing string with large indices
    let logic = json!({"slice": ["hello", i64::MAX, null]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!(""));

    // Test slicing with negative overflow
    let logic = json!({"slice": ["world", i64::MIN, 3]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!("wor"));
}

#[test]
fn test_array_operations_with_large_indices() {
    let engine = DataLogic::new();

    // Test map with current element (should not overflow)
    let logic = json!({
        "map": [
            [10, 20, 30],
            {"*": [{"var": ""}, 2]}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([20, 40, 60])); // element * 2

    // Test filter with large array (index should be safe)
    let large_array: Vec<i32> = (0..1000).collect();
    let logic = json!({
        "filter": [
            large_array,
            {">": [{"var": ""}, 995]}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([996, 997, 998, 999]));
}

#[test]
fn test_chained_arithmetic_overflow() {
    let engine = DataLogic::new();

    // Test chained addition that would overflow
    let logic = json!({"+": [i64::MAX / 2, i64::MAX / 2, 10]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should switch to float
    assert!(result.as_f64().is_some());

    // Test chained subtraction with overflow
    let logic = json!({"-": [0, i64::MAX, 10]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert!(result.as_f64().is_some());

    // Test chained multiplication with overflow detection
    let logic = json!({"*": [1000000, 1000000, 1000000]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should switch to float due to overflow
    assert!(result.as_f64().is_some());
    assert_eq!(result.as_f64().unwrap(), 1e18);
}

#[test]
fn test_edge_cases() {
    let engine = DataLogic::new();

    // Test division by zero is handled
    let logic = json!({"/": [1, 0]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    assert!(result.is_err());

    // Test modulo by zero is handled
    let logic = json!({"%": [10, 0]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    assert!(result.is_err());

    // Test substr with empty string
    let logic = json!({"substr": ["", i64::MAX, i64::MIN]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!(""));

    // Test slice with empty array
    let logic = json!({"slice": [[], i64::MIN, i64::MAX, -1000]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert_eq!(result, json!([]));
}
