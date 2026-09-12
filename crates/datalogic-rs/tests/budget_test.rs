//! Operation-budget tests: what the counter counts, where the abort
//! lands, and the paths that would otherwise be able to slip past it.
//!
//! The point of a budget is that it is *not* a wall-clock timeout: the
//! same rule over the same data must cost the same number of operations
//! on every machine, and the evaluation must be refused before the work
//! rather than reported after it. The cases below pin the parts of that
//! promise which are easy to break by accident — compile-time fast paths
//! that skip dispatch, and `try`, which catches everything else.

#![cfg(all(feature = "budget", feature = "serde_json"))]

use bumpalo::Bump;
use datalogic_rs::{Engine, EvaluationConfig, Metered};

/// Evaluate under `budget`, returning the operations spent or the refusal.
fn under_budget(rule: &str, data: &str, budget: u64) -> Result<u64, datalogic_rs::Error> {
    let engine = Engine::new();
    let compiled = engine.compile(rule).expect("compile");
    let arena = Bump::new();
    engine
        .evaluate_metered(&compiled, data, &arena, budget)
        .map(|m| m.ops)
}

/// Operations charged by `rule` over `data`, with no ceiling.
fn ops(rule: &str, data: &str) -> u64 {
    under_budget(rule, data, u64::MAX).expect("eval")
}

/// The workhorse rule: one dispatched node per item over `xs`.
const DOUBLE_XS: &str = r#"{"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]}"#;

fn array_of(n: usize) -> String {
    let items: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    format!(r#"{{"xs": [{}]}}"#, items.join(","))
}

// ---------------------------------------------------------------------------
// What one operation is
// ---------------------------------------------------------------------------

#[test]
fn a_literal_costs_nothing() {
    assert_eq!(ops("42", "null"), 0);
    assert_eq!(ops(r#"[1, 2, 3]"#, "null"), 0);
}

#[test]
fn a_folded_subtree_costs_nothing() {
    // The compiler resolved this before evaluation; charging for it would
    // make the count depend on whether folding was enabled.
    assert_eq!(ops(r#"{"+": [1, 2]}"#, "null"), 0);
}

#[test]
fn each_dispatched_node_costs_one() {
    // `+` and `var`. The literal operand is free.
    assert_eq!(ops(r#"{"+": [{"var": "x"}, 2]}"#, r#"{"x": 1}"#), 2);
    // `+`, `var`, `*`, `var`.
    assert_eq!(
        ops(
            r#"{"+": [{"var": "x"}, {"*": [{"var": "y"}, 2]}]}"#,
            r#"{"x": 1, "y": 2}"#
        ),
        4
    );
}

#[test]
fn the_count_is_the_same_for_the_same_data() {
    let rule = r#"{"filter": [{"var": "xs"}, {">": [{"var": ""}, 2]}]}"#;
    let data = array_of(50);
    let first = ops(rule, &data);
    for _ in 0..5 {
        assert_eq!(ops(rule, &data), first);
    }
}

// ---------------------------------------------------------------------------
// Iteration costs at least its input length — whichever path runs
// ---------------------------------------------------------------------------
//
// `filter` and the quantifiers recognise some predicate shapes at compile
// time and evaluate them inline, without dispatching the body. `map`
// fuses some body shapes the same way, and `reduce` folds arithmetic
// bodies directly. Those paths would cost 0 per item if the per-item
// charge lived in the body dispatch, which would make "does this rule fit
// its budget" depend on which shape the populate pass happened to
// recognise. Each case below pairs a rule that takes the fast path with
// one that cannot.

const N: usize = 100;

#[test]
fn filter_fast_predicate_costs_at_least_one_per_item() {
    // `{">": [{"var": ""}, 2]}` is a `FastPredicate::NumericCmp`.
    assert!(
        ops(
            r#"{"filter": [{"var": "xs"}, {">": [{"var": ""}, 2]}]}"#,
            &array_of(N)
        ) >= N as u64
    );
}

#[test]
fn filter_strict_eq_field_fast_path_costs_at_least_one_per_item() {
    let rows: Vec<String> = (0..N).map(|i| format!(r#"{{"id": {i}}}"#)).collect();
    let data = format!(r#"{{"xs": [{}]}}"#, rows.join(","));
    assert!(
        ops(
            r#"{"filter": [{"var": "xs"}, {"===": [{"var": "id"}, 7]}]}"#,
            &data
        ) >= N as u64
    );
}

#[test]
fn quantifier_fast_predicate_costs_at_least_one_per_item() {
    for op in ["all", "some", "none"] {
        let rule = format!(r#"{{"{op}": [{{"var": "xs"}}, {{">=": [{{"var": ""}}, 0]}}]}}"#);
        assert!(
            ops(&rule, &array_of(N)) >= N as u64,
            "{op} charged less than one per item"
        );
    }
}

#[test]
fn map_fused_body_costs_at_least_one_per_item() {
    // `{"*": [{"var": ""}, 2]}` is a fusible `ArithVarLit` body.
    assert!(ops(DOUBLE_XS, &array_of(N)) >= N as u64);
}

#[test]
fn reduce_fold_fast_path_costs_at_least_one_per_item() {
    let rule =
        r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#;
    assert!(ops(rule, &array_of(N)) >= N as u64);
}

#[test]
fn the_general_path_costs_at_least_one_per_item_too() {
    // A predicate shape no fast path recognises still charges per item.
    let rule = r#"{"filter": [{"var": "xs"}, {"if": [{">": [{"var": ""}, 2]}, true, false]}]}"#;
    assert!(ops(rule, &array_of(N)) >= N as u64);
}

#[test]
fn nested_iteration_multiplies() {
    // The shape a recursion-depth cap cannot see: one boundary call, N × M
    // items of work.
    let outer = 20;
    let inner: Vec<String> = (0..outer).map(|_| "[1,2,3,4,5]".to_string()).collect();
    let data = format!(r#"{{"xs": [{}]}}"#, inner.join(","));
    let rule = r#"{"map": [{"var": "xs"}, {"map": [{"var": ""}, {"*": [{"var": ""}, 2]}]}]}"#;
    assert!(ops(rule, &data) >= (outer * 5) as u64);
}

// ---------------------------------------------------------------------------
// The abort
// ---------------------------------------------------------------------------

#[test]
fn crossing_the_ceiling_is_refused() {
    let rule = DOUBLE_XS;
    let data = array_of(N);
    let err = under_budget(rule, &data, 10).expect_err("should be refused");
    assert_eq!(err.tag(), "BudgetExceeded");
}

#[test]
fn the_error_reports_the_ceiling_and_what_was_spent() {
    let rule = DOUBLE_XS;
    let err = under_budget(rule, &array_of(N), 10).expect_err("should be refused");
    let json: serde_json::Value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["type"], "BudgetExceeded");
    assert_eq!(json["budget"], 10);
    assert!(json["spent"].as_u64().expect("spent") > 10);
    assert!(err.to_string().contains("budget"), "{err}");
}

#[test]
fn the_error_carries_the_node_breadcrumb() {
    let rule = DOUBLE_XS;
    let err = under_budget(rule, &array_of(N), 10).expect_err("should be refused");
    assert!(
        !err.node_ids().is_empty(),
        "BudgetExceeded should be decorated like every other error"
    );
}

#[test]
fn a_budget_that_fits_is_not_refused() {
    let rule = DOUBLE_XS;
    let data = array_of(10);
    let spent = ops(rule, &data);
    assert_eq!(
        under_budget(rule, &data, spent).expect("exact budget fits"),
        spent
    );
    assert!(
        under_budget(rule, &data, spent - 1).is_err(),
        "one short is refused"
    );
}

#[cfg(feature = "error-handling")]
#[test]
fn try_cannot_recover_from_an_exhausted_budget() {
    // The catch arm is a literal, which returns before dispatch and so
    // would not charge anything: if `try` treated this like any other
    // error, the abort would not be an abort.
    let rule = format!(r#"{{"try": [{DOUBLE_XS}, "fallback"]}}"#);
    let err = under_budget(&rule, &array_of(N), 10).expect_err("should not be caught");
    assert_eq!(err.tag(), "BudgetExceeded");

    // Same with a catch arm that does dispatch.
    let rule = format!(r#"{{"try": [{DOUBLE_XS}, {{"var": "xs"}}]}}"#);
    let err = under_budget(&rule, &array_of(N), 10).expect_err("should not be caught");
    assert_eq!(err.tag(), "BudgetExceeded");
}

#[cfg(feature = "error-handling")]
#[test]
fn try_still_recovers_from_everything_else_under_a_budget() {
    let engine = Engine::builder()
        .with_config(EvaluationConfig::default().with_ops_budget(Some(1_000)))
        .build();
    let result = engine
        .eval_str(r#"{"try": [{"throw": "boom"}, "caught"]}"#, "null")
        .expect("ordinary errors stay catchable");
    assert_eq!(result, "\"caught\"");
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

#[test]
fn the_engine_wide_budget_applies_to_every_entry_point() {
    let engine = Engine::builder()
        .with_config(EvaluationConfig::default().with_ops_budget(Some(10)))
        .build();
    let rule = DOUBLE_XS;
    let data = array_of(N);

    assert_eq!(
        engine.eval_str(rule, &data).unwrap_err().tag(),
        "BudgetExceeded"
    );
    assert_eq!(
        engine.eval(rule, &data).unwrap_err().tag(),
        "BudgetExceeded"
    );

    let compiled = engine.compile(rule).expect("compile");
    let mut session = engine.session();
    assert_eq!(
        session.eval(&compiled, data.as_str()).unwrap_err().tag(),
        "BudgetExceeded"
    );
}

#[test]
fn the_per_call_budget_overrides_the_engine_wide_one() {
    let engine = Engine::builder()
        .with_config(EvaluationConfig::default().with_ops_budget(Some(10)))
        .build();
    let compiled = engine.compile(DOUBLE_XS).expect("compile");
    let arena = Bump::new();
    let data = array_of(N);

    // Wider than the engine default: allowed.
    let metered = engine
        .evaluate_metered(&compiled, data.as_str(), &arena, u64::MAX)
        .expect("per-call budget wins");
    assert!(metered.ops > 10);

    // Narrower: refused.
    assert!(
        engine
            .evaluate_metered(&compiled, data.as_str(), &arena, 5)
            .is_err()
    );
}

#[test]
fn session_eval_metered_reports_the_same_count() {
    let engine = Engine::new();
    let rule = r#"{"filter": [{"var": "xs"}, {">": [{"var": ""}, 2]}]}"#;
    let compiled = engine.compile(rule).expect("compile");
    let data = array_of(20);
    let mut session = engine.session();
    let Metered { value, ops: spent } = session
        .eval_metered(&compiled, data.as_str(), u64::MAX)
        .expect("eval");
    assert_eq!(value.as_array().expect("array").len(), 17);
    assert_eq!(spent, ops(rule, &data));
}

#[test]
fn an_unset_budget_evaluates_unbounded() {
    let engine = Engine::new();
    assert!(engine.config().ops_budget.is_none());
    let rule = DOUBLE_XS;
    assert!(engine.eval_str(rule, &array_of(10_000)).is_ok());
}

// ---------------------------------------------------------------------------
// Config wire format
// ---------------------------------------------------------------------------

#[test]
fn ops_budget_round_trips_through_the_json_config() {
    let config = EvaluationConfig::from_json_str(r#"{"ops_budget": 500}"#).expect("parse");
    assert_eq!(config.ops_budget, Some(500));

    let config = EvaluationConfig::from_json_str(r#"{"ops_budget": null}"#).expect("parse");
    assert_eq!(config.ops_budget, None);

    for bad in [
        r#"{"ops_budget": 0}"#,
        r#"{"ops_budget": -1}"#,
        r#"{"ops_budget": "many"}"#,
    ] {
        let err = EvaluationConfig::from_json_str(bad).expect_err(bad);
        assert_eq!(err.tag(), "ConfigurationError", "{bad}");
    }
}

// ---------------------------------------------------------------------------
// Operators that charge for themselves
// ---------------------------------------------------------------------------

#[cfg(feature = "tensor")]
#[test]
fn tensor_operators_charge_for_the_data_they_move() {
    // `zeros` allocates 1,000 elements from a rule of three nodes: the
    // node count alone would price this at nothing.
    let rule = r#"{"zeros": [[10, 10, 10], "f32"]}"#;
    assert!(ops(rule, "null") >= 1_000);
}

/// Every tensor operator's charge, against the table it was specified
/// with. This is the test that keeps the family honest: the whole reason
/// it is arithmetic-free is that each operator's cost is proportional to
/// the data it moves, and that claim is only worth anything if the
/// numbers are pinned.
///
/// Each case subtracts the cost of building its operands and the 1 the
/// dispatcher charges per node, leaving the operator's own charge. The
/// node count is spelled out per case because a bracketed operand list
/// (`stack`, `concat`) is itself a dispatched `Array` node.
#[cfg(feature = "tensor")]
#[test]
fn every_tensor_operator_charges_what_the_table_says() {
    /// `{"tensor": [[1,2,3,4], "u8"]}` — 1 node + numel 4.
    const T4: &str = r#"{"tensor": [[1,2,3,4], "u8"]}"#;
    /// The same four elements as 2x2 — T4 plus a `reshape` (1 node + 1).
    const T2X2: &str = r#"{"reshape": [{"tensor": [[1,2,3,4], "u8"]}, [2,2]]}"#;

    let b4 = ops(T4, "null");
    let b2x2 = ops(T2X2, "null");
    assert_eq!((b4, b2x2), (5, 7), "operand baselines moved");

    // (name, rule, operand cost to subtract, nodes this rule adds, expected charge)
    let cases: Vec<(&str, String, u64, u64, u64)> = vec![
        // Constructors: numel, except where the table says otherwise.
        ("tensor", T4.to_string(), 0, 1, 4),
        ("zeros", r#"{"zeros": [[4], "u8"]}"#.to_string(), 0, 1, 4),
        ("full", r#"{"full": [[4], "u8", 7]}"#.to_string(), 0, 1, 4),
        // max(points, numel) = max(2, 4).
        (
            "scatter",
            r#"{"scatter": [[[0],[2]], [4], "u8"]}"#.to_string(),
            0,
            1,
            4,
        ),
        // max(runs, numel) = max(2, 4).
        (
            "rle_expand",
            r#"{"rle_expand": [[1,2,3,2], [4], "u8"]}"#.to_string(),
            0,
            1,
            4,
        ),
        // len x depth = 3 x 4.
        (
            "one_hot",
            r#"{"one_hot": [[0,1,2], 4, "u8"]}"#.to_string(),
            0,
            1,
            12,
        ),
        // Shape moves. The operand list is an `Array` node, hence 2 nodes.
        (
            "stack",
            format!(r#"{{"stack": [[{T4}, {T4}], 0]}}"#),
            2 * b4,
            2,
            8,
        ),
        (
            "concat",
            format!(r#"{{"concat": [[{T4}, {T4}], 0]}}"#),
            2 * b4,
            2,
            8,
        ),
        (
            "unstack",
            format!(r#"{{"unstack": [{T2X2}, 0]}}"#),
            b2x2,
            1,
            4,
        ),
        // A new header over the same bytes: constant.
        (
            "reshape",
            format!(r#"{{"reshape": [{T4}, [2,2]]}}"#),
            b4,
            1,
            1,
        ),
        (
            "transpose",
            format!(r#"{{"transpose": [{T2X2}]}}"#),
            b2x2,
            1,
            4,
        ),
        // max(in, out): 4 -> 6 grows, 4 -> 2 shrinks.
        ("pad", format!(r#"{{"pad": [{T4}, [1], [1]]}}"#), b4, 1, 6),
        ("crop", format!(r#"{{"crop": [{T4}, [1], [2]]}}"#), b4, 1, 4),
        (
            "gather",
            format!(r#"{{"gather": [{T4}, [0,1]]}}"#),
            b4,
            1,
            4,
        ),
        // Readers.
        ("cast", format!(r#"{{"cast": [{T4}, "f32"]}}"#), b4, 1, 4),
        (
            "normalize",
            format!(r#"{{"normalize": [{T4}, 0]}}"#),
            b4,
            1,
            4,
        ),
        (
            "argmax",
            format!(r#"{{"argmax": [{T2X2}, 1]}}"#),
            b2x2,
            1,
            4,
        ),
        ("to_list", format!(r#"{{"to_list": [{T4}]}}"#), b4, 1, 4),
        // Header reads: constant.
        ("shape", format!(r#"{{"shape": [{T4}]}}"#), b4, 1, 1),
        ("dtype", format!(r#"{{"dtype": [{T4}]}}"#), b4, 1, 1),
    ];

    assert_eq!(cases.len(), 20, "the family is 20 operators");
    for (name, rule, operands, nodes, expected) in cases {
        let charged = ops(&rule, "null") - operands - nodes;
        assert_eq!(
            charged, expected,
            "{name} charged {charged}, table says {expected}"
        );
    }
}

/// A tensor must never escape as `null`. The engine's own half of that:
/// it renders as the tagged form from `eval_str`, nested inside an array
/// and an iterator result, and in a trace step — which goes through a
/// different serializer than the result does.
#[cfg(all(feature = "tensor", feature = "trace"))]
#[test]
fn a_tensor_renders_as_the_tagged_form_everywhere_it_can_escape() {
    const TAGGED: &str = r#"{"tensor":{"dtype":"u8","shape":[3],"data":"AQID"}}"#;
    let engine = Engine::new();

    assert_eq!(
        engine
            .eval_str(r#"{"tensor": [[1,2,3], "u8"]}"#, "null")
            .unwrap(),
        TAGGED
    );
    assert_eq!(
        engine
            .eval_str(r#"[{"tensor": [[1,2,3], "u8"]}]"#, "null")
            .unwrap(),
        format!("[{TAGGED}]")
    );

    // Trace renders steps through the serde bridge rather than the result
    // serializer; a missing arm there would surface as `null` per step.
    let run = engine
        .trace()
        .eval_str(r#"{"tensor": [[1,2,3], "u8"]}"#, "null");
    let result = run.result.expect("traced eval");
    assert!(result.contains("\"tensor\""), "traced result: {result}");
    let last = run.steps.last().expect("at least one step");
    let step = serde_json::to_string(&last.result).expect("step json");
    assert!(step.contains("\"tensor\""), "traced step: {step}");
    assert!(
        step.contains("AQID"),
        "traced step lost its payload: {step}"
    );
}

#[cfg(feature = "tensor")]
#[test]
fn a_tensor_too_big_for_the_budget_is_refused_before_it_is_allocated() {
    let rule = r#"{"zeros": [[1000, 1000, 100], "f64"]}"#;
    let err = under_budget(rule, "null", 1_000).expect_err("should be refused");
    assert_eq!(err.tag(), "BudgetExceeded");
}

// ---------------------------------------------------------------------------
// Custom operators
// ---------------------------------------------------------------------------

mod custom {
    use super::*;
    use datalogic_rs::{CustomOperator, DataValue, Result, operator::EvalContext};

    /// Charges `n` and returns it, so a test can hand-count the total.
    struct Charger;

    impl CustomOperator for Charger {
        fn evaluate<'a>(
            &self,
            args: &[&'a DataValue<'a>],
            ctx: &mut EvalContext<'_, 'a>,
            arena: &'a bumpalo::Bump,
        ) -> Result<&'a DataValue<'a>> {
            let n = args.first().and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64;
            ctx.charge(n)?;
            Ok(datalogic_rs::ArenaExt::i64(arena, n as i64))
        }
    }

    fn engine() -> Engine {
        Engine::builder().add_operator("charge", Charger).build()
    }

    #[test]
    fn a_custom_operators_charge_lands_in_the_total() {
        let engine = engine();
        let compiled = engine.compile(r#"{"charge": [40]}"#).expect("compile");
        let arena = Bump::new();
        let metered = engine
            .evaluate_metered(&compiled, "null", &arena, u64::MAX)
            .expect("eval");
        // 1 for the operator node, 40 charged by the operator. The `40`
        // argument is a literal and costs nothing.
        assert_eq!(metered.ops, 41);
    }

    #[test]
    fn a_custom_operator_can_be_refused_by_the_budget() {
        let engine = engine();
        let compiled = engine.compile(r#"{"charge": [1000]}"#).expect("compile");
        let arena = Bump::new();
        let err = engine
            .evaluate_metered(&compiled, "null", &arena, 100)
            .expect_err("should be refused");
        assert_eq!(err.tag(), "BudgetExceeded");
    }
}
