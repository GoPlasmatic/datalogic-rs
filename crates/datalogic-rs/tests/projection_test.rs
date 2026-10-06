//! Read projection: evaluating a compiled rule against an owned or serde
//! value brings into the arena only the paths the rule reads
//! ([`Logic::facts`]), not a spine over the whole input. The result must
//! be exactly what evaluating the whole input gives, for every rule; a rule
//! whose reads are not all known (a computed path, a custom operator that
//! reads the context) or that reads the whole input is evaluated against
//! the whole input as before.

#![cfg(all(
    feature = "serde_json",
    feature = "templating",
    feature = "all-operators"
))]

use std::path::Path;

use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    ArenaExt, CustomOperator, CustomOperatorInfo, DataValue, Engine, EvaluationConfig, Logic,
    MissingVar, Result, Roots,
};
use serde_json::{Value, json};

fn outcome(r: Result<String>) -> String {
    match r {
        Ok(v) => format!("ok {v}"),
        Err(e) => format!("err {}", e.tag()),
    }
}

fn owned(v: &Value) -> OwnedDataValue {
    OwnedDataValue::from_json(&v.to_string()).unwrap()
}

/// The three ways in: JSON text (parsed whole), an owned value and a serde
/// value (both projected when the rule allows).
fn all_ways(engine: &Engine, logic: &Logic, data: &Value) -> [String; 3] {
    let text = data.to_string();
    let own = owned(data);
    let mut session = engine.session();
    let a = outcome(session.eval_str(logic, text.as_str()));
    session.reset();
    let b = outcome(session.eval_str(logic, &own));
    session.reset();
    let c = outcome(session.eval_str(logic, data));
    [a, b, c]
}

/// Builds an engine for a case's templating mode and key escape.
type MakeEngine = Box<dyn Fn(bool, Option<char>) -> Engine>;

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

/// Every suite case, on every engine flavour, gives the same outcome
/// through each way in.
#[test]
fn every_suite_case_agrees_projected_and_whole() {
    let mut files = Vec::new();
    suite_files(Path::new("tests/suites"), &mut files);
    files.sort();
    let flavours: Vec<(&str, MakeEngine)> = vec![
        (
            "default",
            Box::new(|t, e| {
                let mut b = Engine::builder().with_templating(t);
                if let Some(c) = e {
                    b = b.with_template_key_escape(c);
                }
                b.build()
            }),
        ),
        (
            "unfolded",
            Box::new(|t, e| {
                let mut b = Engine::builder()
                    .with_templating(t)
                    .with_constant_folding(false);
                if let Some(c) = e {
                    b = b.with_template_key_escape(c);
                }
                b.build()
            }),
        ),
        (
            "missing-var-error",
            Box::new(|t, e| {
                let mut b = Engine::builder()
                    .with_templating(t)
                    .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error));
                if let Some(c) = e {
                    b = b.with_template_key_escape(c);
                }
                b.build()
            }),
        ),
    ];
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for (name, make) in &flavours {
        let mut engines: std::collections::HashMap<(bool, Option<char>), Engine> =
            Default::default();
        for file in &files {
            let cases: Value =
                serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
            for (index, case) in cases.as_array().unwrap().iter().enumerate() {
                let Some(case) = case.as_object() else {
                    continue;
                };
                let data = case.get("data").cloned().unwrap_or(Value::Null);
                let templating = case
                    .get("templating")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let escape = case
                    .get("template_key_escape")
                    .and_then(Value::as_str)
                    .and_then(|s| s.chars().next());
                let engine = engines
                    .entry((templating, escape))
                    .or_insert_with(|| make(templating, escape));
                let Ok(logic) = engine.compile(&case["rule"]) else {
                    continue;
                };
                let [text, own, serde] = all_ways(engine, &logic, &data);
                if own != text || serde != text {
                    failures.push(format!(
                        "{name} {}#{index}: text {text}, owned {own}, serde {serde}",
                        file.display()
                    ));
                }
                checked += 1;
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(checked > 5000, "{checked}");
}

/// A wide input with one small field the rules read.
fn wide() -> Value {
    let mut fields = serde_json::Map::new();
    for i in 0..2000 {
        fields.insert(format!("k{i}"), json!({"a": [i, i + 1], "b": {"c": "x"}}));
    }
    fields.insert(
        "user".into(),
        json!({"name": "ada", "tags": ["x", "y"], "age": 36}),
    );
    Value::Object(fields)
}

/// The outcome of `rule` over `data` and the arena bytes it took, checked
/// against the same data given as JSON text (always viewed whole).
fn arena_bytes(engine: &Engine, rule: &str, data: &OwnedDataValue) -> (String, usize) {
    let logic = engine.compile(rule).unwrap();
    let mut session = engine.session();
    session.reset_with_capacity(64);
    let before = session.allocated_bytes();
    let out = outcome(session.eval_str(&logic, data));
    let bytes = session.allocated_bytes() - before;
    session.reset();
    let text = data.to_json_string();
    let whole = outcome(session.eval_str(&logic, text.as_str()));
    assert_eq!(out, whole, "{rule}: projected and whole differ");
    (out, bytes)
}

#[test]
fn a_rule_that_reads_one_field_brings_in_only_that_field() {
    let engine = Engine::new();
    let data = owned(&wide());
    let (out, projected) = arena_bytes(&engine, r#"{"var": "user.name"}"#, &data);
    assert_eq!(out, r#"ok "ada""#);
    // The whole input as a view: what a rule that reads the root costs.
    let (_, whole) = arena_bytes(&engine, r#"{"var": ""}"#, &data);
    assert!(
        projected * 50 < whole,
        "projected {projected} bytes, whole {whole} bytes"
    );
}

#[test]
fn a_read_of_a_container_brings_in_all_of_it() {
    let engine = Engine::new();
    let data = owned(&wide());
    let (out, _) = arena_bytes(
        &engine,
        r#"{"map": [{"var": "user.tags"}, {"cat": [{"var": ""}, "!"]}]}"#,
        &data,
    );
    assert_eq!(out, r#"ok ["x!","y!"]"#);
    let (out, _) = arena_bytes(&engine, r#"{"keys": {"var": "user"}}"#, &data);
    // `wide()` is built through a sorted serde map.
    assert_eq!(out, r#"ok ["age","name","tags"]"#);
    let (out, _) = arena_bytes(&engine, r#"{"length": {"var": "k7.a"}}"#, &data);
    assert_eq!(out, "ok 2");
}

#[test]
fn a_path_through_an_array_reads_the_whole_array() {
    let engine = Engine::new();
    let data = owned(&json!({"rows": [{"v": 1}, {"v": 2}], "other": 1}));
    for (rule, want) in [
        (r#"{"var": "rows.1.v"}"#, "ok 2"),
        (r#"{"var": "rows.5.v"}"#, "ok null"),
        (r#"{"missing": ["rows.0.v", "rows.3"]}"#, r#"ok ["rows.3"]"#),
        // `exists` does not index arrays, on the whole input either.
        (r#"{"exists": ["rows", "0"]}"#, "ok false"),
        (r#"{"exists": ["rows"]}"#, "ok true"),
    ] {
        let (out, _) = arena_bytes(&engine, rule, &data);
        assert_eq!(out, want, "{rule}");
    }
}

#[test]
fn a_missing_path_stays_missing() {
    let strict = Engine::builder()
        .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
        .build();
    let data = owned(&json!({"a": {"b": 1}}));
    let (out, _) = arena_bytes(&strict, r#"{"var": "a.c"}"#, &data);
    assert_eq!(out, "err VariableNotFound");
    let (out, _) = arena_bytes(&Engine::new(), r#"{"missing": ["a.b", "a.c", "z"]}"#, &data);
    assert_eq!(out, r#"ok ["a.c","z"]"#);
}

#[test]
fn a_scoped_read_of_the_root_is_brought_in() {
    // `val` climbing out of two iterations reads the root.
    let engine = Engine::new();
    let data = owned(&json!({"rate": 2, "rows": [[1, 2], [3]], "noise": {"x": 1}}));
    let (out, _) = arena_bytes(
        &engine,
        r#"{"map": [{"var": "rows"}, {"map": [{"var": ""}, {"*": [{"var": ""}, {"val": [[4], "rate"]}]}]}]}"#,
        &data,
    );
    assert_eq!(out, "ok [[2,4],[6]]");
}

#[test]
fn a_computed_path_reads_the_whole_input() {
    let engine = Engine::new();
    let data = owned(&json!({"which": "b", "a": 1, "b": 2}));
    let (out, _) = arena_bytes(&engine, r#"{"var": {"var": "which"}}"#, &data);
    assert_eq!(out, "ok 2");
}

/// Reads `user.name` through the context, without declaring it.
struct ReadsContext;

impl CustomOperator for ReadsContext {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let root = ctx.root_input();
        Ok(root
            .get("user")
            .and_then(|u| u.get("name"))
            .unwrap_or_else(|| arena.null()))
    }
}

/// Declares that it reads nothing but its arguments.
struct Pure;

impl CustomOperator for Pure {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(args[0])
    }

    fn info(&self) -> CustomOperatorInfo {
        CustomOperatorInfo::pure()
    }
}

#[test]
fn a_custom_operator_that_reads_the_context_sees_all_of_it() {
    let engine = Engine::builder()
        .add_operator("who", ReadsContext)
        .add_operator("id", Pure)
        .build();
    let data = owned(&json!({"user": {"name": "ada"}, "x": 1}));
    let (out, _) = arena_bytes(&engine, r#"[{"who": []}, {"var": "x"}]"#, &data);
    assert_eq!(out, r#"ok ["ada",1]"#);
    let (out, _) = arena_bytes(&engine, r#"{"id": [{"var": "x"}]}"#, &data);
    assert_eq!(out, "ok 1");
}

#[test]
fn a_template_reads_through_projection() {
    let engine = Engine::builder().with_templating(true).build();
    let data = owned(&json!({"user": {"name": "ada", "age": 36}, "noise": [1, 2, 3]}));
    let (out, _) = arena_bytes(
        &engine,
        r#"{"who": {"var": "user.name"}, "next": {"+": [{"var": "user.age"}, 1]}}"#,
        &data,
    );
    assert_eq!(out, r#"ok {"who":"ada","next":37}"#);
}

#[test]
fn duplicate_keys_resolve_as_on_the_whole_input() {
    // An owned object can carry a key twice; a read sees what it would see
    // on the whole input.
    let data = OwnedDataValue::Object(vec![
        ("a".into(), OwnedDataValue::from(1i64)),
        ("b".into(), OwnedDataValue::from(2i64)),
        ("a".into(), OwnedDataValue::from(3i64)),
    ]);
    let engine = Engine::new();
    let logic = engine.compile(r#"{"var": "a"}"#).unwrap();
    let projected = outcome(engine.session().eval_str(&logic, &data));
    let whole = engine.compile(r#"[{"var": "a"}, {"var": ""}]"#).unwrap();
    let both = outcome(engine.session().eval_str(&whole, &data));
    let whole_a = both
        .strip_prefix("ok [")
        .and_then(|s| s.split(',').next())
        .unwrap()
        .to_string();
    assert_eq!(projected, format!("ok {whole_a}"));
}

#[test]
fn a_non_object_input_is_unchanged() {
    let engine = Engine::new();
    for (rule, data, want) in [
        (r#"{"var": "1"}"#, json!([10, 20]), "ok 20"),
        (r#"{"var": ""}"#, json!(5), "ok 5"),
        (r#"{"var": "a"}"#, json!("str"), "ok null"),
    ] {
        let (out, _) = arena_bytes(&engine, rule, &owned(&data));
        assert_eq!(out, want, "{rule}");
    }
}

/// Every suite case whose data is an object, split into roots (alternating
/// owned and serde values), gives what the whole object gives.
#[test]
fn roots_are_projected_too() {
    let mut files = Vec::new();
    suite_files(Path::new("tests/suites"), &mut files);
    files.sort();
    let mut engines: std::collections::HashMap<(bool, Option<char>), Engine> = Default::default();
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for file in &files {
        let cases: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
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
            let owned_parts: Vec<OwnedDataValue> = fields.values().map(owned).collect();
            let mut roots = Roots::new();
            for (i, ((name, json), own)) in fields.iter().zip(owned_parts.iter()).enumerate() {
                roots = if i % 2 == 0 {
                    roots.root(name, json)
                } else {
                    roots.root(name, own)
                };
            }
            let text = Value::Object(fields.clone()).to_string();
            let mut session = engine.session();
            let whole = outcome(session.eval_str(&logic, text.as_str()));
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
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(checked > 1000, "{checked}");
}

#[test]
fn roots_bring_in_only_what_is_read() {
    let engine = Engine::new();
    let big = owned(&wide());
    let meta = json!({"tenant": "acme", "region": "eu"});
    let logic = engine
        .compile(r#"[{"var": "data.user.name"}, {"var": "metadata.tenant"}]"#)
        .unwrap();
    let roots = Roots::new().root("data", &big).root("metadata", &meta);
    let mut session = engine.session();
    session.reset_with_capacity(64);
    let before = session.allocated_bytes();
    assert_eq!(
        session.eval_str(&logic, &roots).unwrap(),
        r#"["ada","acme"]"#
    );
    let projected = session.allocated_bytes() - before;
    session.reset_with_capacity(64);
    let whole_logic = Engine::new()
        .compile(r#"[{"var": "data.user.name"}, {"var": "metadata.tenant"}]"#)
        .unwrap();
    let before = session.allocated_bytes();
    assert_eq!(
        session.eval_str(&whole_logic, &roots).unwrap(),
        r#"["ada","acme"]"#
    );
    let whole = session.allocated_bytes() - before;
    assert!(
        projected * 50 < whole,
        "projected {projected}, whole {whole}"
    );
}

/// A read path is as long as the rule makes it; one far past any sane
/// nesting is read from the whole input rather than recursing once per
/// segment to build its projection.
#[test]
fn a_very_long_read_path_does_not_overflow() {
    let engine = Engine::new();
    let path = vec!["a"; 50_000].join(".");
    let logic = engine.compile(&json!({ "var": path })).unwrap();
    let data = json!({"a": {"a": 1}});
    assert_eq!(
        all_ways(&engine, &logic, &data),
        ["ok null", "ok null", "ok null"]
    );
}

/// A wide object that repeats a key the rule reads gives the same value
/// projected as whole: wide objects are looked up by an ordered probe,
/// which may find a later copy of the key than a first-match scan.
#[test]
fn a_repeated_key_in_a_wide_object_reads_as_on_the_whole_input() {
    let engine = Engine::new();
    for fields in [4, 31, 40] {
        let mut text = String::from(r#"{"k":1"#);
        for i in 0..fields {
            text.push_str(&format!(r#","f{i}":null"#));
        }
        text.push_str(r#","k":2}"#);
        let own = OwnedDataValue::from_json(&text).unwrap();
        for rule in [json!({"var": "k"}), json!([{"var": "k"}, {"var": "f0"}])] {
            let logic = engine.compile(&rule).unwrap();
            let whole = outcome(engine.session().eval_str(&logic, text.as_str()));
            let projected = outcome(engine.session().eval_str(&logic, &own));
            assert_eq!(projected, whole, "{rule} over {fields} fields");
        }
    }
}
