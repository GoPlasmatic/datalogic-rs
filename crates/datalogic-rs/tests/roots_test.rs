//! `Roots`: several values evaluated as one top-level object, without
//! building that object first.
//!
//! The reference for every case is the combined value a host would have
//! built by hand (`json!({"data": data, "metadata": metadata})`): a rule
//! evaluated against `Roots` must give exactly what it gives against that
//! object, through every entry point that takes input.
#![cfg(feature = "serde_json")]

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{Engine, OwnedInput, ParsedData, Roots};
use serde_json::{Value, json};

fn owned(v: &Value) -> OwnedDataValue {
    OwnedDataValue::from_json(&v.to_string()).unwrap()
}

const RULE: &str = r#"{"if": [
    {"and": [{"var": "data.ok"}, {"==": [{"var": "metadata.channel"}, "web"]}]},
    {"cat": [{"var": "data.user.name"}, "@", {"var": "metadata.channel"}]},
    "denied"
]}"#;

#[test]
fn reads_each_root_by_name() {
    let data = json!({"ok": true, "user": {"name": "ana"}});
    let metadata = owned(&json!({"channel": "web"}));
    let roots = Roots::new().root("data", &data).root("metadata", &metadata);

    let engine = Engine::new();
    assert_eq!(engine.eval_str(RULE, &roots).unwrap(), r#""ana@web""#);
}

/// Every entry point that takes input gives the same answer as the
/// combined object, by reference and by value.
#[test]
fn every_entry_point_matches_the_combined_object() {
    let data = json!({"ok": true, "user": {"name": "ana"}});
    let metadata = json!({"channel": "web"});
    let combined = json!({"data": data, "metadata": metadata});
    let expected = r#""ana@web""#;
    let roots = || Roots::from([("data", &data), ("metadata", &metadata)]);

    let engine = Engine::new();
    let logic = engine.compile(RULE).unwrap();
    assert_eq!(engine.eval_str(RULE, &combined).unwrap(), expected);

    // One-shot (`OwnedInput`), by reference and by value.
    let r = roots();
    assert_eq!(engine.eval_str(RULE, &r).unwrap(), expected);
    assert_eq!(engine.eval_str(RULE, roots()).unwrap(), expected);
    assert_eq!(
        engine.eval(RULE, &r).unwrap(),
        engine.eval(RULE, &combined).unwrap()
    );
    let v: Value = engine.eval_into(RULE, &r).unwrap();
    assert_eq!(v, json!("ana@web"));
    assert_eq!(datalogic_rs::eval_str(RULE, &r).unwrap(), expected);

    // Session (`EvalInput`).
    let mut session = engine.session();
    assert_eq!(session.eval_str(&logic, &r).unwrap(), expected);
    session.reset();
    assert_eq!(session.eval_str(&logic, roots()).unwrap(), expected);
    session.reset();
    let v: Value = session.eval_into(&logic, &r).unwrap();
    assert_eq!(v, json!("ana@web"));
    session.reset();
    assert_eq!(
        session.eval(&logic, &r).unwrap(),
        engine.eval(RULE, &combined).unwrap()
    );
    session.reset();
    let borrowed = session.eval_borrowed(&logic, &r).unwrap();
    assert_eq!(borrowed.as_str(), Some("ana@web"));

    // Raw tier with a caller arena.
    let arena = Bump::new();
    let out = engine.evaluate(&logic, &r, &arena).unwrap();
    assert_eq!(out.as_str(), Some("ana@web"));

    // Traced runs.
    #[cfg(feature = "trace")]
    {
        let traced = engine.trace().eval_str(RULE, &r);
        assert_eq!(traced.result.unwrap(), expected);
    }
}

/// Each root may be any input representation the engine takes by
/// reference.
#[test]
fn mixed_representations() {
    let a = json!({"x": 1});
    let b = owned(&json!({"x": 2}));
    let c = ParsedData::from_json(r#"{"x": 3}"#).unwrap();
    let d_source = ParsedData::from_json(r#"{"x": 4}"#).unwrap();
    let d = d_source.value();

    let roots = Roots::new()
        .root("a", &a)
        .root("b", &b)
        .root("c", &c)
        .root("d", d);
    let engine = Engine::new();
    assert_eq!(
        engine
            .eval_str(
                r#"[{"var": "a.x"}, {"var": "b.x"}, {"var": "c.x"}, {"var": "d.x"}]"#,
                &roots
            )
            .unwrap(),
        "[1,2,3,4]"
    );
    assert_eq!(
        engine.eval_str(r#"{"var": ""}"#, &roots).unwrap(),
        r#"{"a":{"x":1},"b":{"x":2},"c":{"x":3},"d":{"x":4}}"#
    );
}

/// A root may be any value, not only an object.
#[test]
fn scalar_and_array_roots() {
    let n = json!(41);
    let xs = json!([1, 2, 3]);
    let roots = Roots::from([("n", &n), ("xs", &xs)]);
    let engine = Engine::new();
    assert_eq!(
        engine
            .eval_str(r#"{"+": [{"var": "n"}, {"var": "xs.0"}]}"#, &roots)
            .unwrap(),
        "42"
    );
    assert_eq!(
        engine
            .eval_str(r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#, &roots)
            .unwrap(),
        "6"
    );
}

/// Names keep the order they were first given in; a repeated name replaces
/// the earlier value in place.
#[test]
fn repeated_name_replaces_in_place() {
    let one = json!(1);
    let two = json!(2);
    let three = json!(3);
    let roots = Roots::new()
        .root("a", &one)
        .root("b", &two)
        .root("a", &three);
    assert_eq!(roots.len(), 2);
    let engine = Engine::new();
    assert_eq!(
        engine.eval_str(r#"{"var": ""}"#, &roots).unwrap(),
        r#"{"a":3,"b":2}"#
    );
    assert_eq!(engine.eval_str(r#"{"var": "a"}"#, &roots).unwrap(), "3");
}

#[test]
fn empty_and_missing_roots() {
    let engine = Engine::new();
    let empty = Roots::new();
    assert!(empty.is_empty());
    assert_eq!(engine.eval_str(r#"{"var": ""}"#, &empty).unwrap(), "{}");
    assert_eq!(
        engine.eval_str(r#"{"var": "data.x"}"#, &empty).unwrap(),
        "null"
    );

    let data = json!({"x": 1});
    let roots = Roots::from([("data", &data)]);
    assert_eq!(
        engine.eval_str(r#"{"var": "claims.sub"}"#, &roots).unwrap(),
        "null"
    );
    assert_eq!(
        engine
            .eval_str(r#"{"missing": ["data.x", "claims.sub"]}"#, &roots)
            .unwrap(),
        r#"["claims.sub"]"#
    );
}

/// A level marker inside an iterator climbs back to the combined root.
#[test]
fn iteration_reaches_other_roots() {
    let data = json!({"items": [1, 2, 3]});
    let metadata = json!({"rate": 10});
    let roots = Roots::from([("data", &data), ("metadata", &metadata)]);
    let engine = Engine::new();
    let rule = r#"{"map": [{"var": "data.items"}, {"*": [{"var": ""}, {"val": [[1], "metadata", "rate"]}]}]}"#;
    assert_eq!(engine.eval_str(rule, &roots).unwrap(), "[10,20,30]");
}

/// One `Roots` serves any number of evaluations.
#[test]
fn reused_across_evaluations() {
    let data = json!({"n": 2});
    let roots = Roots::from([("data", &data)]);
    let engine = Engine::new();
    let logic = engine.compile(r#"{"*": [{"var": "data.n"}, 21]}"#).unwrap();
    let mut session = engine.session();
    for _ in 0..100 {
        assert_eq!(session.eval_str(&logic, &roots).unwrap(), "42");
        session.reset();
    }
}

#[test]
fn into_owned_input_is_the_combined_object() {
    let data = json!({"ok": true});
    let metadata = owned(&json!({"channel": "web"}));
    let parsed = ParsedData::from_json("[1, 2]").unwrap();
    let roots = Roots::new()
        .root("data", &data)
        .root("metadata", &metadata)
        .root("list", &parsed);
    assert_eq!(
        (&roots).into_owned_input().unwrap(),
        owned(&json!({"data": {"ok": true}, "metadata": {"channel": "web"}, "list": [1, 2]}))
    );
    assert_eq!(
        roots.into_owned_input().unwrap(),
        owned(&json!({"data": {"ok": true}, "metadata": {"channel": "web"}, "list": [1, 2]}))
    );
}

#[test]
fn names() {
    let a = json!(1);
    let roots = Roots::new().root("data", &a).root("metadata", &a);
    assert_eq!(roots.names().collect::<Vec<_>>(), ["data", "metadata"]);
}

// ── every suite case, split into roots ──────────────────────────────────

#[cfg(all(feature = "templating", feature = "all-operators"))]
mod suites {
    use super::*;
    use std::path::Path;

    fn suite_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                suite_files(&path, out);
            } else if path.extension().is_some_and(|x| x == "json")
                && path.file_name().is_some_and(|n| n != "index.json")
            {
                out.push(path);
            }
        }
    }

    fn outcome(r: datalogic_rs::Result<String>) -> String {
        match r {
            Ok(v) => format!("ok {v}"),
            Err(e) => format!("err {}", e.tag()),
        }
    }

    /// Every suite case whose data is an object, with each top-level key
    /// passed as its own root (alternating serde and owned values), gives
    /// the outcome the whole object gives.
    #[test]
    fn split_data_matches_whole_data() {
        let mut files = Vec::new();
        suite_files(Path::new("tests/suites"), &mut files);
        files.sort();

        let mut engines: std::collections::HashMap<(bool, Option<char>), Engine> =
            Default::default();
        let mut checked = 0usize;
        let mut failures = Vec::new();

        for file in &files {
            let cases: Value =
                serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
            for (index, case) in cases.as_array().unwrap().iter().enumerate() {
                let Some(case) = case.as_object() else {
                    continue;
                };
                let Some(Value::Object(fields)) = case.get("data") else {
                    continue;
                };
                let templating = case
                    .get("templating")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let escape = case
                    .get("template_key_escape")
                    .and_then(Value::as_str)
                    .and_then(|s| s.chars().next());
                let engine = engines.entry((templating, escape)).or_insert_with(|| {
                    let mut b = Engine::builder().with_templating(templating);
                    if let Some(c) = escape {
                        b = b.with_template_key_escape(c);
                    }
                    b.build()
                });
                let Ok(logic) = engine.compile(&case["rule"]) else {
                    continue;
                };

                let owned_parts: Vec<(String, OwnedDataValue)> =
                    fields.iter().map(|(k, v)| (k.clone(), owned(v))).collect();
                let mut roots = Roots::new();
                for (i, ((name, json), (_, own))) in
                    fields.iter().zip(owned_parts.iter()).enumerate()
                {
                    roots = if i % 2 == 0 {
                        roots.root(name, json)
                    } else {
                        roots.root(name, own)
                    };
                }

                let data = Value::Object(fields.clone());
                let mut session = engine.session();
                let whole = outcome(session.eval_str(&logic, &data));
                session.reset();
                let split = outcome(session.eval_str(&logic, &roots));
                if whole != split {
                    failures.push(format!(
                        "{}#{index}: whole {whole}, roots {split}",
                        file.display()
                    ));
                }
                checked += 1;
            }
        }

        assert!(checked > 1000, "only {checked} cases checked");
        assert!(
            failures.is_empty(),
            "{} of {checked} failed:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
