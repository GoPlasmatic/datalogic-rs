//! `ErrorKind` variants that must exist in every build, whatever features
//! are enabled, so downstream code can match on them without mirroring
//! datalogic's feature set.

use datalogic_rs::{Error, ErrorKind};

/// `BudgetExceeded` is raised only with the `budget` feature, but the
/// variant is always present, so a host can match on it rather than on
/// `tag()`.
#[test]
fn budget_exceeded_exists_in_every_build() {
    let err = Error::from(ErrorKind::BudgetExceeded {
        budget: 10,
        spent: 12,
    });
    assert_eq!(err.tag(), "BudgetExceeded");
    assert!(matches!(
        err.kind,
        ErrorKind::BudgetExceeded {
            budget: 10,
            spent: 12
        }
    ));
    assert!(err.to_string().contains("10"), "{err}");
}

// ── ErrorCode ───────────────────────────────────────────────────────────

use datalogic_rs::ErrorCode;
use datavalue::OwnedDataValue;

/// One error of every kind, built through the public constructors.
fn one_of_each() -> Vec<Error> {
    vec![
        Error::invalid_operator("x"),
        Error::invalid_arguments("x"),
        Error::variable_not_found("x"),
        Error::invalid_context_level(3),
        Error::type_error("x"),
        Error::arithmetic_error("x"),
        Error::custom_message("x"),
        Error::parse_error("x"),
        Error::thrown(OwnedDataValue::Null),
        Error::format_error("x"),
        Error::index_out_of_bounds(4, 2),
        Error::configuration_error("x"),
        Error::from(ErrorKind::BudgetExceeded {
            budget: 1,
            spent: 2,
        }),
    ]
}

#[test]
fn every_kind_has_its_own_code() {
    let errors = one_of_each();
    let codes: Vec<ErrorCode> = errors.iter().map(Error::code).collect();
    assert_eq!(
        codes,
        ErrorCode::ALL,
        "one code per kind, in declaration order"
    );
    for (error, code) in errors.iter().zip(&codes) {
        assert_eq!(error.kind.code(), *code);
    }
}

#[test]
fn a_code_names_the_kind_as_tag_does() {
    for error in one_of_each() {
        assert_eq!(error.code().as_str(), error.tag());
        assert_eq!(error.code().to_string(), error.tag());
    }
}

#[test]
fn a_code_parses_back_from_its_name() {
    for code in ErrorCode::ALL {
        assert_eq!(code.as_str().parse::<ErrorCode>(), Ok(*code));
    }
    assert!("NotAKind".parse::<ErrorCode>().is_err());
    assert!("invalidarguments".parse::<ErrorCode>().is_err());
}

#[test]
fn codes_are_distinct_values() {
    let mut names: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), ErrorCode::ALL.len());
    let set: std::collections::HashSet<ErrorCode> = ErrorCode::ALL.iter().copied().collect();
    assert_eq!(set.len(), ErrorCode::ALL.len());
}

/// The code a host reads off an evaluation error, without matching on the
/// kind's payload.
#[test]
fn an_evaluation_error_carries_its_code() {
    let engine = datalogic_rs::Engine::new();
    let thrown = engine.eval_str(r#"{"+": ["x", 1]}"#, "null").unwrap_err();
    assert_eq!(thrown.code(), ErrorCode::Thrown);
    let unknown = engine.eval_str(r#"{"nope": []}"#, "null").unwrap_err();
    assert_eq!(unknown.code(), ErrorCode::InvalidOperator);
    let bad = engine.eval_str(r#"{"map": [1]}"#, "null").unwrap_err();
    assert_eq!(bad.code(), ErrorCode::InvalidArguments);
}
