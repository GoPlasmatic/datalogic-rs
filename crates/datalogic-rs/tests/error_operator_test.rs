//! `Error::operator` names the innermost operator that failed, custom
//! operators included, and every evaluation path agrees on it.
//!
//! It used to be the root operator unless a deeper site named one, and the
//! traced path always overwrote it with the root operator. A failing
//! custom operator nested in `+` inside `if` was reported as `+` on the
//! plain path (the folded `if` was gone) and as `if` on the traced path.
#![cfg(feature = "trace")]

use datalogic_rs::{CustomOperator, DataValue, Engine, Error, operator::EvalContext};

struct Fail;

impl CustomOperator for Fail {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> datalogic_rs::Result<&'a DataValue<'a>> {
        Err(Error::custom_message("boom"))
    }
}

/// The operator each path reports for `rule` over `data`.
fn reported(engine: &Engine, rule: &str, data: &str) -> [Option<String>; 4] {
    let name = |e: Error| e.operator().map(str::to_owned);
    let compiled = engine.compile(rule).unwrap();
    let arena = datalogic_rs::bumpalo::Bump::new();
    let evaluate = engine.evaluate(&compiled, data, &arena).unwrap_err();
    let one_shot = engine.eval_str(rule, data).unwrap_err();
    let session = engine.session().eval(&compiled, data).unwrap_err();
    let traced = engine.trace().eval_str(rule, data).result.unwrap_err();
    [name(evaluate), name(one_shot), name(session), name(traced)]
}

#[test]
fn names_the_innermost_failing_operator_on_every_path() {
    let engine = Engine::builder().add_operator("fail", Fail).build();
    let data = r#"{"x": 1, "y": "a"}"#;
    for (rule, want) in [
        // A custom operator nested in built-ins, under a folded `if`.
        (
            r#"{"if": [true, {"+": [1, {"fail": [{"var": "x"}]}]}, 0]}"#,
            "fail",
        ),
        // A built-in failing under another built-in.
        (
            r#"{"if": [{"var": "x"}, {"+": [1, {"var": "y"}]}, 0]}"#,
            "+",
        ),
        // Misused arguments, named at compile time.
        (r#"{"if": [{"var": "x"}, {"and": {"var": "x"}}, 0]}"#, "and"),
        // The failing operator at the root.
        (r#"{"fail": []}"#, "fail"),
        // Inside an iteration body.
        (
            r#"{"map": [[1, 2], {"cat": ["n", {"fail": [{"var": ""}]}]}]}"#,
            "fail",
        ),
    ] {
        let want = Some(want.to_owned());
        assert_eq!(
            reported(&engine, rule, data),
            [want.clone(), want.clone(), want.clone(), want],
            "{rule}"
        );
    }
}

/// An operator name set where the error was raised is kept.
#[test]
fn keeps_a_name_set_by_the_raising_site() {
    struct Named;
    impl CustomOperator for Named {
        fn evaluate<'a>(
            &self,
            _args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            _arena: &'a datalogic_rs::bumpalo::Bump,
        ) -> datalogic_rs::Result<&'a DataValue<'a>> {
            Err(Error::custom_message("boom").with_operator("inner_step"))
        }
    }
    let engine = Engine::builder().add_operator("named", Named).build();
    let want = Some("inner_step".to_owned());
    assert_eq!(
        reported(&engine, r#"{"+": [1, {"named": []}]}"#, "{}"),
        [want.clone(), want.clone(), want.clone(), want]
    );
}
