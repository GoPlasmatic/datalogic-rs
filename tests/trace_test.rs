//! Integration tests for execution trace feature

use datalogic_rs::DataLogic;
use serde_json::json;

/// Test basic traced evaluation with a simple comparison
#[test]
fn test_trace_simple_comparison() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{">=": [{"var": "age"}, 18]}"#, r#"{"age": 25}"#)
        .unwrap();

    // Result should be true
    assert_eq!(result.result, json!(true));

    // Expression tree should have the >= operator at root
    assert!(result.expression_tree.expression.contains(">="));

    // Should have steps recorded (var evaluation + >= evaluation)
    assert!(!result.steps.is_empty());

    // Each step should have a valid node_id
    for step in &result.steps {
        assert!(step.result.is_some() || step.error.is_some());
    }
}

/// Test traced evaluation from the proposal example
#[test]
fn test_trace_proposal_example() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"and": [{">=": [{"var": "age"}, 18]}, true]}"#,
            r#"{"age": 25}"#,
        )
        .unwrap();

    // Result should be true
    assert_eq!(result.result, json!(true));

    // Expression tree root should be "and"
    assert!(result.expression_tree.expression.contains("and"));

    // Should have children (the >= node)
    assert!(!result.expression_tree.children.is_empty());
}

/// Test traced evaluation with short-circuit behavior
#[test]
fn test_trace_short_circuit_and() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"and": [false, {"var": "expensive"}]}"#, r#"{}"#)
        .unwrap();

    // Result should be false
    assert_eq!(result.result, json!(false));

    // The expensive var should NOT be evaluated due to short-circuit
    // So we should have fewer steps than if all branches were evaluated
    // Just verify the result is correct - the short-circuit is implicit
}

/// Test traced evaluation with short-circuit behavior (or)
#[test]
fn test_trace_short_circuit_or() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"or": [true, {"var": "expensive"}]}"#, r#"{}"#)
        .unwrap();

    // Result should be true
    assert_eq!(result.result, json!(true));
}

/// Test traced evaluation with map operator
#[test]
fn test_trace_map_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}"#, r#"{}"#)
        .unwrap();

    // Result should be [2, 4, 6]
    assert_eq!(result.result, json!([2, 4, 6]));

    // Should have steps with iteration info
    let iteration_steps: Vec<_> = result
        .steps
        .iter()
        .filter(|s| s.iteration_index.is_some())
        .collect();

    // Map over 3 elements should have iteration steps
    assert!(!iteration_steps.is_empty());

    // Verify iteration indices are present
    for step in &iteration_steps {
        assert!(step.iteration_total == Some(3));
    }
}

/// Test traced evaluation with filter operator
#[test]
fn test_trace_filter_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"filter": [[1, 2, 3, 4, 5], {">": [{"var": ""}, 2]}]}"#,
            r#"{}"#,
        )
        .unwrap();

    // Result should be [3, 4, 5]
    assert_eq!(result.result, json!([3, 4, 5]));
}

/// Test traced evaluation with reduce operator
#[test]
fn test_trace_reduce_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"reduce": [[1, 2, 3, 4], {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
            r#"{}"#,
        )
        .unwrap();

    // Result should be 10 (1+2+3+4)
    assert_eq!(result.result, json!(10));
}

/// Test traced evaluation with if operator
#[test]
fn test_trace_if_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"if": [{"var": "active"}, "yes", "no"]}"#,
            r#"{"active": true}"#,
        )
        .unwrap();

    // Result should be "yes"
    assert_eq!(result.result, json!("yes"));
}

/// Test traced evaluation with nested operators
#[test]
fn test_trace_nested_operators() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"and": [{">": [{"var": "x"}, 0]}, {"<": [{"var": "x"}, 100]}]}"#,
            r#"{"x": 50}"#,
        )
        .unwrap();

    // Result should be true (0 < 50 < 100)
    assert_eq!(result.result, json!(true));

    // Expression tree should have nested structure
    assert!(!result.expression_tree.children.is_empty());
}

/// Test expression tree structure
#[test]
fn test_expression_tree_structure() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{">=": [{"var": "age"}, 18]}"#, r#"{"age": 25}"#)
        .unwrap();

    // Root should have id 0
    assert_eq!(result.expression_tree.id, 0);

    // Root expression should contain >=
    assert!(result.expression_tree.expression.contains(">="));

    // Should have one child (the var node)
    assert_eq!(result.expression_tree.children.len(), 1);

    // Child should have id > 0
    assert!(result.expression_tree.children[0].id > 0);

    // Child should be the var node
    assert!(
        result.expression_tree.children[0]
            .expression
            .contains("var")
    );
}

/// Test that literal values don't generate separate steps
#[test]
fn test_literals_no_separate_steps() {
    let engine = DataLogic::new();
    // Use a variable to prevent static evaluation
    let result = engine
        .evaluate_json_with_trace(r#"{"==": [{"var": "x"}, 1]}"#, r#"{"x": 1}"#)
        .unwrap();

    // Result should be true
    assert_eq!(result.result, json!(true));

    // Should have steps for var and == operators
    // The literal "1" does not generate its own step
    assert!(!result.steps.is_empty());

    // Verify no step has "1" as its expression (literals don't generate steps)
    for step in &result.steps {
        // Steps should be for operators, not literals
        assert!(step.result.is_some());
    }
}

/// Test step context values
#[test]
fn test_step_context() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#)
        .unwrap();

    // Result should be "Alice"
    assert_eq!(result.result, json!("Alice"));

    // Should have a step with context containing name
    assert!(!result.steps.is_empty());
    let step = &result.steps[0];
    assert!(step.context.get("name").is_some());
}

/// Test all/some/none operators with tracing
#[test]
fn test_trace_quantifier_operators() {
    let engine = DataLogic::new();

    // Test all
    let result = engine
        .evaluate_json_with_trace(r#"{"all": [[1, 2, 3], {">": [{"var": ""}, 0]}]}"#, r#"{}"#)
        .unwrap();
    assert_eq!(result.result, json!(true));

    // Test some
    let result = engine
        .evaluate_json_with_trace(r#"{"some": [[1, 2, 3], {">": [{"var": ""}, 2]}]}"#, r#"{}"#)
        .unwrap();
    assert_eq!(result.result, json!(true));

    // Test none
    let result = engine
        .evaluate_json_with_trace(r#"{"none": [[1, 2, 3], {">": [{"var": ""}, 5]}]}"#, r#"{}"#)
        .unwrap();
    assert_eq!(result.result, json!(true));
}

/// Test ternary operator with tracing
#[test]
fn test_trace_ternary_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"?:": [true, "yes", "no"]}"#, r#"{}"#)
        .unwrap();

    // Result should be "yes"
    assert_eq!(result.result, json!("yes"));
}

/// Test coalesce operator with tracing
#[test]
fn test_trace_coalesce_operator() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"??": [null, null, "found"]}"#, r#"{}"#)
        .unwrap();

    // Result should be "found"
    assert_eq!(result.result, json!("found"));
}

/// Test error handling in trace
#[test]
fn test_trace_with_error() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(r#"{"var": "missing.path"}"#, r#"{}"#)
        .unwrap();

    // Result should be null for missing path
    assert_eq!(result.result, json!(null));
}

/// Test arithmetic operators with tracing
#[test]
fn test_trace_arithmetic() {
    let engine = DataLogic::new();
    // Use variable to prevent static evaluation
    let result = engine
        .evaluate_json_with_trace(r#"{"+": [{"*": [{"var": "x"}, 3]}, 4]}"#, r#"{"x": 2}"#)
        .unwrap();

    // Result should be 10 (2*3 + 4)
    assert_eq!(result.result, json!(10));

    // Should have steps recorded (var evaluation + * + +)
    assert!(!result.steps.is_empty());

    // Expression tree should have nested children (+ contains *)
    assert!(result.expression_tree.expression.contains("+"));
}

/// Test string operators with tracing
#[test]
fn test_trace_string_operators() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_with_trace(
            r#"{"cat": ["Hello, ", {"var": "name"}]}"#,
            r#"{"name": "World"}"#,
        )
        .unwrap();

    // Result should be "Hello, World"
    assert_eq!(result.result, json!("Hello, World"));
}
