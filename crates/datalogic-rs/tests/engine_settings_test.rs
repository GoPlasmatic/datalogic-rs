//! Settings travel whole between `EngineBuilder` and `Engine`, `Engine`'s
//! `Debug` shows every one of them, and `try_build` refuses what the JSON
//! config refuses.

use datalogic_rs::{
    CustomOperator, DataValue, Engine, ErrorCode, EvaluationConfig, Family, Result,
    operator::EvalContext,
};

struct Echo;

impl CustomOperator for Echo {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(args.first().copied().unwrap_or(&DataValue::Null))
    }
}

/// A name checked by `try_add_operator` is checked again by `try_build`
/// on a builder made from the engine, so a reload that brings the
/// shadowing family back is refused there too. `to_builder` used to drop
/// the names.
// `upper` is a built-in only with the string family compiled in.
#[cfg(feature = "ext-string")]
#[test]
fn to_builder_keeps_the_checked_names() {
    let engine = Engine::builder()
        .with_families([Family::ExtArray])
        .try_add_operator("upper", Echo)
        .unwrap()
        .try_build()
        .unwrap();
    assert!(engine.to_builder().try_build().is_ok());
    let err = engine
        .to_builder()
        .with_families([Family::ExtString])
        .try_build()
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ConfigurationError);
}

/// `to_builder` carries every setting.
#[test]
fn to_builder_keeps_every_setting() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('~')
        .with_constant_folding(false)
        .with_families([Family::ExtString])
        .with_config(EvaluationConfig::default().with_max_recursion_depth(7))
        .add_operator("echo", Echo)
        .build();
    let rebuilt = engine.to_builder().build();
    assert_eq!(format!("{rebuilt:?}"), format!("{engine:?}"));
}

#[test]
fn debug_shows_every_setting() {
    let engine = Engine::builder()
        .with_constant_folding(false)
        .with_families([Family::ExtString])
        .build();
    let debug = format!("{engine:?}");
    for field in [
        "custom_operators",
        "templating",
        "template_key_escape",
        "constant_folding: false",
        "families",
        "config",
    ] {
        assert!(debug.contains(field), "{field} missing from {debug}");
    }
    if Family::ExtString.is_compiled() {
        assert!(debug.contains(Family::ExtString.name()), "{debug}");
    }
    if Family::ExtArray.is_compiled() {
        assert!(!debug.contains(Family::ExtArray.name()), "{debug}");
    }
}

/// `try_build` refuses a recursion depth of 0, which leaves no evaluation
/// able to start on an engine with custom operators; the JSON config
/// refuses it too.
#[test]
fn try_build_refuses_a_zero_recursion_depth() {
    let zero = EvaluationConfig::default().with_max_recursion_depth(0);
    let err = Engine::builder()
        .with_config(zero.clone())
        .try_build()
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ConfigurationError);
    assert!(
        Engine::builder()
            .with_config(EvaluationConfig::default().with_max_recursion_depth(1))
            .try_build()
            .is_ok()
    );
    // `build` cannot fail and keeps the value, as before.
    let engine = Engine::builder()
        .with_config(zero)
        .add_operator("echo", Echo)
        .build();
    assert_eq!(engine.config().max_recursion_depth, 0);
    #[cfg(feature = "serde_json")]
    {
        let err = EvaluationConfig::from_json_str(r#"{"max_recursion_depth": 0}"#).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConfigurationError);
    }
}
