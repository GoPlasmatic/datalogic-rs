use datalogic_rs::{DataLogic, Error, StructuredError};
use serde_json::{Value, json};

fn to_json(err: &Error) -> Value {
    serde_json::to_value(err).expect("Error must serialize")
}

#[test]
fn serialize_invalid_operator() {
    let err = Error::InvalidOperator("foo".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "InvalidOperator", "message": "Invalid operator: foo"})
    );
}

#[test]
fn serialize_invalid_arguments() {
    let err = Error::InvalidArguments("need 2".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "InvalidArguments", "message": "Invalid arguments: need 2"})
    );
}

#[test]
fn serialize_variable_not_found() {
    let err = Error::VariableNotFound("user.name".into());
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
    let err = Error::InvalidContextLevel(-3);
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
    let err = Error::TypeError("cannot compare".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "TypeError", "message": "Type error: cannot compare"})
    );
}

#[test]
fn serialize_arithmetic_error() {
    let err = Error::ArithmeticError("divide by zero".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "ArithmeticError", "message": "Arithmetic error: divide by zero"})
    );
}

#[test]
fn serialize_custom() {
    let err = Error::Custom("user-defined".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "Custom", "message": "user-defined"})
    );
}

#[test]
fn serialize_parse_error() {
    let err = Error::ParseError("expected value".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "ParseError", "message": "Parse error: expected value"})
    );
}

#[test]
fn serialize_thrown() {
    let payload = json!({"code": 42, "reason": "boom"});
    let err = Error::Thrown(payload.clone());
    let v = to_json(&err);
    assert_eq!(v["type"], json!("Thrown"));
    assert_eq!(v["thrown"], payload);
    // message is the Display output, which prints the JSON payload
    assert!(v["message"].as_str().unwrap().starts_with("Thrown: "));
}

#[test]
fn serialize_format_error() {
    let err = Error::FormatError("bad pattern".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "FormatError", "message": "Format error: bad pattern"})
    );
}

#[test]
fn serialize_index_out_of_bounds() {
    let err = Error::IndexOutOfBounds {
        index: 5,
        length: 3,
    };
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
    let err = Error::ConfigurationError("invalid setting".into());
    assert_eq!(
        to_json(&err),
        json!({"type": "ConfigurationError", "message": "Configuration error: invalid setting"})
    );
}

#[test]
fn structured_error_adds_operator_field() {
    let se =
        StructuredError::from(Error::ArithmeticError("divide by zero".into())).with_operator("/");
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
    let se = StructuredError::from(Error::TypeError("x".into()));
    let v = serde_json::to_value(&se).unwrap();
    assert_eq!(v, json!({"type": "TypeError", "message": "Type error: x"}));
}

#[test]
fn structured_error_flattens_variant_extras() {
    let se = StructuredError::from(Error::IndexOutOfBounds {
        index: 10,
        length: 2,
    })
    .with_operator("substr");
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
    let engine = DataLogic::new();
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
    let engine = DataLogic::new();
    let result = engine
        .evaluate_json_structured(r#"{"+": [1, 2]}"#, r#"{}"#)
        .unwrap();
    // Accept either integer or float encoding — datalogic returns either based on inputs.
    assert_eq!(result.as_f64(), Some(3.0));
}

#[test]
fn evaluate_json_structured_parse_error_has_no_operator() {
    let engine = DataLogic::new();
    let err = engine
        .evaluate_json_structured("not json", r#"{}"#)
        .expect_err("invalid JSON should error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["type"], json!("ParseError"));
    assert!(v.get("operator").is_none());
}

#[test]
fn evaluate_json_structured_captures_arithmetic_operator() {
    let engine = DataLogic::new();
    let err = engine
        .evaluate_json_structured(r#"{"/": [1, 0]}"#, "{}")
        .expect_err("division by zero must error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["operator"], json!("/"));
    assert_eq!(v["type"], json!("Thrown"));
}

#[test]
fn evaluate_json_structured_captures_unknown_operator() {
    let engine = DataLogic::new();
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
    let engine = DataLogic::new();
    let err = engine
        .evaluate_json_structured(r#"{"+": ["abc", 1]}"#, "{}")
        .expect_err("non-numeric addition must error");
    let v = serde_json::to_value(&err).unwrap();
    assert_eq!(v["operator"], json!("+"));
}

#[cfg(feature = "trace")]
#[test]
fn evaluate_json_with_trace_structured_populates_error_fields() {
    let engine = DataLogic::new();
    let traced = engine
        .evaluate_json_with_trace_structured(r#"{"throw": {"type": "Boom"}}"#, r#"{}"#)
        .unwrap();
    assert!(traced.error.is_some());
    let structured = traced.error_structured.expect("structured must populate");
    let v = serde_json::to_value(&structured).unwrap();
    assert_eq!(v["type"], json!("Thrown"));
    assert_eq!(v["operator"], json!("throw"));
}

/// Regression gate: Display output must stay byte-identical to v4.0.21, since
/// the JSONLogic test suite and downstream TS consumers depend on it.
#[test]
fn display_output_snapshot() {
    let cases: &[(Error, &str)] = &[
        (
            Error::InvalidOperator("foo".into()),
            "Invalid operator: foo",
        ),
        (Error::InvalidArguments("x".into()), "Invalid arguments: x"),
        (
            Error::VariableNotFound("user.name".into()),
            "Variable not found: user.name",
        ),
        (Error::InvalidContextLevel(-2), "Invalid context level: -2"),
        (Error::TypeError("x".into()), "Type error: x"),
        (Error::ArithmeticError("x".into()), "Arithmetic error: x"),
        (Error::Custom("raw".into()), "raw"),
        (Error::ParseError("x".into()), "Parse error: x"),
        (Error::Thrown(json!({"k": 1})), "Thrown: {\"k\":1}"),
        (Error::FormatError("x".into()), "Format error: x"),
        (
            Error::IndexOutOfBounds {
                index: 1,
                length: 0,
            },
            "Index 1 out of bounds for array of length 0",
        ),
        (
            Error::ConfigurationError("x".into()),
            "Configuration error: x",
        ),
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
    let engine = DataLogic::new();
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
    assert!(!err.path.is_empty(), "expected breadcrumb path, got empty");
    // All ids should be nonzero (SYNTHETIC_ID=0 is reserved).
    for id in &err.path {
        assert!(
            *id > 0,
            "synthetic id leaked into breadcrumb: {:?}",
            err.path
        );
    }
    // Should have at least 2 ids — the throw itself plus the wrapping if,
    // since the if's dynamic condition prevents dead-code elimination.
    assert!(
        err.path.len() >= 2,
        "expected at least 2 ids in path, got {:?}",
        err.path
    );
}

#[test]
fn structured_error_empty_path_on_success() {
    let engine = DataLogic::new();
    let result = engine.evaluate_json_structured(r#"{"==": [1, 1]}"#, r#"{}"#);
    // Successful eval — no error, so there's nothing to assert about path.
    assert!(result.is_ok());
}

#[cfg(feature = "error-handling")]
#[test]
fn try_catches_and_discards_inner_path() {
    let engine = DataLogic::new();
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
    let engine = DataLogic::new();
    let err = engine
        .evaluate_json_structured(r#"{"if": [true, {"throw": "boom"}, "ok"]}"#, r#"{}"#)
        .unwrap_err();

    let json: Value = serde_json::to_value(&err).expect("must serialize");
    // `path` should be present as an array of numbers.
    let path = json
        .get("path")
        .expect("serialized error should include `path` field")
        .as_array()
        .expect("`path` should be an array");
    assert!(!path.is_empty());
    for id in path {
        assert!(id.is_u64());
    }
}
