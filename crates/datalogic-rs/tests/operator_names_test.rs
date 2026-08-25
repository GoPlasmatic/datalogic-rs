//! Integration tests for `Engine::builtin_operator_names` (issue #65).
//!
//! Deliberately NOT gated on `serde_json`: the per-family assertions below
//! run in both directions (`cfg(feature)` and `cfg(not(feature))`), and
//! the `not` legs only execute under `--no-default-features`, where a
//! `serde_json` gate would skip the whole file.

use bumpalo::Bump;
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};

fn names() -> Vec<&'static str> {
    Engine::new().builtin_operator_names().collect()
}

#[test]
fn baseline_names_are_always_present() {
    let names = names();
    for expected in [
        "val",
        "var",
        "==",
        "!==",
        "!",
        "!!",
        "and",
        "or",
        "if",
        "?:",
        "+",
        "%",
        "max",
        "cat",
        "substr",
        "in",
        "merge",
        "filter",
        "map",
        "reduce",
        "all",
        "some",
        "none",
        "missing",
        "missing_some",
    ] {
        assert!(names.contains(&expected), "baseline {expected:?} missing");
    }
}

#[test]
fn canonical_name_precedes_its_alias() {
    let names = names();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(pos("val") < pos("var"));
    assert!(pos("if") < pos("?:"));
}

#[test]
fn iterator_does_not_borrow_the_engine() {
    // `+ use<>` on the return type: the iterator must outlive the engine
    // it was obtained from, since the vocabulary is a build-time fact.
    let iter = {
        let engine = Engine::new();
        engine.builtin_operator_names()
    };
    assert!(iter.count() > 0);
}

#[test]
fn builtin_and_custom_sets_are_disjoint() {
    struct Double;
    impl CustomOperator for Double {
        fn evaluate<'a>(
            &self,
            args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            arena: &'a Bump,
        ) -> Result<&'a DataValue<'a>> {
            let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
        }
    }

    let engine = Engine::builder().add_operator("double", Double).build();
    assert!(!engine.builtin_operator_names().any(|n| n == "double"));
    assert!(engine.custom_operator_names().any(|n| n == "double"));
    // Registration does not change the built-in vocabulary.
    assert_eq!(
        engine.builtin_operator_names().count(),
        Engine::new().builtin_operator_names().count()
    );
}

/// The issue's motivating case: under templating mode an unknown key is
/// not an error, the object echoes back as a literal. Every name the
/// iterator reports must therefore be *live* — evaluating `{name: [1]}`
/// must never return the echoed object. An evaluation error is fine (it
/// proves the operator ran); only a silent echo is a drift bug.
#[cfg(feature = "templating")]
#[test]
fn every_reported_name_is_live_under_templating() {
    let engine = Engine::builder().with_templating(true).build();
    for name in engine.builtin_operator_names() {
        let rule = format!("{{{name:?}: [1]}}");
        if let Ok(OwnedDataValue::Object(fields)) = engine.eval(rule.as_str(), "{}") {
            assert!(
                !(fields.len() == 1 && fields[0].0 == name),
                "{name:?} is reported as built-in but echoed back as a literal"
            );
        }
    }
    // Control: a name that is not in the vocabulary does echo.
    let echoed = engine.eval(r#"{"lenght": [1]}"#, "{}").unwrap();
    assert!(matches!(echoed, OwnedDataValue::Object(ref f) if f.len() == 1 && f[0].0 == "lenght"));
}

// ---------------------------------------------------------------------
// Per-family presence, asserted in both directions so a family can be
// neither leaked into a build that lacks it nor dropped from one that
// has it. Representative names per family.
// ---------------------------------------------------------------------

macro_rules! family_tests {
    ($feature:literal, $present:ident, $absent:ident, [$($name:literal),+ $(,)?]) => {
        #[cfg(feature = $feature)]
        #[test]
        fn $present() {
            let names = names();
            $(assert!(names.contains(&$name), concat!($feature, " enabled but ", $name, " missing"));)+
        }

        #[cfg(not(feature = $feature))]
        #[test]
        fn $absent() {
            let names = names();
            $(assert!(!names.contains(&$name), concat!($feature, " disabled but ", $name, " reported"));)+
        }
    };
}

family_tests!(
    "datetime",
    datetime_names_present_when_enabled,
    datetime_names_absent_when_disabled,
    [
        "datetime",
        "timestamp",
        "parse_date",
        "format_date",
        "date_diff",
        "now"
    ]
);
family_tests!(
    "ext-string",
    ext_string_names_present_when_enabled,
    ext_string_names_absent_when_disabled,
    [
        "length",
        "starts_with",
        "ends_with",
        "upper",
        "lower",
        "trim",
        "split"
    ]
);
family_tests!(
    "ext-array",
    ext_array_names_present_when_enabled,
    ext_array_names_absent_when_disabled,
    ["sort", "slice", "group_by", "distinct"]
);
family_tests!(
    "ext-object",
    ext_object_names_present_when_enabled,
    ext_object_names_absent_when_disabled,
    ["keys", "values", "entries"]
);
family_tests!(
    "ext-control",
    ext_control_names_present_when_enabled,
    ext_control_names_absent_when_disabled,
    ["exists", "??", "switch", "match", "type"]
);
family_tests!(
    "error-handling",
    error_handling_names_present_when_enabled,
    error_handling_names_absent_when_disabled,
    ["try", "throw"]
);
family_tests!(
    "ext-math",
    ext_math_names_present_when_enabled,
    ext_math_names_absent_when_disabled,
    ["abs", "ceil", "floor"]
);
family_tests!(
    "flagd",
    flagd_names_present_when_enabled,
    flagd_names_absent_when_disabled,
    ["fractional", "sem_ver"]
);
