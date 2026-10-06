//! `Engine::truthy_of`: the engine's truthiness for a value the host already
//! holds, whatever its representation, under every `TruthyEvaluator`.
//!
//! The reference is the rule `{"!!": {"var": ""}}` evaluated by the same
//! engine: whatever the evaluator decides for a value inside a rule,
//! `truthy_of` must decide for the value outside one.
#![cfg(feature = "serde_json")]

use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{Engine, EvaluationConfig, ParsedData, TruthyEvaluator};
use serde_json::{Value, json};

fn values() -> Vec<Value> {
    vec![
        json!(null),
        json!(true),
        json!(false),
        json!(0),
        json!(1),
        json!(-1),
        json!(0.0),
        json!(-0.0),
        json!(0.5),
        json!(1e300),
        json!(i64::MAX),
        json!(i64::MIN),
        json!(u64::MAX),
        json!(""),
        json!("0"),
        json!("false"),
        json!(" "),
        json!([]),
        json!([0]),
        json!([[]]),
        json!({}),
        json!({"a": 0}),
        json!({"": null}),
    ]
}

fn engines() -> Vec<(&'static str, Engine)> {
    let with = |t: TruthyEvaluator| {
        Engine::builder()
            .with_config(EvaluationConfig::default().with_truthy_evaluator(t))
            .build()
    };
    vec![
        ("default", Engine::new()),
        ("javascript", with(TruthyEvaluator::JavaScript)),
        ("python", with(TruthyEvaluator::Python)),
        ("strict", with(TruthyEvaluator::StrictBoolean)),
        (
            "custom",
            // Truthy exactly when the value is a non-empty string, so the
            // custom evaluator disagrees with every built-in one somewhere.
            with(TruthyEvaluator::custom(
                |v| matches!(v, OwnedDataValue::String(s) if !s.is_empty()),
            )),
        ),
    ]
}

/// What the engine decides inside a rule.
fn in_rule(engine: &Engine, value: &Value) -> bool {
    let out = engine.eval(r#"{"!!": {"var": ""}}"#, value).unwrap();
    out.as_bool().unwrap()
}

#[test]
fn every_representation_agrees_with_the_rule() {
    for (name, engine) in engines() {
        for value in values() {
            let expected = in_rule(&engine, &value);
            let owned = OwnedDataValue::from_json(&value.to_string()).unwrap();
            let parsed = ParsedData::from_value(&value);

            assert_eq!(engine.truthy_of(&value), expected, "{name} serde {value}");
            assert_eq!(engine.truthy_of(&owned), expected, "{name} owned {value}");
            assert_eq!(engine.truthy_of(&parsed), expected, "{name} parsed {value}");
            assert_eq!(
                engine.truthy_of(parsed.value()),
                expected,
                "{name} arena {value}"
            );
            // The arena form `Engine::truthy` already took agrees too.
            assert_eq!(engine.truthy(parsed.value()), expected, "{name} {value}");
        }
    }
}

/// JSONLogic truthiness: an empty object is falsy, like an empty array.
#[test]
fn empty_object_is_falsy() {
    let engine = Engine::new();
    assert!(!engine.truthy_of(&json!({})));
    assert!(engine.truthy_of(&json!({"a": null})));
    assert!(!engine.truthy_of(&json!([])));
    assert!(!engine.truthy_of(&json!("")));
    assert!(!engine.truthy_of(&json!(0)));
    assert!(engine.truthy_of(&json!("0")));
}

/// The custom evaluator sees the whole value, not just its shape.
#[test]
fn custom_evaluator_sees_the_value() {
    let engine = Engine::builder()
        .with_config(
            EvaluationConfig::default().with_truthy_evaluator(TruthyEvaluator::custom(|v| {
                v.as_object()
                    .and_then(|o| o.iter().find(|(k, _)| k == "ok"))
                    .is_some_and(|(_, v)| v.as_bool() == Some(true))
            })),
        )
        .build();
    assert!(engine.truthy_of(&json!({"ok": true, "n": [1, 2]})));
    assert!(!engine.truthy_of(&json!({"ok": false})));
    assert!(!engine.truthy_of(&json!(true)));
}

/// A result evaluated into a `serde_json::Value` (the shape Orion's guards
/// hold) is judged the way the rule itself would have judged it.
#[test]
fn result_of_an_evaluation() {
    let engine = Engine::new();
    let rule = engine.compile(r#"{"var": "payload"}"#).unwrap();
    let mut session = engine.session();
    let out: Value = session.eval_into(&rule, &json!({"payload": {}})).unwrap();
    assert_eq!(out, json!({}));
    assert!(!engine.truthy_of(&out));
}

/// Folding must not change a result under a custom evaluator: collapsing
/// `!(!x)` into `!!x` (and the other nested truth forms) assumes a boolean
/// is its own truthiness, which a custom evaluator need not keep.
#[test]
fn nested_truth_operators_under_a_custom_evaluator_are_not_collapsed() {
    let even = || {
        TruthyEvaluator::custom(|v: &OwnedDataValue| {
            v.as_i64().map(|n| n % 2 == 0).unwrap_or(false)
        })
    };
    let build = |fold: bool| {
        Engine::builder()
            .with_config(EvaluationConfig::default().with_truthy_evaluator(even()))
            .with_constant_folding(fold)
            .build()
    };
    let (folded, unfolded) = (build(true), build(false));
    for rule in [
        json!({"!!": [{"!": [{"var": "n"}]}]}),
        json!({"!": [{"!!": [{"var": "n"}]}]}),
        json!({"!": [{"!": [{"var": "n"}]}]}),
        json!({"!!": [{"!!": [{"var": "n"}]}]}),
    ] {
        for n in [2, 3] {
            let data = json!({ "n": n });
            let run = |e: &Engine| e.eval_str(&rule.to_string(), &data.to_string()).unwrap();
            assert_eq!(run(&folded), run(&unfolded), "{rule} with n={n}");
        }
    }
}
