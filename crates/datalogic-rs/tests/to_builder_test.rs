//! `Engine::to_builder` rebuilds an engine with the same operators and
//! settings. A host that reloads (dataflow on every config change) starts
//! from the running engine instead of registering every operator again.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    ArenaExt, CustomOperator, CustomOperatorInfo, DataValue, Engine, EvaluationConfig, MissingVar,
    Result,
};

/// Counts its calls, so a test can see whether two engines share it.
struct Counter(Arc<AtomicUsize>);

impl CustomOperator for Counter {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(arena.i64(n as i64))
    }
}

/// Returns its tag.
struct Tag(i64);

impl CustomOperator for Tag {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.i64(self.0))
    }

    fn info(&self) -> CustomOperatorInfo {
        CustomOperatorInfo::pure().with_args(0, Some(1))
    }
}

fn run(engine: &Engine, rule: &str, data: &str) -> String {
    match engine.eval_str(rule, data) {
        Ok(v) => v,
        Err(e) => format!("err {}", e.tag()),
    }
}

#[test]
fn the_rebuilt_engine_shares_each_operator_instance() {
    let calls = Arc::new(AtomicUsize::new(0));
    let original = Engine::builder()
        .add_operator("count", Counter(Arc::clone(&calls)))
        .build();
    let rebuilt = original.to_builder().build();
    assert_eq!(run(&original, r#"{"count": []}"#, "null"), "1");
    assert_eq!(run(&rebuilt, r#"{"count": []}"#, "null"), "2");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn the_rebuilt_engine_outlives_the_original() {
    let original = Engine::builder().add_operator("tag", Tag(4)).build();
    let rebuilt = original.to_builder().build();
    drop(original);
    assert_eq!(run(&rebuilt, r#"{"tag": []}"#, "null"), "4");
}

#[test]
fn operators_added_to_the_rebuild_leave_the_original_alone() {
    let original = Engine::builder().add_operator("a", Tag(1)).build();
    let rebuilt = original
        .to_builder()
        .add_operator("b", Tag(2))
        .add_operator("a", Tag(10))
        .build();
    assert_eq!(run(&rebuilt, r#"[{"a": []}, {"b": []}]"#, "null"), "[10,2]");
    assert_eq!(run(&original, r#"{"a": []}"#, "null"), "1");
    assert!(!original.has_custom_operator("b"));
    let mut names: Vec<&str> = rebuilt.custom_operator_names().collect();
    names.sort_unstable();
    assert_eq!(names, ["a", "b"]);
}

#[test]
fn the_declared_info_carries_over() {
    let original = Engine::builder().add_operator("tag", Tag(1)).build();
    let rebuilt = original.to_builder().build();
    assert_eq!(
        rebuilt.custom_operator_info("tag"),
        original.custom_operator_info("tag")
    );
    // Declared pure, so a call with constant arguments still folds.
    assert!(rebuilt.compile(r#"{"tag": []}"#).unwrap().is_static());
}

#[test]
fn a_rule_compiled_elsewhere_runs_on_the_rebuild() {
    let original = Engine::builder()
        .add_operator("a", Tag(1))
        .add_operator("b", Tag(2))
        .build();
    let rebuilt = original.to_builder().add_operator("b", Tag(20)).build();
    // Not folded, so the call reaches dispatch on the rebuilt engine.
    let logic = Engine::builder()
        .add_operator("a", Tag(1))
        .add_operator("b", Tag(2))
        .with_constant_folding(false)
        .build()
        .compile(r#"[{"a": []}, {"b": []}]"#)
        .unwrap();
    assert_eq!(
        rebuilt.session().eval_str(&logic, "null").unwrap(),
        "[1,20]"
    );
}

#[test]
fn the_config_carries_over() {
    let original = Engine::builder()
        .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
        .build();
    let rebuilt = original.to_builder().build();
    assert_eq!(
        format!("{:?}", rebuilt.config()),
        format!("{:?}", original.config())
    );
    assert_eq!(
        run(&rebuilt, r#"{"var": "x"}"#, "{}"),
        "err VariableNotFound"
    );
}

#[test]
fn the_folding_setting_carries_over() {
    let unfolded = Engine::builder().with_constant_folding(false).build();
    let rebuilt = unfolded.to_builder().build();
    assert_eq!(
        rebuilt.compile(r#"{"+": [1, 2]}"#).unwrap().to_json(),
        unfolded.compile(r#"{"+": [1, 2]}"#).unwrap().to_json()
    );
    assert_ne!(
        rebuilt.compile(r#"{"+": [1, 2]}"#).unwrap().to_json(),
        Engine::new().compile(r#"{"+": [1, 2]}"#).unwrap().to_json()
    );
}

#[cfg(feature = "templating")]
#[test]
fn templating_and_its_escape_carry_over() {
    let original = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .build();
    let rebuilt = original.to_builder().build();
    let rule = r#"{"a": {"var": "x"}, "$if": 1}"#;
    assert_eq!(
        run(&rebuilt, rule, r#"{"x": 2}"#),
        run(&original, rule, r#"{"x": 2}"#)
    );
    assert_eq!(run(&rebuilt, rule, r#"{"x": 2}"#), r#"{"a":2,"if":1}"#);
}

#[test]
fn settings_can_be_changed_on_the_rebuild() {
    let original = Engine::builder().add_operator("tag", Tag(3)).build();
    let rebuilt = original
        .to_builder()
        .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
        .build();
    assert_eq!(run(&rebuilt, r#"{"tag": []}"#, "null"), "3");
    assert_eq!(
        run(&rebuilt, r#"{"var": "x"}"#, "{}"),
        "err VariableNotFound"
    );
    assert_eq!(run(&original, r#"{"var": "x"}"#, "{}"), "null");
}

#[test]
fn a_plain_engine_rebuilds_to_a_plain_engine() {
    let rebuilt = Engine::new().to_builder().build();
    assert_eq!(rebuilt.custom_operator_names().count(), 0);
    assert_eq!(
        format!("{:?}", rebuilt.config()),
        format!("{:?}", Engine::new().config())
    );
    assert_eq!(
        run(&rebuilt, r#"{"+": [1, {"var": "x"}]}"#, r#"{"x": 2}"#),
        "3"
    );
}

#[test]
fn an_arc_operator_is_shared_with_the_host() {
    let calls = Arc::new(AtomicUsize::new(0));
    let op = Arc::new(Counter(Arc::clone(&calls)));
    let engine = Engine::builder()
        .add_operator("count", Arc::clone(&op))
        .build();
    let rebuilt = engine.to_builder().build();
    run(&engine, r#"{"count": []}"#, "null");
    run(&rebuilt, r#"{"count": []}"#, "null");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // The host's handle plus the one registration both engines share.
    assert_eq!(Arc::strong_count(&op), 2);
}
