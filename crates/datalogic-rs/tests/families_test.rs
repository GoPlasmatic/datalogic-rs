//! Operator families at runtime: `EngineBuilder::with_families` keeps an
//! engine to the JSONLogic core plus the families it names, whatever this
//! build compiled in. A family left out is not there for that engine: its
//! names are unknown operators, template output fields, or free for a
//! custom operator.

#![cfg(all(
    feature = "ext-string",
    feature = "ext-array",
    feature = "datetime",
    feature = "templating"
))]

use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    ArenaExt, CheckMode, CustomOperator, DataValue, DiagnosticCode, Engine, Family, Result,
};

fn run(engine: &Engine, rule: &str, data: &str) -> String {
    match engine.eval_str(rule, data) {
        Ok(v) => v,
        Err(e) => format!("err {}", e.tag()),
    }
}

/// Returns 99, so a test can see which `length` ran.
struct NinetyNine;

impl CustomOperator for NinetyNine {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.i64(99))
    }
}

fn core_only() -> Engine {
    Engine::builder().with_families([]).build()
}

#[test]
fn by_default_every_compiled_family_is_there() {
    let engine = Engine::new();
    assert_eq!(run(&engine, r#"{"length": "abc"}"#, "null"), "3");
    let families: std::collections::BTreeSet<&str> =
        engine.operators().map(|op| op.family).collect();
    assert!(families.contains("ExtString") && families.contains("DateTime"));
}

#[test]
fn the_core_is_always_there() {
    let engine = core_only();
    assert_eq!(
        run(&engine, r#"{"+": [1, {"var": "x"}]}"#, r#"{"x": 2}"#),
        "3"
    );
    assert_eq!(
        run(
            &engine,
            r#"{"map": [[1, 2], {"*": [{"var": ""}, 2]}]}"#,
            "null"
        ),
        "[2,4]"
    );
}

#[test]
fn a_family_left_out_is_an_unknown_operator() {
    let engine = core_only();
    assert_eq!(
        run(&engine, r#"{"length": "abc"}"#, "null"),
        "err InvalidOperator"
    );
    assert_eq!(
        run(&engine, r#"{"sort": [[2, 1]]}"#, "null"),
        "err InvalidOperator"
    );
    // As for any unknown operator, `compile_checked` refuses it up front.
    assert!(engine.compile_checked(r#"{"sort": [[2, 1]]}"#).is_err());
}

#[test]
fn a_named_family_is_there_and_no_other() {
    let engine = Engine::builder().with_families([Family::ExtString]).build();
    assert_eq!(run(&engine, r#"{"length": "abc"}"#, "null"), "3");
    assert_eq!(
        run(&engine, r#"{"sort": [[2, 1]]}"#, "null"),
        "err InvalidOperator"
    );
    assert_eq!(
        run(&engine, r#"{"now": []}"#, "null"),
        "err InvalidOperator"
    );
}

#[test]
fn the_catalogue_follows_the_families() {
    let engine = Engine::builder().with_families([Family::ExtArray]).build();
    let families: std::collections::BTreeSet<&str> =
        engine.operators().map(|op| op.family).collect();
    assert_eq!(
        families.into_iter().collect::<Vec<_>>(),
        ["Core", "ExtArray"]
    );
    let names: Vec<&str> = engine.builtin_operator_names().collect();
    assert!(names.contains(&"sort") && names.contains(&"+"));
    assert!(!names.contains(&"length"));
}

#[test]
fn a_left_out_name_is_free_for_a_custom_operator() {
    let engine = Engine::builder()
        .with_families([])
        .add_operator("length", NinetyNine)
        .build();
    assert_eq!(run(&engine, r#"{"length": "abc"}"#, "null"), "99");
    // `try_add_operator` refuses only a name a built-in of this engine
    // answers to.
    assert!(
        Engine::builder()
            .with_families([])
            .try_add_operator("length", NinetyNine)
            .is_ok()
    );
    assert!(
        Engine::builder()
            .try_add_operator("length", NinetyNine)
            .is_err()
    );
    assert!(
        Engine::builder()
            .with_families([])
            .try_add_operator("+", NinetyNine)
            .is_err()
    );
}

#[test]
fn the_order_of_builder_calls_does_not_matter() {
    // Families named after the operator: the operator still answers to its
    // name, and `try_add_operator` judged it against the families set then.
    let engine = Engine::builder()
        .add_operator("length", NinetyNine)
        .with_families([])
        .build();
    assert_eq!(run(&engine, r#"{"length": "abc"}"#, "null"), "99");
}

#[test]
fn check_reports_a_left_out_name_and_suggests_only_what_is_there() {
    let engine = core_only();
    let d = engine.check(r#"{"length": "abc"}"#, CheckMode::Engine);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, DiagnosticCode::UnknownOperator);
    // `lenght` is one edit from `length`, which this engine lacks.
    let d = engine.check(r#"{"lenght": "abc"}"#, CheckMode::Engine);
    assert!(!d[0].message.contains("length"), "{}", d[0].message);
    // Nor does `length` find a suggestion in the core.
    assert!(engine.compile_checked(r#"{"length": "abc"}"#).is_err());
}

#[test]
fn in_a_template_a_left_out_name_is_an_output_field() {
    let engine = Engine::builder()
        .with_families([])
        .with_templating(true)
        .build();
    assert_eq!(
        run(&engine, r#"{"length": {"var": "x"}}"#, r#"{"x": 5}"#),
        r#"{"length":5}"#
    );
    let strict = core_only();
    let logic = strict
        .compile_template(r#"{"length": {"var": "x"}}"#)
        .unwrap();
    assert_eq!(
        strict.session().eval_str(&logic, r#"{"x": 5}"#).unwrap(),
        r#"{"length":5}"#
    );
}

#[test]
fn every_compile_entry_point_follows_the_families() {
    let engine = core_only();
    let rule = r#"{"upper": "a"}"#;
    let unknown = |logic: datalogic_rs::Logic| {
        let err = engine.session().eval_str(&logic, "null").unwrap_err();
        assert_eq!(err.tag(), "InvalidOperator");
    };
    unknown(engine.compile(rule).unwrap());
    unknown(engine.compile_strict(rule).unwrap());
    assert!(engine.compile_checked(rule).is_err());
    let traced = engine.trace().eval_str(rule, "null");
    assert_eq!(traced.result.unwrap_err().tag(), "InvalidOperator");
}

#[test]
fn a_rebuilt_engine_keeps_its_families() {
    let engine = Engine::builder().with_families([Family::ExtString]).build();
    let rebuilt = engine.to_builder().build();
    assert_eq!(run(&rebuilt, r#"{"length": "ab"}"#, "null"), "2");
    assert_eq!(
        run(&rebuilt, r#"{"sort": [[2, 1]]}"#, "null"),
        "err InvalidOperator"
    );
    let widened = engine
        .to_builder()
        .with_families([Family::ExtString, Family::ExtArray])
        .build();
    assert_eq!(run(&widened, r#"{"sort": [[2, 1]]}"#, "null"), "[1,2]");
}

#[test]
fn naming_every_family_is_the_default() {
    let all = Engine::builder()
        .with_families(Family::ALL.iter().copied())
        .build();
    assert_eq!(all.operators().count(), Engine::new().operators().count());
}

#[test]
fn families_apply_when_a_rule_is_compiled() {
    // A rule compiled on an engine with every family keeps its operators
    // on any engine that evaluates it, as every 5.x rule does.
    let logic = Engine::new().compile(r#"{"length": "abc"}"#).unwrap();
    assert_eq!(core_only().session().eval_str(&logic, "null").unwrap(), "3");
}

#[test]
fn family_names_are_the_catalogue_names() {
    let names: std::collections::BTreeSet<&str> =
        Engine::new().operators().map(|op| op.family).collect();
    for family in Family::ALL {
        let name = family.name();
        if family.is_compiled() {
            assert!(names.contains(name), "{name}");
        }
    }
    for name in names {
        assert!(
            Family::ALL.iter().any(|f| f.name() == name),
            "{name} has no Family"
        );
    }
    assert!(!Family::Core.name().is_empty());
}
