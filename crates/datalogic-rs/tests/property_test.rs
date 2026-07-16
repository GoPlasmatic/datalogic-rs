// Property-based tests: feed bounded arbitrary (rule, data) JSON pairs
// through the engine and check invariants that must hold for *any* input:
//
//   A. `never_panics` — compile/evaluate may return `Ok` or `Err` for
//      arbitrary JSON, but must never panic (proptest turns a panic into
//      a failing, minimized counterexample).
//   B. `optimized_and_traced_agree` — differential oracle. The optimized
//      pipeline (`Engine::eval_str`: optimizer + constant folding + fast
//      paths) and the traced pipeline (`Engine::trace().eval_str`:
//      compiled via `compile_for_trace` with folding disabled, running
//      the general dispatch path) must agree on every input: both `Ok`
//      with semantically equal JSON, or both `Err`. This is the property
//      that catches fast-path-vs-general-dispatch divergence bugs.
//
// Each property runs 256 cases by default (CI-friendly); override with
// proptest's standard env var, e.g.:
//
//   PROPTEST_CASES=4096 cargo test -p datalogic-rs --all-features \
//     --test property_test
//
// Failing inputs are minimized and persisted under
// `crates/datalogic-rs/proptest-regressions/` — commit those files so the
// regression seeds replay first on every subsequent run.
#![cfg(all(feature = "serde_json", feature = "trace"))]

use datalogic_rs::Engine;
use proptest::prelude::*;
use serde_json::{Value, json};

/// Strings drawn from a small pool: operator names (so generated objects
/// often form meaningful rules) plus short plain identifiers (so `var`
/// paths sometimes hit generated data keys). Used for both object keys
/// and string leaves.
fn arb_key() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("var".to_owned()),
        Just("+".to_owned()),
        Just("if".to_owned()),
        Just("map".to_owned()),
        Just("missing".to_owned()),
        Just("cat".to_owned()),
        Just("==".to_owned()),
        Just("!".to_owned()),
        Just("a.b".to_owned()),
        "[a-c]{1,2}",
    ]
}

/// JSON numbers spanning the shapes the engine special-cases: small
/// integers, arbitrary/extreme i64s (overflow paths), negatives, and
/// finite floats (including negative and fractional).
fn arb_number() -> impl Strategy<Value = Value> {
    prop_oneof![
        (-1000i64..1000).prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        Just(Value::from(i64::MAX)),
        Just(Value::from(i64::MIN)),
        (-1.0e15f64..1.0e15).prop_map(|f| json!(f)),
        Just(json!(0.5)),
        Just(json!(-2.5)),
    ]
}

/// Bounded arbitrary JSON: nesting depth <= 4, collections <= 6 elements,
/// strings/keys from [`arb_key`], numbers from [`arb_number`].
fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        arb_number(),
        arb_key().prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..=6).prop_map(Value::Array),
            prop::collection::btree_map(arb_key(), inner, 0..=6)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

/// A small pure aggregate over a generated var path — the shape the CSE
/// pass targets. Operator and initial accumulator vary so near-twin
/// classes (e.g. `0` vs `0.0` initials) get generated too.
fn arb_aggregate() -> impl Strategy<Value = Value> {
    let body_op = prop_oneof![Just("+"), Just("*"), Just("-")];
    let initial = prop_oneof![
        Just(json!(0)),
        Just(json!(0.0)),
        Just(json!(1)),
        Just(json!(100)),
    ];
    ("[a-c]{1,2}", body_op, initial).prop_map(|(path, op, initial)| {
        json!({
            "reduce": [
                {"var": path},
                {op: [{"var": "accumulator"}, {"var": "current"}]},
                initial
            ]
        })
    })
}

/// Splice 2–4 occurrences of one aggregate (plus arbitrary siblings) into
/// a combining operator, sometimes nesting an occurrence inside a `map`
/// body or an `if` branch — the placements the CSE gates must handle.
fn arb_spliced_rule() -> impl Strategy<Value = Value> {
    (arb_aggregate(), arb_json(), 2usize..=4, 0usize..=2).prop_map(
        |(agg, extra, copies, placement)| {
            let mut args: Vec<Value> = std::iter::repeat_n(agg.clone(), copies).collect();
            args.push(extra);
            match placement {
                // All occurrences at combinable positions.
                0 => json!({"+": args}),
                // One extra occurrence inside a map body (per-item
                // context — must not share the root memo).
                1 => json!({"+": [
                    {"map": [{"var": "a"}, agg]},
                    args
                ]}),
                // Occurrences split across if branches.
                _ => json!({"if": [{"var": "a"}, {"+": args}, agg]}),
            }
        },
    )
}

/// A fused-shape pipeline: `reduce` over a `map` with an extract or
/// arithmetic body — exactly what the Stage 2 fusion intercepts. Operand
/// orders, fold orders, extreme literals, and fractional initials all vary
/// to probe the integer/f64 mode boundaries.
fn arb_fused_pipeline() -> impl Strategy<Value = Value> {
    let arith_op = || prop_oneof![Just("+"), Just("*"), Just("-")];
    let lit = prop_oneof![
        Just(json!(2)),
        Just(json!(0.5)),
        Just(json!(-3)),
        any::<i64>().prop_map(Value::from),
    ];
    let map_body = (
        "[a-c]{1,2}",
        arith_op(),
        lit,
        any::<bool>(),
        prop::option::of(prop::option::of("[a-c]{1,2}")),
    )
        .prop_map(|(p1, op, lit, var_is_lhs, second)| match second {
            // Extract body.
            None => json!({ "var": p1 }),
            // var ⊗ var body.
            Some(Some(p2)) => json!({ op: [{ "var": p1 }, { "var": p2 }] }),
            // var ⊗ literal body, both operand orders.
            Some(None) => {
                if var_is_lhs {
                    json!({ op: [{ "var": p1 }, lit] })
                } else {
                    json!({ op: [lit, { "var": p1 }] })
                }
            }
        });
    let initial = prop_oneof![
        Just(json!(0)),
        Just(json!(1)),
        Just(json!(0.25)),
        Just(Value::from(i64::MAX)),
    ];
    ("[a-c]{1,2}", map_body, arith_op(), any::<bool>(), initial).prop_map(
        |(src, body, fold_op, acc_is_lhs, initial)| {
            let fold = if acc_is_lhs {
                json!({ fold_op: [{ "var": "accumulator" }, { "var": "current" }] })
            } else {
                json!({ fold_op: [{ "var": "current" }, { "var": "accumulator" }] })
            };
            json!({ "reduce": [{ "map": [{ "var": src }, body] }, fold, initial] })
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property A: arbitrary (rule, data) never panics — every outcome
    /// surfaces as `Ok` or `Err`. Drives both the string boundary (JSON
    /// parse + full pipeline) and the typed `&serde_json::Value` boundary.
    #[test]
    fn never_panics(rule in arb_json(), data in arb_json()) {
        let engine = Engine::new();
        let rule_str = serde_json::to_string(&rule).expect("generated rule serialises");
        let data_str = serde_json::to_string(&data).expect("generated data serialises");

        let _ = engine.eval_str(rule_str.as_str(), data_str.as_str());
        let _ = engine.eval_into::<Value, _, _>(&rule, &data);
    }

    /// Property B: the optimized and traced pipelines agree — both `Ok`
    /// with semantically equal JSON (`serde_json::Value` equality, so key
    /// order / float text differences don't false-alarm) or both `Err`.
    #[test]
    fn optimized_and_traced_agree(rule in arb_json(), data in arb_json()) {
        let engine = Engine::new();
        let rule_str = serde_json::to_string(&rule).expect("generated rule serialises");
        let data_str = serde_json::to_string(&data).expect("generated data serialises");

        let optimized = engine.eval_str(rule_str.as_str(), data_str.as_str());
        let traced = engine.trace().eval_str(rule_str.as_str(), data_str.as_str()).result;

        match (optimized, traced) {
            (Ok(optimized), Ok(traced)) => {
                let optimized: Value =
                    serde_json::from_str(&optimized).expect("engine emits valid JSON");
                let traced: Value =
                    serde_json::from_str(&traced).expect("engine emits valid JSON");
                prop_assert_eq!(
                    optimized,
                    traced,
                    "optimized vs traced result mismatch for rule={} data={}",
                    rule_str,
                    data_str
                );
            }
            (Err(_), Err(_)) => {}
            (optimized, traced) => prop_assert!(
                false,
                "optimized vs traced Ok/Err divergence for rule={} data={}: optimized={:?} traced={:?}",
                rule_str,
                data_str,
                optimized,
                traced
            ),
        }
    }

    /// Property C: CSE differential oracle. Rules with spliced repeated
    /// aggregates — the exact shape the CSE pass wraps — must agree
    /// between the optimized pipeline (folding + CSE + fast paths) and
    /// the traced pipeline (no-fold compile, zero Cse nodes, general
    /// dispatch): both `Ok` with equal JSON or both `Err`.
    #[test]
    fn cse_spliced_aggregates_agree(rule in arb_spliced_rule(), data in arb_json()) {
        let engine = Engine::new();
        let rule_str = serde_json::to_string(&rule).expect("generated rule serialises");
        let data_str = serde_json::to_string(&data).expect("generated data serialises");

        let optimized = engine.eval_str(rule_str.as_str(), data_str.as_str());
        let traced = engine.trace().eval_str(rule_str.as_str(), data_str.as_str()).result;

        match (optimized, traced) {
            (Ok(optimized), Ok(traced)) => {
                let optimized: Value =
                    serde_json::from_str(&optimized).expect("engine emits valid JSON");
                let traced: Value =
                    serde_json::from_str(&traced).expect("engine emits valid JSON");
                prop_assert_eq!(
                    optimized,
                    traced,
                    "CSE vs traced result mismatch for rule={} data={}",
                    rule_str,
                    data_str
                );
            }
            (Err(_), Err(_)) => {}
            (optimized, traced) => prop_assert!(
                false,
                "CSE vs traced Ok/Err divergence for rule={} data={}: optimized={:?} traced={:?}",
                rule_str,
                data_str,
                optimized,
                traced
            ),
        }
    }

    /// Property D: reduce(map(...)) fusion differential oracle. Fused
    /// pipelines must agree with the traced pipeline (no-fold compile,
    /// general dispatch, no fusion) on every input — including the
    /// integer-overflow and int/f64 representation boundaries the
    /// extreme literals and initials probe.
    #[test]
    fn fused_reduce_map_agrees(rule in arb_fused_pipeline(), data in arb_json()) {
        let engine = Engine::new();
        let rule_str = serde_json::to_string(&rule).expect("generated rule serialises");
        let data_str = serde_json::to_string(&data).expect("generated data serialises");

        let optimized = engine.eval_str(rule_str.as_str(), data_str.as_str());
        let traced = engine.trace().eval_str(rule_str.as_str(), data_str.as_str()).result;

        match (optimized, traced) {
            (Ok(optimized), Ok(traced)) => {
                let optimized: Value =
                    serde_json::from_str(&optimized).expect("engine emits valid JSON");
                let traced: Value =
                    serde_json::from_str(&traced).expect("engine emits valid JSON");
                prop_assert_eq!(
                    optimized,
                    traced,
                    "fused vs traced result mismatch for rule={} data={}",
                    rule_str,
                    data_str
                );
            }
            (Err(_), Err(_)) => {}
            (optimized, traced) => prop_assert!(
                false,
                "fused vs traced Ok/Err divergence for rule={} data={}: optimized={:?} traced={:?}",
                rule_str,
                data_str,
                optimized,
                traced
            ),
        }
    }
}
