//! `CustomOperator::info`: what the engine may assume about a custom
//! operator. Declaring it deterministic lets a call with constant arguments
//! fold and makes `Facts` precise; declaring that it does not read the
//! context completes `Facts::reads`; a declared argument count is checked
//! before any argument is evaluated.
#![cfg(all(feature = "serde_json", feature = "all-operators"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use datalogic_rs::{
    ArenaExt, CustomOperator, CustomOperatorInfo, DataValue, Engine, ErrorKind, Result,
    operator::EvalContext,
};

/// Doubles its first argument and counts its calls. Its `info` is whatever
/// the test declares.
struct Double {
    info: CustomOperatorInfo,
    calls: Arc<AtomicUsize>,
}

impl Double {
    fn new(info: CustomOperatorInfo) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Double {
                info,
                calls: calls.clone(),
            },
            calls,
        )
    }
}

impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match args.first().and_then(|v| v.as_i64()) {
            Some(n) => Ok(arena.i64(n * 2)),
            None => Err(datalogic_rs::Error::invalid_arguments("not a number")),
        }
    }

    fn info(&self) -> CustomOperatorInfo {
        self.info
    }
}

/// An operator that implements only `evaluate`.
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

fn engine_with(info: CustomOperatorInfo) -> (Engine, Arc<AtomicUsize>) {
    let (op, calls) = Double::new(info);
    (Engine::builder().add_operator("double", op).build(), calls)
}

fn eval(engine: &Engine, rule: &str, data: &str) -> String {
    match engine.compile(rule) {
        Ok(logic) => match engine.session().eval_str(&logic, data) {
            Ok(v) => v,
            Err(e) => format!("err {}", e.tag()),
        },
        Err(e) => format!("compile err {}", e.tag()),
    }
}

// ── the declaration ─────────────────────────────────────────────────────

#[test]
fn an_operator_that_declares_nothing_is_opaque() {
    let info = Plain.info();
    assert_eq!(info, CustomOperatorInfo::opaque());
    assert!(!info.deterministic);
    assert!(info.reads_context);
    assert_eq!((info.min_args, info.max_args), (0, None));
    assert_eq!(CustomOperatorInfo::default(), CustomOperatorInfo::opaque());
}

#[test]
fn the_constructors_compose() {
    let pure = CustomOperatorInfo::pure();
    assert!(pure.deterministic && !pure.reads_context);
    let ctx = CustomOperatorInfo::pure().reading_context();
    assert!(ctx.deterministic && ctx.reads_context);
    let arity = CustomOperatorInfo::pure().with_args(1, Some(2));
    assert_eq!((arity.min_args, arity.max_args), (1, Some(2)));
    assert!(arity.deterministic);
}

#[test]
fn the_engine_reports_each_operator_info() {
    let (op, _) = Double::new(CustomOperatorInfo::pure().with_args(1, Some(1)));
    let engine = Engine::builder()
        .add_operator("double", op)
        .add_operator("plain", Plain)
        .build();
    assert_eq!(
        engine.custom_operator_info("double"),
        Some(CustomOperatorInfo::pure().with_args(1, Some(1)))
    );
    assert_eq!(
        engine.custom_operator_info("plain"),
        Some(CustomOperatorInfo::opaque())
    );
    assert_eq!(engine.custom_operator_info("missing"), None);
    // A built-in is not a custom operator.
    assert_eq!(engine.custom_operator_info("+"), None);
}

#[test]
fn arc_and_box_forward_the_declaration() {
    let info = CustomOperatorInfo::pure().with_args(1, None);
    let (op, _) = Double::new(info);
    let shared: Arc<dyn CustomOperator> = Arc::new(op);
    assert_eq!(shared.info(), info);
    let (op, _) = Double::new(info);
    let boxed: Box<dyn CustomOperator> = Box::new(op);
    assert_eq!(boxed.info(), info);
    let engine = Engine::builder().add_operator("double", shared).build();
    assert_eq!(engine.custom_operator_info("double"), Some(info));
}

// ── folding ─────────────────────────────────────────────────────────────

#[test]
fn a_pure_call_with_constant_arguments_folds() {
    let (engine, calls) = engine_with(CustomOperatorInfo::pure());
    let logic = engine.compile(r#"{"double": [21]}"#).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "evaluated once, at compile time"
    );
    let mut session = engine.session();
    for _ in 0..3 {
        assert_eq!(session.eval_str(&logic, "null").unwrap(), "42");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1, "never at evaluation");
    assert_eq!(logic.to_json(), "42");
}

#[test]
fn a_folded_call_lets_its_parent_fold() {
    let (engine, calls) = engine_with(CustomOperatorInfo::pure());
    let logic = engine
        .compile(r#"{"+": [{"double": [{"*": [3, 7]}]}, 1]}"#)
        .unwrap();
    assert_eq!(logic.to_json(), "43");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_opaque_call_never_folds() {
    let (engine, calls) = engine_with(CustomOperatorInfo::opaque());
    let logic = engine.compile(r#"{"double": [21]}"#).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut session = engine.session();
    session.eval_str(&logic, "null").unwrap();
    session.eval_str(&logic, "null").unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_deterministic_call_that_reads_the_context_never_folds() {
    let (engine, calls) = engine_with(CustomOperatorInfo::pure().reading_context());
    let logic = engine.compile(r#"{"double": [21]}"#).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(engine.session().eval_str(&logic, "null").unwrap(), "42");
}

#[test]
fn a_pure_call_with_a_data_argument_does_not_fold() {
    let (engine, calls) = engine_with(CustomOperatorInfo::pure());
    let logic = engine.compile(r#"{"double": [{"var": "n"}]}"#).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        engine.session().eval_str(&logic, r#"{"n": 4}"#).unwrap(),
        "8"
    );
}

#[test]
fn a_pure_call_that_fails_is_left_for_evaluation() {
    let (engine, _) = engine_with(CustomOperatorInfo::pure());
    // The fold attempt fails, so the error is raised where the call runs,
    // and not at all when it does not.
    assert_eq!(
        eval(&engine, r#"{"double": ["x"]}"#, "null"),
        "err InvalidArguments"
    );
    assert_eq!(
        eval(
            &engine,
            r#"{"if": [{"var": "go"}, {"double": ["x"]}, 0]}"#,
            "{}"
        ),
        "0"
    );
}

#[test]
fn folding_off_means_no_fold() {
    let (op, calls) = Double::new(CustomOperatorInfo::pure());
    let engine = Engine::builder()
        .add_operator("double", op)
        .with_constant_folding(false)
        .build();
    let logic = engine.compile(r#"{"double": [21]}"#).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(engine.session().eval_str(&logic, "null").unwrap(), "42");
}

#[cfg(feature = "trace")]
#[test]
fn a_trace_runs_the_call() {
    let (engine, calls) = engine_with(CustomOperatorInfo::pure());
    let run = engine.trace().eval_str(r#"{"double": [21]}"#, "null");
    assert_eq!(run.result.unwrap(), "42");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "templating")]
#[test]
fn a_pure_call_in_a_template_folds() {
    let (op, calls) = Double::new(CustomOperatorInfo::pure());
    let engine = Engine::builder()
        .with_templating(true)
        .add_operator("double", op)
        .build();
    let logic = engine
        .compile(r#"{"a": {"double": [2]}, "b": {"var": "x"}}"#)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.session().eval_str(&logic, r#"{"x": 1}"#).unwrap(),
        r#"{"a":4,"b":1}"#
    );
}

// ── facts ───────────────────────────────────────────────────────────────

#[test]
fn facts_trust_a_deterministic_operator() {
    let (engine, _) = engine_with(CustomOperatorInfo::pure());
    let f = engine
        .compile(r#"{"double": [{"var": "n"}]}"#)
        .unwrap()
        .facts();
    assert!(f.is_deterministic());
    assert!(f.reads_complete());
    assert_eq!(f.custom_operators(), ["double"]);
    let reads: Vec<String> = f.reads().iter().map(|p| p.to_string()).collect();
    assert_eq!(reads, ["n"]);
}

#[test]
fn facts_distrust_an_opaque_operator() {
    let (engine, _) = engine_with(CustomOperatorInfo::opaque());
    let f = engine
        .compile(r#"{"double": [{"var": "n"}]}"#)
        .unwrap()
        .facts();
    assert!(!f.is_deterministic());
    assert!(!f.reads_complete());
    assert!(f.reads_data());
}

#[test]
fn a_context_reader_leaves_reads_incomplete_but_stays_deterministic() {
    let (engine, _) = engine_with(CustomOperatorInfo::pure().reading_context());
    let f = engine.compile(r#"{"double": [1]}"#).unwrap().facts();
    assert!(f.is_deterministic());
    assert!(!f.reads_complete());
    assert!(f.reads_data());
}

#[test]
fn a_folded_call_leaves_no_trace_in_the_facts() {
    let (engine, _) = engine_with(CustomOperatorInfo::pure());
    let f = engine.compile(r#"{"double": [21]}"#).unwrap().facts();
    assert!(f.custom_operators().is_empty());
    assert!(!f.reads_data());
}

#[test]
fn facts_describe_the_operator_the_rule_was_compiled_with() {
    let (engine, _) = engine_with(CustomOperatorInfo::pure());
    // Unknown when compiled: opaque.
    let f = Engine::new()
        .compile(r#"{"double": [{"var": "n"}]}"#)
        .unwrap()
        .facts();
    assert!(!f.is_deterministic());
    drop(engine);
}

// ── argument count ──────────────────────────────────────────────────────

#[test]
fn a_declared_argument_count_is_checked_before_any_argument_runs() {
    let (engine, calls) = engine_with(CustomOperatorInfo::opaque().with_args(1, Some(1)));
    let too_few = eval(&engine, r#"{"double": []}"#, "null");
    let too_many = eval(&engine, r#"{"double": [1, {"throw": "x"}]}"#, "null");
    assert_eq!(too_few, "err InvalidArguments");
    // `throw` never ran: the count failed first.
    assert_eq!(too_many, "err InvalidArguments");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(eval(&engine, r#"{"double": 5}"#, "null"), "10");
}

#[test]
fn an_undeclared_argument_count_is_the_operator_own_business() {
    let (engine, calls) = engine_with(CustomOperatorInfo::opaque());
    assert_eq!(eval(&engine, r#"{"double": [5, 6, 7]}"#, "null"), "10");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_minimum_alone_bounds_from_below() {
    let (engine, _) = engine_with(CustomOperatorInfo::opaque().with_args(2, None));
    assert_eq!(
        eval(&engine, r#"{"double": [5]}"#, "null"),
        "err InvalidArguments"
    );
    assert_eq!(eval(&engine, r#"{"double": [5, 1, 2, 3]}"#, "null"), "10");
}

#[test]
fn the_count_error_is_the_canonical_one() {
    let (engine, _) = engine_with(CustomOperatorInfo::opaque().with_args(1, Some(1)));
    let logic = engine.compile(r#"{"double": []}"#).unwrap();
    let err = engine.session().eval_str(&logic, "null").unwrap_err();
    assert!(
        matches!(&err.kind, ErrorKind::InvalidArguments(m) if m == "Invalid Arguments"),
        "{err:?}"
    );
    assert_eq!(err.operator(), Some("double"));
}
