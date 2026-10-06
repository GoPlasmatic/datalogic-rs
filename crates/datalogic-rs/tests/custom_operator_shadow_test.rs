//! `EngineBuilder::try_add_operator`: registering a custom operator under a
//! name a built-in already answers to is an error, instead of an operator
//! that silently never runs.

use datalogic_rs::{CustomOperator, DataValue, Engine, ErrorKind, Result, operator::EvalContext};

struct Answer;

impl CustomOperator for Answer {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(arena.alloc(DataValue::from_f64(42.0)))
    }
}

#[test]
fn a_free_name_registers() {
    let engine = Engine::builder()
        .try_add_operator("answer", Answer)
        .unwrap()
        .build();
    assert!(engine.has_custom_operator("answer"));
    assert_eq!(engine.eval_str(r#"{"answer": []}"#, "null").unwrap(), "42");
}

#[test]
fn every_builtin_name_is_refused() {
    let names: Vec<&'static str> = Engine::new().builtin_operator_names().collect();
    assert!(names.contains(&"var"), "aliases are included");
    assert!(names.contains(&"?:"), "aliases are included");
    for name in names {
        let err = Engine::builder()
            .try_add_operator(name, Answer)
            .err()
            .unwrap_or_else(|| panic!("`{name}` was accepted"));
        assert!(
            matches!(err.kind, ErrorKind::ConfigurationError(_)),
            "{name}: {err:?}"
        );
        assert!(err.to_string().contains(name), "{name}: {err}");
    }
}

/// The refusal names what wins, so a host can report it.
#[test]
fn message_names_the_builtin() {
    let err = Engine::builder()
        .try_add_operator("var", Answer)
        .err()
        .unwrap();
    assert_eq!(err.tag(), "ConfigurationError");
    assert!(err.to_string().contains("`var`"), "{err}");
    assert!(err.to_string().contains("`val`"), "{err}");
}

/// A name is only shadowed by an operator compiled into this build. Without
/// the `datetime` family, `now` is free.
#[cfg(not(feature = "datetime"))]
#[test]
fn a_family_not_compiled_in_does_not_shadow() {
    let engine = Engine::builder()
        .try_add_operator("now", Answer)
        .unwrap()
        .build();
    assert_eq!(engine.eval_str(r#"{"now": []}"#, "null").unwrap(), "42");
}

/// `add_operator` keeps its 5.x behaviour: the registration is accepted
/// and the built-in wins.
#[test]
fn add_operator_is_unchanged() {
    let engine = Engine::builder().add_operator("if", Answer).build();
    assert_eq!(
        engine.eval_str(r#"{"if": [true, 1, 2]}"#, "null").unwrap(),
        "1"
    );
}

// `upper` is a built-in only with the string family compiled in.
#[cfg(feature = "ext-string")]
#[test]
fn try_build_checks_a_name_against_families_set_after_it() {
    use datalogic_rs::Family;
    // Free when registered, taken once the string family is back.
    let builder = Engine::builder()
        .with_families([Family::ExtArray])
        .try_add_operator("upper", Answer)
        .unwrap()
        .with_families([Family::ExtString]);
    assert!(builder.check_operator_names().is_err());
    let err = builder.try_build().err().unwrap();
    assert!(
        matches!(err.kind, ErrorKind::ConfigurationError(_)),
        "{err:?}"
    );
    // Still free with the final families: builds.
    let engine = Engine::builder()
        .try_add_operator("answer", Answer)
        .unwrap()
        .with_families([Family::ExtArray])
        .try_build()
        .unwrap();
    assert_eq!(engine.eval_str(r#"{"answer": []}"#, "null").unwrap(), "42");
}

#[cfg(feature = "templating")]
#[test]
fn a_name_beginning_with_the_template_key_escape_is_refused() {
    let builder = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$');
    assert!(builder.check_operator_name("$up").is_err());
    assert!(builder.check_operator_name("up$").is_ok());
    // Set after the operator: try_build catches it.
    let builder = Engine::builder()
        .with_templating(true)
        .try_add_operator("$up", Answer)
        .unwrap()
        .with_template_key_escape('$');
    assert!(builder.try_build().is_err());
}
