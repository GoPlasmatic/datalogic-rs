//! `CustomOperator::check`: a custom operator validates its arguments as
//! written when a rule is checked (`Engine::check`, `compile_checked`),
//! before anything runs. `compile` is unchanged.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    ArenaExt, CheckMode, CustomOperator, CustomOperatorInfo, DataValue, Diagnostic, DiagnosticCode,
    Engine, Result, Severity, datavalue::OwnedDataValue,
};

fn ok<'a>(arena: &'a datalogic_rs::bumpalo::Bump) -> Result<&'a DataValue<'a>> {
    Ok(arena.i64(0))
}

/// Accepts anything: the default `check`.
struct Plain;

impl CustomOperator for Plain {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        ok(arena)
    }
}

/// `{"lookup": [table, key]}`: the table must be a literal string naming a
/// known table. A computed table is allowed with a warning.
struct Lookup;

impl CustomOperator for Lookup {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        ok(arena)
    }

    fn info(&self) -> CustomOperatorInfo {
        CustomOperatorInfo::opaque().with_args(2, Some(2))
    }

    fn check(&self, args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> {
        match &args[0] {
            OwnedDataValue::String(name) if name == "users" || name == "orders" => Ok(()),
            OwnedDataValue::String(name) => {
                Err(Diagnostic::error(format!("no table named {name:?}")).at_argument(0))
            }
            OwnedDataValue::Object(_) => Err(Diagnostic::warning(
                "a computed table name is only checked when the rule runs",
            )
            .at_argument(0)),
            _ => Err(Diagnostic::error("the table name must be a string")),
        }
    }
}

/// Records every argument list it is asked to check.
struct Recorder(Arc<std::sync::Mutex<Vec<Vec<OwnedDataValue>>>>);

impl CustomOperator for Recorder {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        ok(arena)
    }

    fn check(&self, args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> {
        self.0.lock().unwrap().push(args.to_vec());
        Ok(())
    }
}

/// Counts how often `check` runs.
struct CountChecks(Arc<AtomicUsize>);

impl CustomOperator for CountChecks {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        ok(arena)
    }

    fn info(&self) -> CustomOperatorInfo {
        CustomOperatorInfo::opaque().with_args(1, Some(1))
    }

    fn check(&self, _args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn engine() -> Engine {
    Engine::builder()
        .add_operator("plain", Plain)
        .add_operator("lookup", Lookup)
        .build()
}

fn check(engine: &Engine, rule: &str) -> Vec<Diagnostic> {
    engine.check(rule, CheckMode::Engine)
}

#[test]
fn the_default_check_accepts_anything() {
    assert!(check(&engine(), r#"{"plain": [1, {"var": "x"}, "y"]}"#).is_empty());
}

#[test]
fn a_rejected_argument_is_an_error_at_that_argument() {
    let d = check(&engine(), r#"{"lookup": ["accounts", {"var": "id"}]}"#);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, DiagnosticCode::OperatorCheck);
    assert_eq!(d[0].severity, Severity::Error);
    assert_eq!(d[0].pointer, "/lookup/0");
    assert_eq!(d[0].operator.as_deref(), Some("lookup"));
    assert_eq!(d[0].message, r#"no table named "accounts""#);
}

#[test]
fn a_known_value_passes() {
    assert!(check(&engine(), r#"{"lookup": ["users", {"var": "id"}]}"#).is_empty());
}

#[test]
fn a_problem_with_the_whole_call_points_at_the_call() {
    let d = check(&engine(), r#"{"if": [true, {"lookup": [1, 2]}]}"#);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].pointer, "/if/1");
    assert_eq!(d[0].message, "the table name must be a string");
}

#[test]
fn a_warning_does_not_block_compile_checked() {
    let rule = r#"{"lookup": [{"cat": ["us", "ers"]}, 1]}"#;
    let d = check(&engine(), rule);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].severity, Severity::Warning);
    assert_eq!(d[0].pointer, "/lookup/0");
    assert!(engine().compile_checked(rule).is_ok());
}

#[test]
fn an_error_blocks_compile_checked_but_not_compile() {
    let rule = r#"{"lookup": ["accounts", 1]}"#;
    let err = engine().compile_checked(rule).unwrap_err();
    assert_eq!(err.diagnostics.len(), 1);
    assert_eq!(err.diagnostics[0].code, DiagnosticCode::OperatorCheck);
    assert!(engine().compile(rule).is_ok());
}

#[test]
fn arguments_arrive_as_written() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let engine = Engine::builder()
        .add_operator("rec", Recorder(Arc::clone(&seen)))
        .build();
    check(&engine, r#"{"rec": [1, "a", {"var": "x"}, [2, 3]]}"#);
    // A single argument that is not an array is the whole argument list.
    check(&engine, r#"{"rec": "solo"}"#);
    check(&engine, r#"{"rec": []}"#);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert_eq!(
        seen[0],
        vec![
            OwnedDataValue::from(1i64),
            OwnedDataValue::String("a".into()),
            OwnedDataValue::Object(vec![("var".into(), OwnedDataValue::String("x".into()))]),
            OwnedDataValue::Array(vec![OwnedDataValue::from(2i64), OwnedDataValue::from(3i64)]),
        ]
    );
    assert_eq!(seen[1], vec![OwnedDataValue::String("solo".into())]);
    assert!(seen[2].is_empty());
}

#[test]
fn a_single_argument_is_argument_zero() {
    let d = check(&engine(), r#"{"lookup": "accounts"}"#);
    // Wrong count first: one argument where two are declared.
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, DiagnosticCode::ArgumentCount);

    let engine = Engine::builder().add_operator("one", OneArg).build();
    let d = check(&engine, r#"{"one": "bad"}"#);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].pointer, "/one");
}

/// Rejects its only argument.
struct OneArg;

impl CustomOperator for OneArg {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        ok(arena)
    }

    fn check(&self, _args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> {
        Err(Diagnostic::error("never").at_argument(0))
    }
}

#[test]
fn a_wrong_count_skips_the_operator_check() {
    let count = Arc::new(AtomicUsize::new(0));
    let engine = Engine::builder()
        .add_operator("c", CountChecks(Arc::clone(&count)))
        .build();
    let d = check(&engine, r#"{"c": [1, 2]}"#);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].code, DiagnosticCode::ArgumentCount);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(check(&engine, r#"{"c": [1]}"#).is_empty());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn nested_calls_are_still_checked() {
    let d = check(
        &engine(),
        r#"{"lookup": ["users", {"lookup": ["nope", 1]}]}"#,
    );
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].pointer, "/lookup/1/lookup/0");
}

#[test]
fn every_problem_is_reported_in_rule_order() {
    let d = check(
        &engine(),
        r#"[{"lookup": ["a", 1]}, {"vr": 1}, {"lookup": ["b", 1]}]"#,
    );
    let got: Vec<(&str, DiagnosticCode)> = d.iter().map(|d| (d.pointer.as_str(), d.code)).collect();
    assert_eq!(
        got,
        [
            ("/0/lookup/0", DiagnosticCode::OperatorCheck),
            ("/1", DiagnosticCode::UnknownOperator),
            ("/2/lookup/0", DiagnosticCode::OperatorCheck),
        ]
    );
}

#[test]
fn a_pointer_escapes_the_operator_name() {
    let engine = Engine::builder().add_operator("a/b~c", OneArg).build();
    let d = check(&engine, r#"{"a/b~c": [1]}"#);
    assert_eq!(d[0].pointer, "/a~1b~0c/0");
}

#[test]
fn box_and_arc_forward_the_check() {
    let boxed: Box<dyn CustomOperator> = Box::new(Lookup);
    let engine = Engine::builder()
        .add_operator("boxed", boxed)
        .add_operator("shared", Arc::new(Lookup))
        .build();
    let d = check(&engine, r#"[{"boxed": ["x", 1]}, {"shared": ["y", 1]}]"#);
    assert_eq!(d.len(), 2, "{d:?}");
    assert!(d.iter().all(|d| d.code == DiagnosticCode::OperatorCheck));
}

#[cfg(feature = "templating")]
#[test]
fn a_template_checks_its_calls() {
    let engine = Engine::builder()
        .add_operator("lookup", Lookup)
        .with_templating(true)
        .build();
    let d = engine.check(
        r#"{"a": {"lookup": ["nope", 1]}, "b": 2}"#,
        CheckMode::Engine,
    );
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].pointer, "/a/lookup/0");
}

#[cfg(feature = "serde_json")]
#[test]
fn the_code_serialises_by_name() {
    let d = check(&engine(), r#"{"lookup": ["nope", 1]}"#);
    let v = serde_json::to_value(&d[0]).unwrap();
    assert_eq!(v["code"], "OperatorCheck");
    assert_eq!(v["severity"], "error");
    assert_eq!(v["pointer"], "/lookup/0");
}

#[test]
fn the_constructors_describe_the_problem() {
    let d = Diagnostic::warning("w");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.code, DiagnosticCode::OperatorCheck);
    assert_eq!(d.message, "w");
    assert_eq!(d.pointer, "");
    assert_eq!(d.operator, None);
    assert_eq!(Diagnostic::error("e").at_argument(3).pointer, "/3");
}
