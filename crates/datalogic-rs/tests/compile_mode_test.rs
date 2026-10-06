//! `Engine::compile_template` and `Engine::compile_strict`: the templating
//! mode chosen per compile instead of per engine, so one engine (with one
//! set of custom operators) can check a rule strictly and compile an
//! output template.
//!
//! The reference is an engine built in the requested mode: compiling with
//! a mode must give exactly what compiling on such an engine gives.
#![cfg(all(feature = "templating", feature = "serde_json"))]

use datalogic_rs::{CustomOperator, DataValue, Engine, ErrorKind, Result, operator::EvalContext};

struct Double;

impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

/// Compile then evaluate, folding a compile error and an evaluation error
/// into one comparable outcome.
fn run(logic: Result<datalogic_rs::Logic>, engine: &Engine, data: &str) -> String {
    match logic {
        Err(e) => format!("compile err {}", e.tag()),
        Ok(l) => match engine.session().eval_str(&l, data) {
            Ok(v) => format!("ok {v}"),
            Err(e) => format!("err {}", e.tag()),
        },
    }
}

const TEMPLATE: &str = r#"{"name": {"var": "n"}, "double": {"double": [{"var": "x"}]}, "k": 1}"#;

#[test]
fn template_on_a_strict_engine() {
    let engine = Engine::builder().add_operator("double", Double).build();
    assert_eq!(
        run(engine.compile(TEMPLATE), &engine, r#"{"n": "a", "x": 2}"#),
        "compile err InvalidOperator"
    );
    assert_eq!(
        run(
            engine.compile_template(TEMPLATE),
            &engine,
            r#"{"n": "a", "x": 2}"#
        ),
        r#"ok {"name":"a","double":4,"k":1}"#
    );
}

#[test]
fn strict_on_a_templating_engine() {
    let engine = Engine::builder()
        .with_templating(true)
        .add_operator("double", Double)
        .build();
    assert_eq!(
        run(engine.compile(TEMPLATE), &engine, r#"{"n": "a", "x": 2}"#),
        r#"ok {"name":"a","double":4,"k":1}"#
    );
    let err = engine.compile_strict(TEMPLATE).err().unwrap();
    assert!(matches!(err.kind, ErrorKind::InvalidOperator(_)), "{err:?}");

    // Custom operators still resolve in strict mode.
    assert_eq!(
        run(engine.compile_strict(r#"{"double": 21}"#), &engine, "null"),
        "ok 42"
    );
    // A typo is an error in strict mode, an output field in templating.
    assert_eq!(
        run(engine.compile_strict(r#"{"doubel": 21}"#), &engine, "null"),
        run(
            Engine::new().compile(r#"{"doubel": 21}"#),
            &Engine::new(),
            "null"
        ),
    );
    assert_eq!(
        run(engine.compile(r#"{"doubel": 21}"#), &engine, "null"),
        r#"ok {"doubel":21}"#
    );
}

/// The escape prefix belongs to the engine and applies in template mode.
#[test]
fn template_uses_the_engine_escape() {
    let engine = Engine::builder().with_template_key_escape('$').build();
    assert_eq!(
        run(
            engine.compile_template(r#"{"$type": "x", "a": 1}"#),
            &engine,
            "null"
        ),
        r#"ok {"type":"x","a":1}"#
    );
}

#[test]
fn modes_honour_constant_folding() {
    let folding = Engine::new();
    assert!(
        folding
            .compile_strict(r#"{"+": [1, 2]}"#)
            .unwrap()
            .is_constant()
    );
    assert!(
        folding
            .compile_template(r#"{"+": [1, 2]}"#)
            .unwrap()
            .is_constant()
    );

    let no_fold = Engine::builder().with_constant_folding(false).build();
    assert!(
        !no_fold
            .compile_strict(r#"{"+": [1, 2]}"#)
            .unwrap()
            .is_constant()
    );
    assert!(
        !no_fold
            .compile_template(r#"{"+": [1, 2]}"#)
            .unwrap()
            .is_constant()
    );
}

#[cfg(feature = "trace")]
#[test]
fn a_template_compile_traces() {
    let engine = Engine::new();
    let logic = engine.compile_template(r#"{"a": {"var": "x"}}"#).unwrap();
    let run = engine.trace().eval(&logic, r#"{"x": 1}"#);
    assert_eq!(
        run.result.unwrap(),
        datalogic_rs::datavalue::OwnedDataValue::from_json(r#"{"a": 1}"#).unwrap()
    );
}

// ── every suite case, both directions ───────────────────────────────────

#[cfg(feature = "all-operators")]
mod suites {
    use super::*;
    use serde_json::Value;
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

    fn engine(templating: bool, escape: Option<char>, folding: bool) -> Engine {
        let mut b = Engine::builder()
            .with_templating(templating)
            .with_constant_folding(folding)
            .add_operator("double", Double);
        if let Some(c) = escape {
            b = b.with_template_key_escape(c);
        }
        b.build()
    }

    /// For every suite case, under every escape and folding setting the
    /// suites use: `compile_template` on a strict engine matches `compile`
    /// on a templating one, and `compile_strict` on a templating engine
    /// matches `compile` on a strict one. Rule shape (`to_json`) and
    /// outcome both.
    #[test]
    fn a_mode_matches_an_engine_built_in_it() {
        let mut files = Vec::new();
        suite_files(Path::new("tests/suites"), &mut files);
        files.sort();

        let mut checked = 0usize;
        let mut failures = Vec::new();
        let mut engines = std::collections::HashMap::new();

        for file in &files {
            let cases: Value =
                serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
            for (index, case) in cases.as_array().unwrap().iter().enumerate() {
                let Some(case) = case.as_object() else {
                    continue;
                };
                let rule = &case["rule"];
                let data = case.get("data").cloned().unwrap_or(Value::Null).to_string();
                let escape = case
                    .get("template_key_escape")
                    .and_then(Value::as_str)
                    .and_then(|s| s.chars().next());
                for folding in [true, false] {
                    let (strict, templating) =
                        engines.entry((escape, folding)).or_insert_with(|| {
                            (
                                engine(false, escape, folding),
                                engine(true, escape, folding),
                            )
                        });
                    let pairs = [
                        (
                            "compile_template",
                            strict.compile_template(rule),
                            templating.compile(rule),
                        ),
                        (
                            "compile_strict",
                            templating.compile_strict(rule),
                            strict.compile(rule),
                        ),
                    ];
                    for (mode, by_mode, by_engine) in pairs {
                        let shape = |l: &Result<datalogic_rs::Logic>| {
                            l.as_ref().map(|l| l.to_json()).map_err(|e| e.tag())
                        };
                        let (a, b) = (shape(&by_mode), shape(&by_engine));
                        // The engine that evaluates is the one in the mode's
                        // reference; evaluation does not read the flag.
                        let (ra, rb) = (run(by_mode, strict, &data), run(by_engine, strict, &data));
                        if a != b || ra != rb {
                            failures.push(format!(
                                "{}#{index} {mode} folding={folding}: {a:?} / {ra} vs {b:?} / {rb}",
                                file.display()
                            ));
                        }
                        checked += 1;
                    }
                }
            }
        }

        assert!(checked > 4000, "only {checked} checks");
        assert!(
            failures.is_empty(),
            "{} of {checked} failed:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// A traced compile reads the rule in the mode it is given, so the
/// debugger can trace a rule compiled with `compile_template` or
/// `compile_strict`.
#[test]
fn a_trace_compiles_in_the_mode_it_is_given() {
    use datalogic_rs::CheckMode;
    let template = r#"{"user": {"var": "name"}, "source": "api"}"#;
    let plain = Engine::new();
    assert!(plain.compile_template(template).is_ok());
    assert!(plain.trace().compile(template).is_err());
    let run = plain
        .trace()
        .with_mode(CheckMode::Template)
        .eval_str(template, r#"{"name": "a"}"#);
    assert_eq!(run.result.unwrap(), r#"{"user":"a","source":"api"}"#);
    assert!(!run.steps.is_empty());

    let templating = Engine::builder().with_templating(true).build();
    let typo = r#"{"sourec": "api"}"#;
    // Strictly, an unknown operator fails when it runs.
    let logic = templating.compile_strict(typo).unwrap();
    assert!(templating.session().eval_str(&logic, "null").is_err());
    assert!(templating.trace().eval_str(typo, "null").result.is_ok());
    let strict = templating.trace().with_mode(CheckMode::Strict);
    assert!(strict.eval_str(typo, "null").result.is_err());
    // A multi-key object does not compile strictly at all.
    assert!(strict.compile(template).is_err());
}
