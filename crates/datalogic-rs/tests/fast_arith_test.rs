//! Arithmetic fast paths in `map` and `reduce` agree with the general path.
//!
//! Three shapes skip per-item dispatch: a `map` whose body is arithmetic on
//! fields and literals, a `reduce` arithmetic fold (`{op: [current,
//! accumulator]}` in either order, optionally on a path under `current`),
//! and the fused `reduce(map(..))` that folds the mapped values without
//! building the intermediate array. A traced evaluation skips all three,
//! so it is the oracle. The datasets cover the representation edges these
//! paths have got wrong before: integers past 2^53, `i64` overflow in both
//! directions next to large exact neighbours, whole floats, the issue-#61
//! reproduction, and non-numbers that must fall back to the general path.

#![cfg(all(feature = "trace", feature = "serde_json"))]

use datalogic_rs::Engine;
use serde_json::{Value, json};

/// Element lists, each exercising one edge.
fn datasets() -> Vec<Vec<Value>> {
    vec![
        vec![json!(1), json!(2), json!(3), json!(-4), json!(5)],
        vec![json!(0.5), json!(1.25), json!(-2.5)],
        vec![json!(1), json!(2.0), json!(3)],
        vec![
            json!(9007199254740993_i64),
            json!(2),
            json!(-9007199254740991_i64),
        ],
        vec![json!(i64::MAX), json!(1), json!(i64::MIN), json!(-1)],
        // One overflowing element among exact values past 2^53.
        vec![
            json!(i64::MAX),
            json!(9007199254740993_i64),
            json!(-9007199254740995_i64),
        ],
        vec![json!(-9591485970090907_i64); 6],
        vec![json!(1), json!("2"), json!(3)],
        vec![json!(1), json!(null)],
        vec![json!(true), json!(2)],
        vec![],
    ]
}

/// Initial accumulators, including a non-number (the fast paths decline).
fn initials() -> Vec<Value> {
    vec![
        json!(0),
        json!(1),
        json!(0.25),
        json!(-1),
        json!(i64::MAX),
        json!("5"),
    ]
}

fn fold_bodies(current: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for op in ["+", "-", "*"] {
        out.push(json!({ op: [{"var": current}, {"var": "accumulator"}] }));
        out.push(json!({ op: [{"var": "accumulator"}, {"var": current}] }));
    }
    out
}

/// `map` bodies the fusion recognises: extraction, arithmetic with a
/// literal on either side, and arithmetic over two fields.
fn map_bodies() -> Vec<Value> {
    vec![
        json!({"var": "x"}),
        json!({"*": [{"var": "x"}, 2]}),
        json!({"-": [10, {"var": "x"}]}),
        json!({"+": [{"var": "x"}, 0.5]}),
        json!({"+": [{"var": "x"}, {"var": "y"}]}),
        json!({"-": [{"var": "y"}, {"var": "x"}]}),
        json!({"*": [{"var": "x"}, {"var": "y"}]}),
    ]
}

/// Collects every disagreement between the fast and the general path.
struct Oracle {
    engine: Engine,
    checked: usize,
    mismatches: Vec<String>,
}

impl Oracle {
    fn check(&mut self, rule: &Value, data: &Value) {
        let (rule, data) = (rule.to_string(), data.to_string());
        let fast = self.engine.eval_str(&rule, &data).map_err(|e| e.kind);
        let general = self
            .engine
            .trace()
            .eval_str(&rule, &data)
            .result
            .map_err(|e| e.kind);
        let (fast, general) = (format!("{fast:?}"), format!("{general:?}"));
        if fast != general {
            self.mismatches.push(format!(
                "{rule}\n    over {data}\n    fast    {fast}\n    general {general}"
            ));
        }
        self.checked += 1;
    }

    fn finish(self, at_least: usize) {
        assert!(self.checked >= at_least, "only {} cases", self.checked);
        assert!(
            self.mismatches.is_empty(),
            "{} of {} cases disagree; first ones:\n{}",
            self.mismatches.len(),
            self.checked,
            self.mismatches
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn map_arithmetic_matches_the_general_path() {
    let mut oracle = Oracle {
        engine: Engine::new(),
        checked: 0,
        mismatches: Vec::new(),
    };
    for values in datasets() {
        for y in [json!(3), json!(-2.5), json!("3"), json!(i64::MIN)] {
            let rows: Vec<Value> = values.iter().map(|v| json!({ "x": v, "y": y })).collect();
            let data = json!({ "xs": rows });
            for map_body in map_bodies() {
                oracle.check(&json!({"map": [{"var": "xs"}, map_body]}), &data);
            }
            // Bare-item bodies over the values themselves.
            let data = json!({ "xs": values });
            for op in ["+", "-", "*"] {
                for lit in [json!(2), json!(-1), json!(0.5), json!(i64::MAX)] {
                    for body in [
                        json!({ op: [{"var": ""}, lit] }),
                        json!({ op: [lit, {"var": ""}] }),
                    ] {
                        oracle.check(&json!({"map": [{"var": "xs"}, body]}), &data);
                    }
                }
            }
        }
    }
    oracle.finish(1300);
}

#[test]
fn arithmetic_fold_matches_the_general_path() {
    let mut oracle = Oracle {
        engine: Engine::new(),
        checked: 0,
        mismatches: Vec::new(),
    };
    for values in datasets() {
        // Bare `current` over the values themselves, and `current.x` over
        // rows holding them (plus a row missing the field).
        let rows: Vec<Value> = values
            .iter()
            .map(|v| json!({ "x": v }))
            .chain(std::iter::once(json!({"w": 1})))
            .collect();
        for initial in initials() {
            for (current, items) in [("current", &values), ("current.x", &rows)] {
                let data = json!({ "xs": items });
                for body in fold_bodies(current) {
                    let rule = json!({"reduce": [{"var": "xs"}, body, initial]});
                    oracle.check(&rule, &data);
                }
                // Without an initial value the accumulator starts as null,
                // which the fast path declines.
                let rule = json!({"reduce": [{"var": "xs"}, fold_bodies(current)[0]]});
                oracle.check(&rule, &data);
            }
        }
    }
    oracle.finish(700);
}

#[test]
fn fused_reduce_map_matches_the_general_path() {
    let mut oracle = Oracle {
        engine: Engine::new(),
        checked: 0,
        mismatches: Vec::new(),
    };
    for values in datasets() {
        // `x` takes each value, `y` a fixed 3 (or a non-number in one row
        // set, so two-field bodies also hit the fallback).
        for y in [json!(3), json!(-2.5), json!("3")] {
            let rows: Vec<Value> = values.iter().map(|v| json!({ "x": v, "y": y })).collect();
            let data = json!({ "xs": rows });
            for map_body in map_bodies() {
                let mapped = json!({"map": [{"var": "xs"}, map_body]});
                for initial in initials() {
                    for body in fold_bodies("current") {
                        let rule = json!({"reduce": [mapped, body, initial]});
                        oracle.check(&rule, &data);
                    }
                }
            }
        }
    }
    oracle.finish(7000);
}
