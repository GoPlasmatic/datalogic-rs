//! Fast-predicate paths agree with the general path.
//!
//! `filter`, `all`, `some` and `none` evaluate a recognised predicate shape
//! (a comparison against a literal, `in` over string literals, `and` / `or`
//! / `!` / `!!` trees of those) without dispatching it per item. A traced
//! evaluation skips every fast path, so it is the oracle: for each predicate
//! shape, path shape and dataset, the plain result must equal the traced
//! one. The datasets mix value types so the fast paths also hit their
//! "indeterminate, re-run on the general path" exits.

#![cfg(all(feature = "trace", feature = "serde_json"))]

use datalogic_rs::Engine;
use serde_json::{Value, json};

/// Every value shape a fast leaf has to classify: integers, floats,
/// negatives, numeric and plain strings, booleans, null and containers.
fn mixed() -> Vec<Value> {
    vec![
        json!(0),
        json!(1),
        json!(1.0),
        json!(2),
        json!(2.0),
        json!(2.5),
        json!(3),
        json!(-1),
        json!("2"),
        json!("a"),
        json!("b"),
        json!(true),
        json!(false),
        json!(null),
        json!([1]),
        json!({"a": 1}),
    ]
}

/// Datasets per value: all numbers (fast path completes), all strings
/// (string leaves complete, numeric ones fall back), and everything.
fn datasets() -> Vec<Vec<Value>> {
    vec![
        vec![
            json!(0),
            json!(1),
            json!(1.0),
            json!(2),
            json!(2.0),
            json!(2.5),
            json!(3),
            json!(-1),
        ],
        vec![json!("a"), json!("b"), json!("c"), json!("")],
        mixed(),
    ]
}

/// Leaf predicates over `var`, in both operand orders where the shape has
/// two operands.
fn leaves(var: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for op in ["===", "!=="] {
        for lit in [
            json!(1),
            json!(1.0),
            json!(2.5),
            json!("a"),
            json!(null),
            json!(true),
        ] {
            out.push(json!({ op: [var, lit] }));
            out.push(json!({ op: [lit, var] }));
        }
    }
    for op in [">", ">=", "<", "<="] {
        for lit in [json!(2), json!(2.5), json!(-1)] {
            out.push(json!({ op: [var, lit] }));
            out.push(json!({ op: [lit, var] }));
        }
    }
    for op in ["==", "!="] {
        for lit in [json!(2), json!(0.0), json!("b")] {
            out.push(json!({ op: [var, lit] }));
            out.push(json!({ op: [lit, var] }));
        }
    }
    out.push(json!({"in": [var, ["a", "b"]]}));
    out
}

/// Combinator trees over a few leaves, plus a bare truthy `var` arm.
fn combinators(var: &Value) -> Vec<Value> {
    let gt = json!({">": [var, 1]});
    let eq = json!({"===": [var, "a"]});
    vec![
        json!({"and": [var, gt]}),
        json!({"or": [eq, gt]}),
        json!({"!": gt}),
        json!({"!!": eq}),
        json!({"and": [{"!": eq}, {"or": [gt, {"in": [var, ["b"]]}]}]}),
    ]
}

/// How a dataset wraps each value so the predicate's `var` path finds it.
type Wrap = fn(Value) -> Value;

#[test]
fn fast_predicates_match_the_general_path() {
    let engine = Engine::new();
    let mut checked = 0;
    let mut mismatches = Vec::new();
    // (the predicate's view of an item, how the dataset wraps each value)
    let paths: [(Value, Wrap); 3] = [
        (json!({"var": ""}), |v| v),
        (json!({"var": "v"}), |v| json!({ "v": v })),
        (json!({"var": "o.v"}), |v| json!({ "o": { "v": v } })),
    ];
    for (var, wrap) in &paths {
        let mut predicates = leaves(var);
        predicates.extend(combinators(var));
        for values in datasets() {
            let mut items: Vec<Value> = values.into_iter().map(wrap).collect();
            if var != &json!({"var": ""}) {
                // A missing field: `var`'s implicit null.
                items.push(json!({"w": 1}));
            }
            let data = json!({ "xs": items }).to_string();
            for pred in &predicates {
                for consumer in ["filter", "all", "some", "none"] {
                    let rule = json!({ consumer: [{"var": "xs"}, pred] }).to_string();
                    let fast = engine.eval_str(&rule, &data).map_err(|e| e.kind);
                    let general = engine
                        .trace()
                        .eval_str(&rule, &data)
                        .result
                        .map_err(|e| e.kind);
                    let (fast, general) = (format!("{fast:?}"), format!("{general:?}"));
                    if fast != general {
                        mismatches.push(format!(
                            "{rule}\n    over {data}\n    fast    {fast}\n    general {general}"
                        ));
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 2000, "only {checked} cases");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} cases disagree; first ones:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Equality fast paths compare as the operators do: numbers as `f64`
/// under `===`, and, with `datetime`, two strings that spell the same
/// instant as equal. Each rule's result matches the traced run and the
/// comparison written outside an iterator.
#[test]
fn equality_fast_paths_compare_as_the_operators_do() {
    let engine = Engine::new();
    let mut cases = vec![
        (
            json!({"===": [{"var": "k"}, 9007199254740992u64]}),
            json!({"k": 9007199254740993u64}),
        ),
        (json!({"===": [{"var": "k"}, 1.0]}), json!({"k": 1})),
        (json!({"!==": [{"var": "k"}, 1]}), json!({"k": 1.0})),
    ];
    if cfg!(feature = "datetime") {
        for op in ["==", "===", "!=", "!=="] {
            cases.push((
                json!({op: [{"var": "k"}, "2024-01-01T00:00:00Z"]}),
                json!({"k": "2024-01-01T00:00:00+00:00"}),
            ));
        }
        cases.push((
            json!({"in": [{"var": "k"}, ["2024-01-01T00:00:00Z", "x"]]}),
            json!({"k": "2024-01-01T00:00:00+00:00"}),
        ));
    }
    for (pred, item) in cases {
        let alone = engine
            .eval_str(&pred.to_string(), &item.to_string())
            .unwrap();
        let data = json!({ "xs": [item] }).to_string();
        for consumer in ["filter", "some", "all", "none"] {
            let rule = json!({ consumer: [{"var": "xs"}, pred] }).to_string();
            let fast = engine.eval_str(&rule, &data).unwrap();
            let general = engine.trace().eval_str(&rule, &data).result.unwrap();
            assert_eq!(fast, general, "{rule}");
            if consumer == "some" {
                assert_eq!(fast, alone, "{rule} vs {pred} alone");
            }
        }
    }
}
