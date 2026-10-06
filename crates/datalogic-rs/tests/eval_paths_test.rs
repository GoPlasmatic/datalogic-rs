//! Every evaluation entry point runs one shared body, and the one-shot
//! methods project an owned or `serde_json` input onto what the rule reads
//! as `Engine::evaluate` and `Session` do. Whatever the path, the result
//! is the same.
#![cfg(feature = "serde_json")]

use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{DataValue, Engine, Roots};

const DATA: &str = r#"{
    "a": {"b": 1, "c": [1, 2, 3], "d.e": "dotted"},
    "a.b": "flat",
    "xs": [{"n": 1}, {"n": 2}],
    "s": "text"
}"#;

const RULES: &[&str] = &[
    r#"{"var": "a.b"}"#,
    r#"{"val": "a.b"}"#,
    r#"{"val": ["a", "d.e"]}"#,
    r#"{"var": ["missing", "fallback"]}"#,
    r#"{"var": ""}"#,
    r#"{"map": [{"var": "xs"}, {"var": "n"}]}"#,
    r#"{"+": [{"var": "a.b"}, {"reduce": [{"var": "a.c"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}]}"#,
    r#"{"missing": ["a.b", "zz"]}"#,
    r#"{"cat": [{"var": "s"}, "-", {"var": "a.d.e"}]}"#,
    r#"{"var": {"cat": ["a", ".b"]}}"#,
];

#[test]
fn every_entry_point_agrees() {
    let engine = Engine::new();
    let owned = OwnedDataValue::from_json(DATA).unwrap();
    let serde: serde_json::Value = serde_json::from_str(DATA).unwrap();
    for rule in RULES {
        let compiled = engine.compile(*rule).unwrap();
        let arena = datalogic_rs::bumpalo::Bump::new();
        let parsed = DataValue::from_str(DATA, &arena).unwrap();
        let want = engine
            .evaluate(&compiled, parsed, &arena)
            .unwrap()
            .to_json_string();

        let got = [
            engine.eval(*rule, DATA).unwrap().to_json_string(),
            engine.eval(*rule, &owned).unwrap().to_json_string(),
            engine.eval(*rule, owned.clone()).unwrap().to_json_string(),
            engine.eval(*rule, &serde).unwrap().to_json_string(),
            datalogic_rs::eval(*rule, &owned).unwrap().to_json_string(),
            engine
                .evaluate(&compiled, &owned, &arena)
                .unwrap()
                .to_json_string(),
            engine
                .evaluate(&compiled, &serde, &arena)
                .unwrap()
                .to_json_string(),
            engine
                .session()
                .eval(&compiled, &owned)
                .unwrap()
                .to_json_string(),
            engine
                .session()
                .eval_borrowed(&compiled, &serde)
                .unwrap()
                .to_json_string(),
        ];
        // Compared as JSON values: a `serde_json` map (no
        // `preserve_order`) reorders keys.
        let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        for (i, got) in got.iter().enumerate() {
            assert_eq!(json(got), json(&want), "{rule}: path {i}");
        }
    }
}

#[test]
fn one_shot_roots_agree_with_evaluate() {
    let engine = Engine::new();
    let a = OwnedDataValue::from_json(r#"{"b": 1, "c": [1, 2]}"#).unwrap();
    let roots: Roots = [("a", &a)].into();
    for rule in [r#"{"var": "a.b"}"#, r#"{"var": "a"}"#, r#"{"var": "z"}"#] {
        let compiled = engine.compile(rule).unwrap();
        let arena = datalogic_rs::bumpalo::Bump::new();
        let want = engine
            .evaluate(&compiled, &roots, &arena)
            .unwrap()
            .to_json_string();
        let got = engine.eval(rule, &roots).unwrap().to_json_string();
        assert_eq!(got, want, "{rule}");
    }
}

/// A rule reading more keys than the projection scans linearly finds
/// each of them through the sorted search, in any read order.
#[test]
fn wide_read_sets_project_every_key() {
    let engine = Engine::new();
    let keys: Vec<String> = (0..40).map(|i| format!("k{:02}", (i * 7) % 40)).collect();
    let data = serde_json::Value::Object(
        (0..60)
            .map(|i| (format!("k{i:02}"), serde_json::json!(i)))
            .collect(),
    );
    let text = data.to_string();
    let owned = OwnedDataValue::from_json(&text).unwrap();
    let rule = serde_json::json!({
        "cat": keys.iter().map(|k| serde_json::json!({"var": k})).collect::<Vec<_>>()
    });
    let compiled = engine.compile(&rule).unwrap();
    let arena = datalogic_rs::bumpalo::Bump::new();
    let parsed = DataValue::from_str(&text, &arena).unwrap();
    let want = engine
        .evaluate(&compiled, parsed, &arena)
        .unwrap()
        .to_json_string();
    for got in [
        engine.evaluate(&compiled, &owned, &arena).unwrap(),
        engine.evaluate(&compiled, &data, &arena).unwrap(),
    ] {
        assert_eq!(got.to_json_string(), want);
    }
}
