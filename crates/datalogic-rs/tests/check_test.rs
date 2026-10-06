//! `Engine::check`: every problem the engine can see in a rule before it
//! runs, collected in one pass, each with a code, a severity and an
//! RFC 6901 pointer into the rule. `Engine::compile_checked` refuses a rule
//! with any error.
#![cfg(all(
    feature = "serde_json",
    feature = "templating",
    feature = "all-operators"
))]

use datalogic_rs::{
    ArenaExt, CheckMode, CustomOperator, CustomOperatorInfo, DataValue, Diagnostic, DiagnosticCode,
    Engine, Result, Severity, operator::EvalContext,
};
use serde_json::{Value, json};

fn check(engine: &Engine, rule: Value) -> Vec<Diagnostic> {
    engine.check(&rule, CheckMode::Engine)
}

/// `(code, severity, pointer)` for each diagnostic, in order.
fn summary(diags: &[Diagnostic]) -> Vec<(DiagnosticCode, Severity, String)> {
    diags
        .iter()
        .map(|d| (d.code, d.severity, d.pointer.clone()))
        .collect()
}

use DiagnosticCode::*;
use Severity::*;

fn s(p: &str) -> String {
    p.to_string()
}

// ── clean rules ─────────────────────────────────────────────────────────

#[test]
fn a_well_formed_rule_has_no_diagnostics() {
    let engine = Engine::new();
    for rule in [
        json!({"if": [{"var": "a"}, {"+": [1, 2]}, "no"]}),
        json!({"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]}),
        json!({"and": [true, {"!": false}]}),
        json!(42),
        json!([1, {"var": "x"}]),
        json!({}),
        json!({"var": ["a", 0]}),
    ] {
        assert!(check(&engine, rule.clone()).is_empty(), "{rule}");
        assert!(engine.compile_checked(&rule).is_ok(), "{rule}");
    }
}

#[test]
fn every_suite_rule_without_an_error_expectation_checks_clean_of_errors() {
    // A rule the suites expect to produce a value must not draw an error
    // diagnostic in its own mode: check reports only what will fail. The
    // exception is a failure a `try` catches, which still fails where it
    // is.
    let index = std::fs::read_to_string("tests/suites/index.json").unwrap();
    let files: Vec<String> = serde_json::from_str(&index).unwrap();
    let strict = Engine::new();
    let templating = Engine::builder().with_templating(true).build();
    let mut checked = 0;
    for file in files {
        let text = std::fs::read_to_string(format!("tests/suites/{file}")).unwrap();
        let suite: Value = serde_json::from_str(&text).unwrap();
        for case in suite.as_array().unwrap().iter().filter(|c| c.is_object()) {
            if case.get("error").is_some() || case.get("template_key_escape").is_some() {
                continue;
            }
            let engine = if case.get("templating").and_then(Value::as_bool) == Some(true) {
                &templating
            } else {
                &strict
            };
            let errors: Vec<_> = check(engine, case["rule"].clone())
                .into_iter()
                .filter(|d| d.severity == Error && !d.pointer.contains("/try/"))
                .collect();
            assert!(
                errors.is_empty(),
                "{file}: {}: {errors:?}",
                case["description"]
            );
            checked += 1;
        }
    }
    assert!(checked > 1000);
}

// ── errors ──────────────────────────────────────────────────────────────

#[test]
fn an_unknown_operator_is_an_error_with_a_suggestion() {
    let engine = Engine::new();
    let diags = check(&engine, json!({"if": [true, {"vr": "x"}, 0]}));
    assert_eq!(summary(&diags), [(UnknownOperator, Error, s("/if/1"))]);
    assert_eq!(diags[0].operator.as_deref(), Some("vr"));
    assert!(diags[0].message.contains("`var`"), "{}", diags[0].message);
    // No suggestion when nothing is close.
    let diags = check(&engine, json!({"zzzzz": []}));
    assert!(
        !diags[0].message.contains("did you mean"),
        "{}",
        diags[0].message
    );
}

#[test]
fn a_registered_custom_operator_is_known() {
    let engine = Engine::builder().add_operator("double", Plain).build();
    assert!(check(&engine, json!({"double": [1]})).is_empty());
    let diags = check(&Engine::new(), json!({"double": [1]}));
    assert_eq!(summary(&diags), [(UnknownOperator, Error, s(""))]);
}

#[test]
fn a_multi_key_object_is_an_error_outside_templating() {
    let diags = check(&Engine::new(), json!({"map": [[1], {"a": 1, "b": 2}]}));
    assert_eq!(summary(&diags), [(NotAnOperator, Error, s("/map/1"))]);
}

#[test]
fn and_or_if_need_an_argument_array() {
    let diags = check(
        &Engine::new(),
        json!({"or": [{"and": true}, {"if": {"var": "x"}}]}),
    );
    assert_eq!(
        summary(&diags),
        [
            (ArgumentForm, Error, s("/or/0")),
            (ArgumentForm, Error, s("/or/1")),
        ]
    );
}

#[test]
fn an_argument_count_the_operator_rejects_is_an_error() {
    let engine = Engine::new();
    assert_eq!(
        summary(&check(&engine, json!({"map": [{"var": "xs"}]}))),
        [(ArgumentCount, Error, s(""))]
    );
    assert_eq!(
        summary(&check(&engine, json!({"filter": [[1], true, 3]}))),
        [(ArgumentCount, Error, s(""))]
    );
    // A fixed-arity operator with an error for too few.
    assert_eq!(
        summary(&check(&engine, json!({"datetime": []}))),
        [(ArgumentCount, Error, s(""))]
    );
}

#[test]
fn a_custom_operator_declared_count_is_checked() {
    let engine = Engine::builder()
        .add_operator(
            "one",
            Declared(CustomOperatorInfo::opaque().with_args(1, Some(1))),
        )
        .build();
    assert!(check(&engine, json!({"one": [1]})).is_empty());
    assert_eq!(
        summary(&check(&engine, json!({"one": [1, 2]}))),
        [(ArgumentCount, Error, s(""))]
    );
    assert_eq!(
        summary(&check(&engine, json!({"one": []}))),
        [(ArgumentCount, Error, s(""))]
    );
}

#[cfg(feature = "datetime")]
#[test]
fn a_literal_unknown_timezone_is_an_error() {
    let diags = check(
        &Engine::new(),
        json!({"format_date": [{"var": "d"}, "%Y", "Mars/Olympus"]}),
    );
    assert_eq!(
        summary(&diags),
        [(InvalidTimezone, Error, s("/format_date/2"))]
    );
    assert!(
        check(
            &Engine::new(),
            json!({"format_date": [{"var": "d"}, "%Y", "Asia/Kolkata"]})
        )
        .is_empty()
    );
}

// ── warnings ────────────────────────────────────────────────────────────

#[test]
fn ignored_extra_arguments_are_a_warning() {
    let diags = check(&Engine::new(), json!({"!": [true, {"var": "x"}]}));
    assert_eq!(summary(&diags), [(ArgumentCount, Warning, s(""))]);
    assert!(
        Engine::new()
            .compile_checked(&json!({"!": [true, 1]}))
            .is_ok()
    );
}

#[test]
fn a_template_key_like_an_operator_is_a_warning() {
    let engine = Engine::builder().with_templating(true).build();
    let diags = check(&engine, json!({"out": {"vr": "x"}, "n": 1}));
    assert_eq!(summary(&diags), [(SimilarToOperator, Warning, s("/out"))]);
    assert!(diags[0].message.contains("`var`"), "{}", diags[0].message);
    // A key nothing like an operator is just a field.
    assert!(check(&engine, json!({"out": {"total": 1}, "n": 1})).is_empty());
    // A multi-key object's keys are always fields, so are not linted.
    assert!(check(&engine, json!({"vr": 1, "mapp": 2})).is_empty());
}

#[test]
fn an_escaped_key_is_deliberate() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .build();
    assert!(check(&engine, json!({"$type": 1, "n": {"$vr": 2}})).is_empty());
}

// ── modes ───────────────────────────────────────────────────────────────

#[test]
fn the_mode_overrides_the_engine_for_one_check() {
    let templating = Engine::builder().with_templating(true).build();
    let rule = json!({"a": {"var": "x"}, "b": 1});
    assert!(templating.check(&rule, CheckMode::Engine).is_empty());
    assert_eq!(
        summary(&templating.check(&rule, CheckMode::Strict)),
        [(NotAnOperator, Error, s(""))]
    );
    assert!(Engine::new().check(&rule, CheckMode::Template).is_empty());
    // An unknown single key is a field in a template, an error otherwise.
    let rule = json!({"bogus_field": 1});
    assert!(Engine::new().check(&rule, CheckMode::Template).is_empty());
    assert_eq!(
        Engine::new().check(&rule, CheckMode::Strict)[0].code,
        UnknownOperator
    );
}

// ── shape of the report ─────────────────────────────────────────────────

#[test]
fn every_problem_is_reported_in_rule_order() {
    let rule = json!({"if": [
        {"bogus": 1},
        {"map": [1]},
        {"a": 1, "b": 2},
        {"!": [1, 2]}
    ]});
    assert_eq!(
        summary(&check(&Engine::new(), rule)),
        [
            (UnknownOperator, Error, s("/if/0")),
            (ArgumentCount, Error, s("/if/1")),
            (NotAnOperator, Error, s("/if/2")),
            (ArgumentCount, Warning, s("/if/3")),
        ]
    );
}

#[test]
fn pointers_escape_and_follow_single_arguments() {
    let engine = Engine::new();
    // `/` is escaped as `~1`; a non-array argument adds no index.
    assert_eq!(
        summary(&check(&engine, json!({"/": [1, {"!": {"bogus": 1}}]}))),
        [(UnknownOperator, Error, s("/~1/1/!"))]
    );
    assert_eq!(
        summary(&check(&engine, json!([0, [{"x~y": 1}]]))),
        [(UnknownOperator, Error, s("/1/0"))]
    );
    let d = &check(&engine, json!({"a/b~c": 1}))[0];
    assert_eq!(d.operator.as_deref(), Some("a/b~c"));
}

#[test]
fn compile_checked_refuses_any_error_and_reports_all() {
    let engine = Engine::new();
    let err = engine
        .compile_checked(&json!({"if": [{"bogus": 1}, {"map": [1]}]}))
        .unwrap_err();
    assert_eq!(err.diagnostics.len(), 2);
    assert!(err.to_string().contains("bogus"), "{err}");
    let ok = engine
        .compile_checked(&json!({"+": [1, {"var": "x"}]}))
        .unwrap();
    assert_eq!(engine.session().eval_str(&ok, r#"{"x": 2}"#).unwrap(), "3");
}

#[test]
fn a_diagnostic_serialises_for_the_bindings() {
    let d = &check(&Engine::new(), json!({"if": [true, {"vr": 1}, 0]}))[0];
    let v = serde_json::to_value(d).unwrap();
    assert_eq!(v["code"], "UnknownOperator");
    assert_eq!(v["severity"], "error");
    assert_eq!(v["pointer"], "/if/1");
    assert_eq!(v["operator"], "vr");
    assert!(v["message"].as_str().unwrap().contains("vr"));
    assert!(d.to_string().contains("/if/1"), "{d}");
}

// ── helpers ─────────────────────────────────────────────────────────────

struct Plain;

impl CustomOperator for Plain {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.i64(1))
    }
}

struct Declared(CustomOperatorInfo);

impl CustomOperator for Declared {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.i64(1))
    }

    fn info(&self) -> CustomOperatorInfo {
        self.0
    }
}

// ── agreement with the compiler ─────────────────────────────────────────

mod against_compile {
    use super::*;
    use proptest::prelude::*;

    fn arb_rule() -> impl Strategy<Value = Value> {
        let key = prop_oneof![
            Just("var"),
            Just("if"),
            Just("and"),
            Just("map"),
            Just("+"),
            Just("!"),
            Just("missing_some"),
            Just("filter"),
            Just("vr"),
            Just("bogus"),
            Just("a"),
            Just("sort"),
            Just("format_date"),
            Just("tensor"),
        ];
        let leaf = prop_oneof![
            Just(json!(null)),
            any::<bool>().prop_map(Value::from),
            (-2i64..3).prop_map(Value::from),
            prop_oneof![Just("x"), Just("UTC"), Just("Nowhere/Zone")].prop_map(Value::from),
        ];
        leaf.prop_recursive(4, 40, 4, move |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
                (key.clone(), inner.clone()).prop_map(|(k, v)| json!({k: v})),
                (key.clone(), prop::collection::vec(inner.clone(), 0..4))
                    .prop_map(|(k, v)| json!({k: v})),
                (inner.clone(), inner).prop_map(|(a, b)| json!({"a": a, "b": b})),
            ]
        })
    }

    proptest! {
        #[test]
        fn check_and_compile_agree(rule in arb_rule(), templating in any::<bool>()) {
            let engine = Engine::builder().with_templating(templating).build();
            let diags = engine.check(&rule, CheckMode::Engine);
            let compile_level = diags.iter().any(|d| matches!(
                d.code,
                DiagnosticCode::NotAnOperator | DiagnosticCode::Unparsable | DiagnosticCode::Compile
            ));
            // Compiling fails exactly when the check finds a compile-level
            // problem.
            prop_assert_eq!(engine.compile(&rule).is_err(), compile_level, "{}", rule);
            // compile_checked accepts exactly the rules with no error.
            let has_error = diags.iter().any(|d| d.severity == Error);
            prop_assert_eq!(engine.compile_checked(&rule).is_err(), has_error, "{}", rule);
        }
    }
}

/// Spike S4: check the rules stored in host repositories, as dataflow-rs
/// compiles them (one templating engine, with the hosts' custom
/// operators). Prints every diagnostic; reviewing them is the spike.
///
/// ```text
/// CHECK_CORPUS=../../../Orion-Projects:../../../dataflow-rs \
///   cargo test -p datalogic-rs --all-features --test check_test corpus -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn corpus() {
    const RULE_KEYS: &[&str] = &["condition", "logic", "over", "message"];
    const HOST_OPERATORS: &[&str] = &[
        "secret",
        "encode",
        "decode",
        "random",
        "url_encode",
        "url_decode",
        "join",
    ];

    fn files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if !matches!(name, "node_modules" | "target" | ".git" | "dist") {
                    files(&path, out);
                }
            } else if name.ends_with(".json") {
                out.push(path);
            }
        }
    }

    fn rules<'v>(v: &'v Value, at: String, out: &mut Vec<(String, &'v Value)>) {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    let here = format!("{at}/{k}");
                    if RULE_KEYS.contains(&k.as_str()) {
                        out.push((here.clone(), child));
                    } else if k == "fields" && child.is_object() {
                        for (f, rule) in child.as_object().unwrap() {
                            out.push((format!("{here}/{f}"), rule));
                        }
                    }
                    rules(child, here, out);
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    rules(child, format!("{at}/{i}"), out);
                }
            }
            _ => {}
        }
    }

    let mut builder = Engine::builder().with_templating(true);
    for name in HOST_OPERATORS {
        builder = builder.add_operator(*name, Plain);
    }
    let engine = builder.build();

    let corpus = std::env::var("CHECK_CORPUS").expect("CHECK_CORPUS=dir:dir");
    let mut paths = Vec::new();
    for dir in corpus.split(':') {
        files(std::path::Path::new(dir), &mut paths);
    }
    paths.sort();
    let (mut checked, mut by_code) = (0usize, std::collections::BTreeMap::new());
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(doc) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let mut found = Vec::new();
        rules(&doc, String::new(), &mut found);
        for (at, rule) in found {
            checked += 1;
            // A condition is never an output template, so it is also checked
            // strictly: what `compile_strict` would say.
            let mode = if at.ends_with("/condition") {
                CheckMode::Strict
            } else {
                CheckMode::Engine
            };
            for d in engine.check(rule, mode) {
                *by_code
                    .entry(format!("{:?} {:?}", d.severity, d.code))
                    .or_insert(0) += 1;
                println!("{}{at}: {d}", path.display());
            }
        }
    }
    println!("\n{checked} rules checked");
    for (code, n) in by_code {
        println!("  {n:5}  {code}");
    }
}

// ── depth and arguments that never fail the rule ───────────────────────

#[test]
fn check_rejects_exactly_the_rules_too_deep_to_compile() {
    let engine = Engine::new();
    for depth in 250..=262 {
        let mut rule = json!(true);
        for _ in 0..depth {
            rule = json!({ "!": [rule] });
        }
        let compiles = engine.compile(&rule).is_ok();
        let diags = check(&engine, rule.clone());
        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Error).collect();
        assert_eq!(errors.is_empty(), compiles, "depth {depth}: {diags:?}");
        if !compiles {
            assert_eq!(errors.len(), 1, "one depth error, not one per level");
            assert_eq!(errors[0].code, Compile);
            assert!(engine.compile_checked(&rule).is_err());
        }
    }
}

#[test]
fn arguments_the_call_never_evaluates_are_not_errors() {
    let engine = Engine::new();
    for rule in [
        // Too few arguments: `in` returns false, `switch` null, unevaluated.
        json!({"in": [{"typo": 1}]}),
        json!({"switch": [{"typo": 1}]}),
        // Extra arguments `!` never reads.
        json!({"!": [true, {"typo": 1}]}),
    ] {
        let diags = check(&engine, rule.clone());
        assert!(
            diags.iter().all(|d| d.severity == Warning),
            "{rule}: {diags:?}"
        );
        assert!(engine.compile_checked(&rule).is_ok(), "{rule}");
    }
    // An argument `!` does read is still checked.
    assert_eq!(
        summary(&check(&engine, json!({"!": [{"typo": 1}, true]}))),
        vec![
            (ArgumentCount, Warning, s("")),
            (UnknownOperator, Error, s("/!/0")),
        ]
    );
}

#[test]
fn an_error_a_try_arm_catches_is_a_warning() {
    let engine = Engine::new();
    let rule = json!({"try": [{"risky_operation": []}, {"var": "type"}]});
    assert_eq!(
        summary(&check(&engine, rule.clone())),
        vec![(UnknownOperator, Warning, s("/try/0"))]
    );
    assert!(engine.compile_checked(&rule).is_ok());
    // The last arm's error is not caught.
    assert_eq!(
        summary(&check(&engine, json!({"try": [1, {"typo": []}]}))),
        vec![(UnknownOperator, Error, s("/try/1"))]
    );
}
