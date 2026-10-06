//! A compiled rule finds its custom operators by slot on the engine that
//! compiled it, and by name on any other. These pin the behaviour the slot
//! cache must keep: a rule evaluated on another engine runs that engine's
//! operator of the same name, whatever order the operators were registered
//! in, and an operator unknown when the rule was compiled is still found.

use datalogic_rs::operator::EvalContext;
use datalogic_rs::{ArenaExt, CustomOperator, DataValue, Engine, ErrorKind, Result};

/// Returns its tag, so a test can see which operator ran.
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
}

fn run(engine: &Engine, logic: &datalogic_rs::Logic) -> String {
    match engine.session().eval_str(logic, "null") {
        Ok(v) => v,
        Err(e) => format!("err {:?}", e.kind),
    }
}

#[test]
fn each_operator_runs_on_its_own_engine() {
    let engine = Engine::builder()
        .add_operator("a", Tag(1))
        .add_operator("b", Tag(2))
        .build();
    let logic = engine
        .compile(r#"[{"a": []}, {"b": []}, {"a": []}]"#)
        .unwrap();
    assert_eq!(run(&engine, &logic), "[1,2,1]");
}

#[test]
fn another_engine_runs_its_own_operator_of_that_name() {
    let compiling = Engine::builder()
        .add_operator("a", Tag(1))
        .add_operator("b", Tag(2))
        .build();
    // Registered in the other order, with other results.
    let serving = Engine::builder()
        .add_operator("b", Tag(20))
        .add_operator("a", Tag(10))
        .build();
    let logic = compiling.compile(r#"[{"a": []}, {"b": []}]"#).unwrap();
    assert_eq!(run(&serving, &logic), "[10,20]");
    assert_eq!(run(&compiling, &logic), "[1,2]");
}

#[test]
fn an_operator_unknown_at_compile_time_is_found_by_name() {
    let logic = Engine::new().compile(r#"{"late": []}"#).unwrap();
    let serving = Engine::builder().add_operator("late", Tag(7)).build();
    assert_eq!(run(&serving, &logic), "7");
}

#[test]
fn an_engine_without_the_operator_names_it_in_the_error() {
    let compiling = Engine::builder().add_operator("a", Tag(1)).build();
    let logic = compiling.compile(r#"{"a": []}"#).unwrap();
    let err = Engine::new()
        .session()
        .eval_str(&logic, "null")
        .unwrap_err();
    assert!(
        matches!(&err.kind, ErrorKind::InvalidOperator(name) if name == "a"),
        "{err:?}"
    );
}

#[test]
fn many_operators_each_resolve() {
    let mut builder = Engine::builder();
    for i in 0..64 {
        builder = builder.add_operator(format!("op{i}"), Tag(i));
    }
    let engine = builder.build();
    let rule = format!(
        "[{}]",
        (0..64)
            .rev()
            .map(|i| format!(r#"{{"op{i}": []}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    let logic = engine.compile(rule.as_str()).unwrap();
    let expected = format!(
        "[{}]",
        (0..64)
            .rev()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(run(&engine, &logic), expected);
}

#[test]
fn a_replaced_registration_runs_the_last_one() {
    let engine = Engine::builder()
        .add_operator("a", Tag(1))
        .add_operator("a", Tag(2))
        .build();
    let logic = engine.compile(r#"{"a": []}"#).unwrap();
    assert_eq!(run(&engine, &logic), "2");
    assert_eq!(engine.custom_operator_names().count(), 1);
}

#[test]
fn engines_built_alike_are_still_different_engines() {
    // Same registrations, separately built: the rule still runs (by name)
    // and gets the second engine's operator.
    let first = Engine::builder().add_operator("a", Tag(1)).build();
    let second = Engine::builder().add_operator("a", Tag(2)).build();
    let logic = first.compile(r#"{"a": []}"#).unwrap();
    assert_eq!(run(&second, &logic), "2");
}

/// Declares an argument count and returns its last argument, so a call
/// that slipped past the count check would index out of bounds.
struct Second(datalogic_rs::CustomOperatorInfo);

impl CustomOperator for Second {
    fn info(&self) -> datalogic_rs::CustomOperatorInfo {
        self.0
    }

    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(args[1])
    }
}

#[test]
fn another_engine_checks_its_own_operators_argument_count() {
    use datalogic_rs::CustomOperatorInfo;
    // Compiled where "op" takes any count; served where it needs exactly 2.
    let compiling = Engine::builder()
        .add_operator("op", Second(CustomOperatorInfo::opaque()))
        .build();
    let serving = Engine::builder()
        .add_operator(
            "op",
            Second(CustomOperatorInfo::opaque().with_args(2, Some(2))),
        )
        .build();
    let logic = compiling.compile(r#"{"op": [1]}"#).unwrap();
    assert!(run(&serving, &logic).starts_with("err InvalidArguments"));

    // The other way round: compiled where "op" takes 1, served where it
    // takes any count, so the 2-argument call runs.
    let compiling = Engine::builder()
        .add_operator(
            "op",
            Second(CustomOperatorInfo::opaque().with_args(1, Some(1))),
        )
        .build();
    let serving = Engine::builder()
        .add_operator("op", Second(CustomOperatorInfo::opaque()))
        .build();
    let logic = compiling.compile(r#"{"op": [1, 2]}"#).unwrap();
    assert_eq!(run(&serving, &logic), "2");
}
