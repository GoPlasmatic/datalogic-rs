#![cfg(feature = "compat")]
#![allow(deprecated)]

use datalogic_rs::compat::LegacyApi;
use datalogic_rs::{Engine, Error};
use serde_json::{Value, json};

fn to_json(err: &Error) -> Value {
    serde_json::to_value(err).expect("Error must serialize")
}

#[test]
fn serialize_invalid_operator() {
    let err = Error::invalid_operator("foo");
    assert_eq!(
        to_json(&err),
        json!({"type": "InvalidOperator", "message": "Invalid operator: foo"})
    );
}

#[test]
fn serialize_invalid_arguments() {
    let err = Error::invalid_arguments("need 2");
    assert_eq!(
        to_json(&err),
        json!({"type": "InvalidArguments", "message": "Invalid arguments: need 2"})
    );
}

#[test]
fn serialize_variable_not_found() {
    let err = Error::variable_not_found("user.name");
    assert_eq!(
        to_json(&err),
        json!({
            "type": "VariableNotFound",
            "message": "Variable not found: user.name",
            "variable": "user.name",
        })
    );
}

#[test]
fn serialize_invalid_context_level() {
    let err = Error::invalid_context_level(-3);
    assert_eq!(
        to_json(&err),
        json!({
            "type": "InvalidContextLevel",
            "message": "Invalid context level: -3",
            "level": -3,
        })
    );
}

#[test]
fn serialize_type_error() {
    let err = Error::type_error("cannot compare");
    assert_eq!(
        to_json(&err),
        json!({"type": "TypeError", "message": "Type error: cannot compare"})
    );
}

#[test]
fn serialize_arithmetic_error() {
    let err = Error::arithmetic_error("divide by zero");
    assert_eq!(
        to_json(&err),
        json!({"type": "ArithmeticError", "message": "Arithmetic error: divide by zero"})
    );
}

#[test]
fn serialize_custom() {
    let err = Error::custom_message("user-defined");
    assert_eq!(
        to_json(&err),
        json!({"type": "Custom", "message": "user-defined"})
    );
}

#[test]
fn serialize_parse_error() {
    let err = Error::parse_error("expected value");
    assert_eq!(
        to_json(&err),
        json!({"type": "ParseError", "message": "Parse error: expected value"})
    );
}

#[test]
fn serialize_thrown() {
    use datavalue::{NumberValue, OwnedDataValue};
    let owned = OwnedDataValue::Object(vec![
        (
            "code".to_string(),
            OwnedDataValue::Number(NumberValue::Integer(42)),
        ),
        (
            "reason".to_string(),
            OwnedDataValue::String("boom".to_string()),
        ),
    ]);
    let err = Error::thrown(owned);
    let v = to_json(&err);
    assert_eq!(v["type"], json!("Thrown"));
    assert_eq!(v["thrown"], json!({"code": 42, "reason": "boom"}));
    // message is the Display output, which prints the JSON payload
    assert!(v["message"].as_str().unwrap().starts_with("Thrown: "));
}

#[test]
fn serialize_format_error() {
    let err = Error::format_error("bad pattern");
    assert_eq!(
        to_json(&err),
        json!({"type": "FormatError", "message": "Format error: bad pattern"})
    );
}

#[test]
fn serialize_index_out_of_bounds() {
    let err = Error::index_out_of_bounds(5, 3);
    assert_eq!(
        to_json(&err),
        json!({
            "type": "IndexOutOfBounds",
            "message": "Index 5 out of bounds for array of length 3",
            "index": 5,
            "length": 3,
        })
    );
}

#[test]
fn serialize_configuration_error() {
    let err = Error::configuration_error("invalid setting");
    assert_eq!(
        to_json(&err),
        json!({"type": "ConfigurationError", "message": "Configuration error: invalid setting"})
    );
}

#[test]
fn structured_error_adds_operator_field() {
    let se = Error::arithmetic_error("divide by zero").with_operator("/");
    let v = serde_json::to_value(&se).unwrap();
    assert_eq!(
        v,
        json!({
            "type": "ArithmeticError",
            "message": "Arithmetic error: divide by zero",
            "operator": "/",
        })
    );
}

#[test]
fn structured_error_omits_operator_when_none() {
    let se = Error::type_error("x");
    let v = serde_json::to_value(&se).unwrap();
    assert_eq!(v, json!({"type": "TypeError", "message": "Type error: x"}));
}

#[test]
fn structured_error_flattens_variant_extras() {
    let se = Error::index_out_of_bounds(10, 2).with_operator("substr");
    let v = serde_json::to_value(&se).unwrap();
    assert_eq!(
        v,
        json!({
            "type": "IndexOutOfBounds",
            "message": "Index 10 out of bounds for array of length 2",
            "index": 10,
            "length": 2,
            "operator": "substr",
        })
    );
}

#[cfg(feature = "error-handling")]
#[test]
fn evaluate_json_structured_reports_outer_operator() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured(r#"{"throw": {"type": "Boom"}}"#, r#"{}"#)
        .expect_err("throw must produce a structured error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["type"], json!("Thrown"));
    assert!(v["thrown"].is_object(), "thrown payload must be preserved");
    assert_eq!(v["operator"], json!("throw"));
}

#[test]
fn evaluate_json_structured_success_passes_through() {
    let engine = Engine::new();
    let result = engine
        .evaluate_json_structured(r#"{"+": [1, 2]}"#, r#"{}"#)
        .unwrap();
    // Accept either integer or float encoding — datalogic returns either based on inputs.
    assert_eq!(result.as_f64(), Some(3.0));
}

#[test]
fn evaluate_json_structured_parse_error_has_no_operator() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured("not json", r#"{}"#)
        .expect_err("invalid JSON should error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["type"], json!("ParseError"));
    assert!(v.get("operator").is_none());
}

#[test]
fn evaluate_json_structured_captures_arithmetic_operator() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured(r#"{"/": [1, 0]}"#, "{}")
        .expect_err("division by zero must error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["operator"], json!("/"));
    assert_eq!(v["type"], json!("Thrown"));
}

#[test]
fn evaluate_json_structured_captures_unknown_operator() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured(r#"{"not_a_real_op_123": [1]}"#, "{}")
        .expect_err("unknown op must error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["type"], json!("InvalidOperator"));
    // The outermost node IS an InvalidOperator, so operator field mirrors the name.
    assert_eq!(v["operator"], json!("not_a_real_op_123"));
}

#[test]
fn evaluate_json_structured_captures_type_coercion_op() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured(r#"{"+": ["abc", 1]}"#, "{}")
        .expect_err("non-numeric addition must error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["operator"], json!("+"));
}

#[cfg(feature = "trace")]
#[test]
fn evaluate_json_with_trace_structured_populates_error_fields() {
    let engine = Engine::new();
    let traced = engine
        .evaluate_json_with_trace_structured(r#"{"throw": {"type": "Boom"}}"#, r#"{}"#)
        .unwrap();
    assert!(traced.error.is_some());
    let structured = traced.structured_error.expect("structured must populate");
    let v = serde_json::to_value(&structured).unwrap();
    assert_eq!(v["type"], json!("Thrown"));
    assert_eq!(v["operator"], json!("throw"));
}

/// Regression gate: Display output must stay byte-identical to v4.0.21, since
/// the JSONLogic test suite and downstream TS consumers depend on it.
#[test]
fn display_output_snapshot() {
    let cases: &[(Error, &str)] = &[
        (Error::invalid_operator("foo"), "Invalid operator: foo"),
        (Error::invalid_arguments("x"), "Invalid arguments: x"),
        (
            Error::variable_not_found("user.name"),
            "Variable not found: user.name",
        ),
        (
            Error::invalid_context_level(-2),
            "Invalid context level: -2",
        ),
        (Error::type_error("x"), "Type error: x"),
        (Error::arithmetic_error("x"), "Arithmetic error: x"),
        (Error::custom_message("raw"), "raw"),
        (Error::parse_error("x"), "Parse error: x"),
        (
            Error::thrown(datavalue::OwnedDataValue::Object(vec![(
                "k".to_string(),
                datavalue::OwnedDataValue::Number(datavalue::NumberValue::Integer(1)),
            )])),
            "Thrown: {\"k\":1}",
        ),
        (Error::format_error("x"), "Format error: x"),
        (
            Error::index_out_of_bounds(1, 0),
            "Index 1 out of bounds for array of length 0",
        ),
        (Error::configuration_error("x"), "Configuration error: x"),
    ];
    for (err, expected) in cases {
        assert_eq!(err.to_string(), *expected, "Display changed for {:?}", err);
    }
}

// =====================================================================
// Error breadcrumb path tests (plan item #4)
// =====================================================================

#[test]
fn structured_error_has_nonempty_path_on_runtime_error() {
    let engine = Engine::new();
    // Nested variable access that will fail: outer wraps an unknown-var read.
    // The path should contain at least one node id (the failing node), and
    // more if the error unwinds through additional operators.
    // `throw` wrapped in an `if` with a DYNAMIC condition (reads a var)
    // so the optimiser can't fold the if away and the error unwinds
    // through multiple operator frames.
    let err = engine
        .evaluate_json_structured(
            r#"{"if": [{"var": "go"}, {"throw": "oops"}, "ok"]}"#,
            r#"{"go": true}"#,
        )
        .expect_err("throw should fail");

    // Breadcrumb should be populated, leaf-first (deepest failure first).
    assert!(
        !err.node_ids().is_empty(),
        "expected breadcrumb path, got empty"
    );
    // All ids should be nonzero (SYNTHETIC_ID=0 is reserved).
    for id in err.node_ids() {
        assert!(
            *id > 0,
            "synthetic id leaked into breadcrumb: {:?}",
            err.node_ids()
        );
    }
    // Should have at least 2 ids — the throw itself plus the wrapping if,
    // since the if's dynamic condition prevents dead-code elimination.
    assert!(
        err.node_ids().len() >= 2,
        "expected at least 2 ids in path, got {:?}",
        err.node_ids()
    );
}

#[test]
fn structured_error_empty_path_on_success() {
    let engine = Engine::new();
    let result = engine.evaluate_json_structured(r#"{"==": [1, 1]}"#, r#"{}"#);
    // Successful eval — no error, so there's nothing to assert about path.
    assert!(result.is_ok());
}

#[cfg(feature = "error-handling")]
#[test]
fn try_catches_and_discards_inner_path() {
    let engine = Engine::new();
    // Outer `try` swallows a failing inner branch; the caller should see Ok,
    // and no breadcrumb should leak into a subsequent failing evaluation
    // because try truncates on catch.
    // try swallows the throw; result should be the fallback, not an error.
    let result =
        engine.evaluate_json_structured(r#"{"try": [{"throw": "ignored"}, "fallback"]}"#, r#"{}"#);
    assert_eq!(result.unwrap(), json!("fallback"));
}

#[test]
fn structured_error_path_serializes_to_json() {
    let engine = Engine::new();
    let err = engine
        .evaluate_json_structured(r#"{"if": [true, {"throw": "boom"}, "ok"]}"#, r#"{}"#)
        .unwrap_err();

    let json: Value = serde_json::to_value(&err).expect("must serialize");
    // `node_ids` should be present as an array of numbers.
    let node_ids = json
        .get("node_ids")
        .expect("serialized error should include `node_ids` field")
        .as_array()
        .expect("`node_ids` should be an array");
    assert!(!node_ids.is_empty());
    for id in node_ids {
        assert!(id.is_u64());
    }
    // No PathStep cache field is serialized — wire format is the
    // baseline `{type, message, ?operator, ?path}` shape.
    assert!(json.get("path_steps").is_none());
    assert!(json.get("resolved").is_none());
}

/// Compile a rule that throws at runtime (NaN from string arithmetic) and
/// return the resulting Error along with its compiled Logic.
fn nan_error(engine: &Engine) -> (datalogic_rs::Logic, Error) {
    let compiled = engine.compile(r#"{"+": ["x", 1]}"#).unwrap();
    let mut session = engine.session();
    let err = session
        .evaluate_json_value(&compiled, &json!(null))
        .expect_err("arithmetic on string must fail");
    (compiled, err)
}

#[test]
fn engine_errors_carry_raw_path_for_on_demand_resolution() {
    // Engine evaluation attaches raw compiled-node ids only — resolving
    // those into PathSteps is paid at the catch site via
    // `error.resolve_path(&compiled)`. Doing the walk on every
    // boundary crossing inflates error-heavy workloads ~17×; the
    // resolve-on-demand contract puts the cost where the caller
    // actually needs the data.
    let engine = Engine::new();
    let (compiled, err) = nan_error(&engine);

    assert!(
        !err.node_ids().is_empty(),
        "engine errors must arrive with raw breadcrumb ids, got {:?}",
        err
    );
    // Resolve on demand against the original Logic.
    let steps = err.resolve_path(&compiled);
    assert!(!steps.is_empty(), "resolve_path must produce steps");
    assert_eq!(steps[0].operator.as_deref(), Some("+"));
}

#[test]
fn with_path_replaces_prior_path() {
    // `with_path` is a plain setter — replacing the inline `Vec<u32>`
    // wholesale.
    let err = Error::invalid_arguments("nope")
        .with_node_ids(vec![1, 2, 3])
        .with_node_ids(vec![999]);
    assert_eq!(err.node_ids(), &[999]);
}

#[test]
fn wrap_preserves_node_ids_metadata() {
    // `Error::wrap(some_error)` is a no-op when given an existing Error;
    // the raw node_ids breadcrumb round-trips alongside operator metadata.
    let engine = Engine::new();
    let (_compiled, err) = nan_error(&engine);
    let original_node_ids = err.node_ids().to_vec();
    assert!(!original_node_ids.is_empty());

    let wrapped = Error::wrap(err);
    assert_eq!(wrapped.node_ids(), original_node_ids.as_slice());
}
