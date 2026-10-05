//! The JSON the bindings emit must match `schemas/*.v1.json`.
//!
//! A small validator for the subset of JSON Schema those files use
//! (`type`, `required`, `properties`, `items`, `enum`, `$ref`). It is
//! stricter than the standard in one way: an object may not carry a
//! property its schema does not list, so a new field fails here until the
//! schema file says so.

use datalogic_bind::{diagnostics_json, facts_json, operators_json, traced_run_json};
use datalogic_rs::{CheckMode, Engine};
use serde_json::Value;

const SCHEMAS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas");

fn schema(file: &str) -> Value {
    let text = std::fs::read_to_string(format!("{SCHEMAS}/{file}")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn type_ok(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        other => panic!("schema type {other}"),
    }
}

/// Validate `v` against `s`, resolving `$ref` within `root` or against a
/// sibling file. Returns the problems, each with its path.
fn validate(v: &Value, s: &Value, root: &Value, at: &str, out: &mut Vec<String>) {
    if let Some(r) = s.get("$ref").and_then(Value::as_str) {
        if let Some(local) = r.strip_prefix("#/$defs/") {
            return validate(v, &root["$defs"][local], root, at, out);
        }
        let other = schema(r);
        return validate(v, &other, &other, at, out);
    }
    match s.get("type") {
        Some(Value::String(t)) if !type_ok(t, v) => {
            out.push(format!("{at}: expected {t}, got {v}"));
            return;
        }
        Some(Value::Array(ts)) if !ts.iter().any(|t| type_ok(t.as_str().unwrap(), v)) => {
            out.push(format!("{at}: expected one of {ts:?}, got {v}"));
            return;
        }
        _ => {}
    }
    if let Some(allowed) = s.get("enum").and_then(Value::as_array)
        && !allowed.contains(v)
    {
        out.push(format!("{at}: {v} is not one of {allowed:?}"));
    }
    if let (Some(obj), Some(props)) = (
        v.as_object(),
        s.get("properties").and_then(Value::as_object),
    ) {
        for req in s
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !obj.contains_key(req.as_str().unwrap()) {
                out.push(format!("{at}: missing {req}"));
            }
        }
        for (k, child) in obj {
            match props.get(k) {
                Some(ps) => validate(child, ps, root, &format!("{at}/{k}"), out),
                None => out.push(format!("{at}: unexpected property {k}")),
            }
        }
    }
    if let (Some(items), Some(is)) = (v.as_array(), s.get("items")) {
        for (i, item) in items.iter().enumerate() {
            validate(item, is, root, &format!("{at}/{i}"), out);
        }
    }
}

fn assert_valid(file: &str, json: &str) {
    let s = schema(file);
    let v: Value = serde_json::from_str(json).unwrap();
    let mut problems = Vec::new();
    validate(&v, &s, &s, "", &mut problems);
    assert!(problems.is_empty(), "{file}: {problems:#?}\n{json}");
}

#[test]
fn operators_match_their_schema() {
    assert_valid("operators.v1.json", &operators_json(&Engine::new()));
}

#[test]
fn facts_match_their_schema() {
    let engine = Engine::new();
    for rule in [
        r#"{"+": [{"var": "a.b"}, {"var": "c"}]}"#,
        r#"{"var": {"cat": ["a", "b"]}}"#,
        r#"{"now": []}"#,
        r#"42"#,
    ] {
        assert_valid(
            "facts.v1.json",
            &facts_json(&engine.compile(rule).unwrap().facts()),
        );
    }
}

#[test]
fn diagnostics_match_their_schema() {
    let engine = Engine::new();
    for rule in [
        r#"{"if": [true, {"vr": 1}, {"map": [1]}]}"#,
        r#"{"a": 1, "b": 2}"#,
        r#"{"!": [1, 2]}"#,
        r#"{"format_date": [{"var": "d"}, "%Y", "Mars/Base"]}"#,
        "not json",
    ] {
        assert_valid(
            "diagnostics.v1.json",
            &diagnostics_json(&engine.check(rule, CheckMode::Engine)),
        );
    }
}

#[test]
fn traced_runs_match_their_schema() {
    let engine = Engine::new();
    for (rule, data) in [
        (
            r#"{"map": [{"var": "xs"}, {"+": [{"var": ""}, 1]}]}"#,
            r#"{"xs": [1, 2]}"#,
        ),
        (r#"{"+": ["a", 1]}"#, "null"),
        (
            r#"{"if": [{"var": "x"}, {"cat": ["a", "b"]}, 0]}"#,
            r#"{"x": true}"#,
        ),
        (r#"{"bogus": 1}"#, "null"),
        ("not json", "null"),
        (r#"{"throw": {"type": "x", "code": 7}}"#, "null"),
        (r#"{"var": "nope"}"#, "{}"),
    ] {
        let run = engine.trace().eval_str(rule, data);
        assert_valid("trace.v1.json", &traced_run_json(&run));
    }
}

#[test]
fn every_error_kind_matches_its_schema() {
    use datalogic_rs::{Error, ErrorKind};
    let errors = [
        Error::invalid_operator("x"),
        Error::invalid_arguments("x"),
        Error::variable_not_found("x"),
        Error::invalid_context_level(3),
        Error::type_error("x"),
        Error::arithmetic_error("x"),
        Error::custom_message("x"),
        Error::parse_error("x"),
        Error::thrown(datalogic_rs::datavalue::OwnedDataValue::Null),
        Error::format_error("x"),
        Error::index_out_of_bounds(4, 2),
        Error::configuration_error("x"),
        Error::from(ErrorKind::BudgetExceeded {
            budget: 1,
            spent: 2,
        })
        .with_operator("map"),
    ];
    for e in errors {
        assert_valid("error.v1.json", &serde_json::to_string(&e).unwrap());
    }
}
