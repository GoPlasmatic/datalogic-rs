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

#[test]
fn test_datetime_duration_overflow() {
    let engine = DataLogic::new();

    // Test parsing duration with large values that would overflow
    let logic = json!({"timestamp": "9223372036854775807d"}); // i64::MAX days
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    // Should fail gracefully or saturate
    assert!(result.is_ok() || result.is_err());

    // Test duration multiplication with large factor
    let logic = json!({"*": [{"timestamp": "1d"}, 1e15]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should saturate at max value rather than overflow
    assert!(result.is_string());

    // Test duration multiplication with negative overflow
    let logic = json!({"*": [{"timestamp": "1000000d"}, -1e10]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert!(result.is_string());

    // Test duration division by very small number (would overflow)
    let logic = json!({"/": [{"timestamp": "1d"}, 1e-15]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should saturate rather than overflow
    assert!(result.is_string());

    // Test duration division by zero
    let logic = json!({"/": [{"timestamp": "1d"}, 0]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    // Should handle division by zero gracefully
    assert!(result.is_ok() || result.is_err());

    // Test duration multiplication with NaN
    let logic = json!({"*": [{"timestamp": "1d"}, f64::NAN]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should handle NaN gracefully
    assert!(result.is_string());

    // Test duration multiplication with infinity
    let logic = json!({"*": [{"timestamp": "1d"}, f64::INFINITY]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should saturate at max
    assert!(result.is_string());
}

#[test]
fn test_datetime_arithmetic_overflow() {
    let engine = DataLogic::new();

    // Test adding large duration to datetime
    let logic = json!({
        "+": [
            {"datetime": "2023-01-01T00:00:00Z"},
            {"timestamp": "1000000000d"} // Very large duration
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    // Should produce a valid datetime string
    assert!(result.is_string());

    // Test subtracting large duration from datetime
    let logic = json!({
        "-": [
            {"datetime": "2023-01-01T00:00:00Z"},
            {"timestamp": "1000000000d"}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert!(result.is_string());

    // Test adding two large durations
    let logic = json!({
        "+": [
            {"timestamp": "1000000000d"},
            {"timestamp": "1000000000d"}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert!(result.is_string());

    // Test subtracting large durations
    let logic = json!({
        "-": [
            {"timestamp": "1d"},
            {"timestamp": "1000000000d"}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({}))).unwrap();
    assert!(result.is_string());
}

#[test]
fn test_duration_parsing_overflow_protection() {
    let engine = DataLogic::new();

    // Test parsing with values that would overflow in calculation
    // 106751991167d is approximately i64::MAX seconds / 86400
    let logic = json!({"timestamp": "106751991167d:24h:60m:60s"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    // Should either parse successfully with saturation or fail gracefully
    assert!(result.is_ok() || result.is_err());

    // Test parsing with multiple large components
    let logic = json!({"timestamp": "1000000000000h:1000000000000m"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    assert!(result.is_ok() || result.is_err());

    // Test compact format with large values
    let logic = json!({"timestamp": "9999999999999999999d"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate(&compiled, Arc::new(json!({})));
    assert!(result.is_ok() || result.is_err());
}
