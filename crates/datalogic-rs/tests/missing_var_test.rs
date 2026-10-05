//! `EvaluationConfig::missing_var`: whether a `var` / `val` read that finds
//! nothing is `null` (JSONLogic, the default) or an error.
//!
//! The error must fire on every read form and through every fast path that
//! reads fields without dispatching a `var` (filter comparisons, `map` and
//! `reduce` arithmetic, `sort` keys), and nowhere else: a default, a present
//! `null`, `missing`, `exists` and iteration metadata are not misses.
#![cfg(all(feature = "serde_json", feature = "all-operators"))]

use datalogic_rs::{Engine, ErrorCode, ErrorKind, EvaluationConfig, MissingVar};

fn strict() -> Engine {
    Engine::builder()
        .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
        .build()
}

fn eval(engine: &Engine, rule: &str, data: &str) -> Result<String, datalogic_rs::Error> {
    engine.eval_str(rule, data)
}

fn missing(engine: &Engine, rule: &str, data: &str) -> String {
    match eval(engine, rule, data) {
        Err(e) if e.code() == ErrorCode::VariableNotFound => match &e.kind {
            ErrorKind::VariableNotFound(name) => name.to_string(),
            _ => unreachable!(),
        },
        other => panic!("{rule} on {data}: expected VariableNotFound, got {other:?}"),
    }
}

const ROWS: &str = r#"{"rows": [{"k": 1, "n": 2}, {"n": 3}], "xs": [1, 2]}"#;

#[test]
fn the_default_is_null() {
    assert_eq!(EvaluationConfig::default().missing_var, MissingVar::Null);
    let engine = Engine::new();
    assert_eq!(eval(&engine, r#"{"var": "x"}"#, "{}").unwrap(), "null");
    assert_eq!(
        eval(&engine, r#"{"map": [{"var": "rows"}, {"var": "k"}]}"#, ROWS).unwrap(),
        "[1,null]"
    );
}

#[test]
fn every_read_form_raises_on_a_miss() {
    let e = strict();
    assert_eq!(missing(&e, r#"{"var": "x"}"#, "{}"), "x");
    assert_eq!(missing(&e, r#"{"var": "a.b"}"#, r#"{"a": {}}"#), "a.b");
    assert_eq!(missing(&e, r#"{"var": ["x"]}"#, "{}"), "x");
    assert_eq!(missing(&e, r#"{"var": 3}"#, "{}"), "3");
    assert_eq!(missing(&e, r#"{"val": "x"}"#, "{}"), "x");
    assert_eq!(missing(&e, r#"{"val": ["a", "b"]}"#, r#"{"a": {}}"#), "a.b");
    assert_eq!(missing(&e, r#"{"var": "xs.5"}"#, ROWS), "xs.5");
    // Computed paths.
    assert_eq!(missing(&e, r#"{"var": {"cat": ["x"]}}"#, "{}"), "x");
    assert_eq!(
        missing(&e, r#"{"val": [{"cat": ["a"]}, "b"]}"#, r#"{"a": {}}"#),
        "a.b"
    );
    assert_eq!(
        missing(&e, r#"{"var": [["a", "b"]]}"#, r#"{"a": {}}"#),
        "a.b"
    );
    // A miss against null data.
    assert_eq!(missing(&e, r#"{"var": "x"}"#, "null"), "x");
}

#[test]
fn a_level_read_raises_but_metadata_does_not() {
    let e = strict();
    // `[[1], "x"]` inside one `map` reads the root.
    assert_eq!(
        missing(
            &e,
            r#"{"map": [{"var": "xs"}, {"val": [[1], "nope"]}]}"#,
            ROWS
        ),
        "nope"
    );
    assert_eq!(
        missing(
            &e,
            r#"{"map": [{"var": "xs"}, {"val": [{"if": [true, [1], 0]}, "nope"]}]}"#,
            ROWS
        ),
        "nope"
    );
    // Iteration metadata is not a variable: outside an iterator it is null.
    assert_eq!(
        eval(&e, r#"{"val": [[1], "index"]}"#, "{}").unwrap(),
        "null"
    );
    assert_eq!(
        eval(
            &e,
            r#"{"map": [{"var": "xs"}, {"val": [[1], "index"]}]}"#,
            ROWS
        )
        .unwrap(),
        "[0,1]"
    );
}

#[test]
fn a_default_or_a_present_null_is_not_a_miss() {
    let e = strict();
    assert_eq!(eval(&e, r#"{"var": ["x", 0]}"#, "{}").unwrap(), "0");
    assert_eq!(eval(&e, r#"{"var": ["x", null]}"#, "{}").unwrap(), "null");
    assert_eq!(
        eval(&e, r#"{"var": [{"cat": ["x"]}, 7]}"#, "{}").unwrap(),
        "7"
    );
    assert_eq!(
        eval(&e, r#"{"var": "x"}"#, r#"{"x": null}"#).unwrap(),
        "null"
    );
    assert_eq!(
        eval(&e, r#"{"var": ""}"#, r#"{"a": 1}"#).unwrap(),
        r#"{"a":1}"#
    );
    assert_eq!(eval(&e, r#"{"val": []}"#, "5").unwrap(), "5");
}

#[test]
fn presence_tests_are_unaffected() {
    let e = strict();
    assert_eq!(
        eval(&e, r#"{"missing": ["x", "a"]}"#, r#"{"a": 1}"#).unwrap(),
        r#"["x"]"#
    );
    assert_eq!(
        eval(&e, r#"{"missing_some": [1, ["x", "a"]]}"#, r#"{"a": 1}"#).unwrap(),
        "[]"
    );
    assert_eq!(eval(&e, r#"{"exists": "x"}"#, "{}").unwrap(), "false");
}

#[test]
fn every_fast_path_raises_on_a_miss() {
    let e = strict();
    let rules = [
        // filter: strict comparison with a field, and the FastPredicate shapes
        r#"{"filter": [{"var": "rows"}, {"===": [{"var": "k"}, 1]}]}"#,
        r#"{"filter": [{"var": "rows"}, {"!==": [{"var": "k"}, 1]}]}"#,
        r#"{"filter": [{"var": "rows"}, {">": [{"var": "k"}, 0]}]}"#,
        r#"{"filter": [{"var": "rows"}, {"==": [{"var": "k"}, 1]}]}"#,
        r#"{"some": [{"var": "rows"}, {"<": [{"var": "k"}, 0]}]}"#,
        r#"{"all": [{"var": "rows"}, {">=": [{"var": "k"}, 0]}]}"#,
        r#"{"none": [{"var": "rows"}, {"==": [{"var": "k"}, 9]}]}"#,
        // map: a pluck, and arithmetic on fields
        r#"{"map": [{"var": "rows"}, {"var": "k"}]}"#,
        r#"{"map": [{"var": "rows"}, {"+": [{"var": "k"}, 1]}]}"#,
        r#"{"map": [{"var": "rows"}, {"*": [{"var": "k"}, {"var": "n"}]}]}"#,
        // reduce: over a field path, and over a map
        r#"{"reduce": [{"var": "rows"}, {"+": [{"var": "current.k"}, {"var": "accumulator"}]}, 0]}"#,
        r#"{"reduce": [{"map": [{"var": "rows"}, {"var": "k"}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
        // sort by a field
        r#"{"sort": [{"var": "rows"}, true, {"var": "k"}]}"#,
    ];
    for rule in rules {
        // The name is the path as written.
        let expected = if rule.contains("current.k") {
            "current.k"
        } else {
            "k"
        };
        assert_eq!(missing(&e, rule, ROWS), expected, "{rule}");
    }
}

#[test]
fn try_catches_a_miss() {
    let e = strict();
    assert_eq!(
        eval(&e, r#"{"try": [{"var": "x"}, "fallback"]}"#, "{}").unwrap(),
        r#""fallback""#
    );
    let message = eval(&e, r#"{"try": [{"var": "x"}, {"var": "type"}]}"#, "{}").unwrap();
    assert!(message.contains("x"), "{message}");
}

#[test]
fn the_json_config_sets_it() {
    let parse = |s: &str| EvaluationConfig::from_json_str(s);
    assert_eq!(
        parse(r#"{"missing_var": "error"}"#).unwrap().missing_var,
        MissingVar::Error
    );
    assert_eq!(
        parse(r#"{"missing_var": "null"}"#).unwrap().missing_var,
        MissingVar::Null
    );
    assert_eq!(parse("{}").unwrap().missing_var, MissingVar::Null);
    assert!(parse(r#"{"missing_var": "throw"}"#).is_err());
    assert!(parse(r#"{"missing_var": true}"#).is_err());
}
