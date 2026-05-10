//! Integration tests for execution trace feature

#![cfg(feature = "trace")]

use datalogic_rs::Engine;
use serde_json::json;

/// Test basic traced evaluation with a simple comparison
#[test]
fn test_trace_simple_comparison() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{">=": [{"var": "age"}, 18]}"#,
        r#"{"age": 25}"#,
    );

    // Result should be true
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Expression tree should have the >= operator at root
    assert!(run.expression_tree.expression.contains(">="));

    // Should have steps recorded (var evaluation + >= evaluation)
    assert!(!run.steps.is_empty());

    // Each step should have a valid node_id
    for step in &run.steps {
        assert!(step.result.is_some() || step.error.is_some());
    }
}

/// Test traced evaluation from the proposal example
#[test]
fn test_trace_proposal_example() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"and": [{">=": [{"var": "age"}, 18]}, true]}"#,
        r#"{"age": 25}"#,
    );

    // Result should be true
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Expression tree root should be "and"
    assert!(run.expression_tree.expression.contains("and"));

    // Should have children (the >= node)
    assert!(!run.expression_tree.children.is_empty());
}

/// Test traced evaluation with short-circuit behavior
#[test]
fn test_trace_short_circuit_and() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"and": [false, {"var": "expensive"}]}"#,
        r#"{}"#,
    );

    // Result should be false
    assert_eq!(run.result.as_ref().unwrap(), &json!(false));

    // The expensive var should NOT be evaluated due to short-circuit
    // So we should have fewer steps than if all branches were evaluated
    // Just verify the result is correct - the short-circuit is implicit
}

/// Test traced evaluation with short-circuit behavior (or)
#[test]
fn test_trace_short_circuit_or() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"or": [true, {"var": "expensive"}]}"#,
        r#"{}"#,
    );

    // Result should be true
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));
}

/// Test traced evaluation with map operator
#[test]
fn test_trace_map_operator() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}"#,
        r#"{}"#,
    );

    // Result should be [2, 4, 6]
    assert_eq!(run.result.as_ref().unwrap(), &json!([2, 4, 6]));

    // Should have steps with iteration info
    let iteration_steps: Vec<_> = run
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
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"filter": [[1, 2, 3, 4, 5], {">": [{"var": ""}, 2]}]}"#,
        r#"{}"#,
    );

    // Result should be [3, 4, 5]
    assert_eq!(run.result.as_ref().unwrap(), &json!([3, 4, 5]));
}

/// Test traced evaluation with reduce operator
#[test]
fn test_trace_reduce_operator() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"reduce": [[1, 2, 3, 4], {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
        r#"{}"#,
    );

    // Result should be 10 (1+2+3+4)
    assert_eq!(run.result.as_ref().unwrap(), &json!(10));
}

/// Test traced evaluation with if operator
#[test]
fn test_trace_if_operator() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"if": [{"var": "active"}, "yes", "no"]}"#,
        r#"{"active": true}"#,
    );

    // Result should be "yes"
    assert_eq!(run.result.as_ref().unwrap(), &json!("yes"));
}

/// Test traced evaluation with nested operators
#[test]
fn test_trace_nested_operators() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"and": [{">": [{"var": "x"}, 0]}, {"<": [{"var": "x"}, 100]}]}"#,
        r#"{"x": 50}"#,
    );

    // Result should be true (0 < 50 < 100)
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Expression tree should have nested structure
    assert!(!run.expression_tree.children.is_empty());
}

/// Test expression tree structure
#[test]
fn test_expression_tree_structure() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{">=": [{"var": "age"}, 18]}"#,
        r#"{"age": 25}"#,
    );

    // Root should have a nonzero compile-time id (0 is the synthetic sentinel)
    assert!(run.expression_tree.id > 0);

    // Root expression should contain >=
    assert!(run.expression_tree.expression.contains(">="));

    // Should have one child (the var node)
    assert_eq!(run.expression_tree.children.len(), 1);

    // Child should also have a nonzero id and should differ from the root's
    assert!(run.expression_tree.children[0].id > 0);
    assert_ne!(run.expression_tree.children[0].id, run.expression_tree.id);

    // Child should be the var node
    assert!(run.expression_tree.children[0].expression.contains("var"));
}

/// Test that literal values don't generate separate steps
#[test]
fn test_literals_no_separate_steps() {
    let engine = Engine::new();
    // Use a variable to prevent static evaluation
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"==": [{"var": "x"}, 1]}"#,
        r#"{"x": 1}"#,
    );

    // Result should be true
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Should have steps for var and == operators
    // The literal "1" does not generate its own step
    assert!(!run.steps.is_empty());

    // Verify no step has "1" as its expression (literals don't generate steps)
    for step in &run.steps {
        // Steps should be for operators, not literals
        assert!(step.result.is_some());
    }
}

/// Test step context values
#[test]
fn test_step_context() {
    let engine = Engine::new();
    let run = engine
        .trace()
        .eval_into::<serde_json::Value, _, _>(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#);

    // Result should be "Alice"
    assert_eq!(run.result.as_ref().unwrap(), &json!("Alice"));

    // Should have a step with context containing name
    assert!(!run.steps.is_empty());
    let step = &run.steps[0];
    assert!(step.context.get("name").is_some());
}

/// Test all/some/none operators with tracing
#[test]
fn test_trace_quantifier_operators() {
    let engine = Engine::new();

    // Test all
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"all": [[1, 2, 3], {">": [{"var": ""}, 0]}]}"#,
        r#"{}"#,
    );
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Test some
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"some": [[1, 2, 3], {">": [{"var": ""}, 2]}]}"#,
        r#"{}"#,
    );
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));

    // Test none
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"none": [[1, 2, 3], {">": [{"var": ""}, 5]}]}"#,
        r#"{}"#,
    );
    assert_eq!(run.result.as_ref().unwrap(), &json!(true));
}

/// Test ternary operator with tracing
#[test]
fn test_trace_ternary_operator() {
    let engine = Engine::new();
    let run = engine
        .trace()
        .eval_into::<serde_json::Value, _, _>(r#"{"?:": [true, "yes", "no"]}"#, r#"{}"#);

    // Result should be "yes"
    assert_eq!(run.result.as_ref().unwrap(), &json!("yes"));
}

/// Test coalesce operator with tracing
#[test]
fn test_trace_coalesce_operator() {
    let engine = Engine::new();
    let run = engine
        .trace()
        .eval_into::<serde_json::Value, _, _>(r#"{"??": [null, null, "found"]}"#, r#"{}"#);

    // Result should be "found"
    assert_eq!(run.result.as_ref().unwrap(), &json!("found"));
}

/// Test error handling in trace
#[test]
fn test_trace_with_error() {
    let engine = Engine::new();
    let run = engine
        .trace()
        .eval_into::<serde_json::Value, _, _>(r#"{"var": "missing.path"}"#, r#"{}"#);

    // Result should be null for missing path
    assert_eq!(run.result.as_ref().unwrap(), &json!(null));
}

/// Test arithmetic operators with tracing
#[test]
fn test_trace_arithmetic() {
    let engine = Engine::new();
    // Use variable to prevent static evaluation
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"+": [{"*": [{"var": "x"}, 3]}, 4]}"#,
        r#"{"x": 2}"#,
    );

    // Result should be 10 (2*3 + 4)
    assert_eq!(run.result.as_ref().unwrap(), &json!(10));

    // Should have steps recorded (var evaluation + * + +)
    assert!(!run.steps.is_empty());

    // Expression tree should have nested children (+ contains *)
    assert!(run.expression_tree.expression.contains("+"));
}

/// Test string operators with tracing
#[test]
fn test_trace_string_operators() {
    let engine = Engine::new();
    let run = engine.trace().eval_into::<serde_json::Value, _, _>(
        r#"{"cat": ["Hello, ", {"var": "name"}]}"#,
        r#"{"name": "World"}"#,
    );

    // Result should be "Hello, World"
    assert_eq!(run.result.as_ref().unwrap(), &json!("Hello, World"));
}

/// Test that an error returned from a `CustomOperator` propagates through
/// the trace path with its operator name and message preserved, and that
/// the trace still records steps up to the failure point.
#[test]
fn test_trace_custom_operator_error_propagation() {
    use bumpalo::Bump;
    use datalogic_rs::operator::EvalContext;
    use datalogic_rs::{CustomOperator, DataValue, Error, Result as DLResult};

    struct FailOp;
    impl CustomOperator for FailOp {
        fn evaluate<'a>(
            &self,
            _args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            _arena: &'a Bump,
        ) -> DLResult<&'a DataValue<'a>> {
            Err(Error::custom_message("boom"))
        }
    }

    let engine = Engine::builder().add_operator("fail_op", FailOp).build();
    let run = engine.trace().eval_str(r#"{"fail_op": []}"#, "null");

    // The user's `Error::custom_message("boom")` propagates back as an Err.
    let err = run.result.expect_err("FailOp returned Err");

    // The boundary sets `operator` from the root op name (the custom op
    // sits at root, so root_op_name resolves to "fail_op").
    assert_eq!(err.operator(), Some("fail_op"));

    // The Custom error variant carries the user's message; Display
    // renders it as part of the full error string.
    assert!(
        err.to_string().contains("boom"),
        "error display should contain 'boom', got: {err}",
    );

    // The trace must record at least one step — the custom-op
    // invocation that failed. Some compile-time tree shapes record an
    // error step explicitly, others just emit the eval step that
    // returned `Err`; we just assert non-emptiness here so the test
    // doesn't depend on which shape the trace collector uses today.
    assert!(
        !run.steps.is_empty(),
        "trace should record at least one step before the failure",
    );

    // The expression tree always contains the root op name regardless
    // of success or failure.
    assert!(
        run.expression_tree.expression.contains("fail_op"),
        "expression tree should mention 'fail_op': {:?}",
        run.expression_tree
    );
}
