//! `Logic::compiled_on`: whether a rule was compiled by a given engine. A
//! host that evaluates rules on an engine other than the one that compiled
//! them (Orion's boot and generation engines) can find those call sites
//! now; in 6.0 such an evaluation is an error.

use std::sync::Arc;

use datalogic_rs::{Engine, SharedSession};

const RULE: &str = r#"{"+": [1, {"var": "x"}]}"#;

#[test]
fn a_rule_was_compiled_on_its_own_engine() {
    let engine = Engine::new();
    let logic = engine.compile(RULE).unwrap();
    assert!(logic.compiled_on(&engine));
}

#[test]
fn not_on_another_engine_built_alike() {
    let a = Engine::new();
    let b = Engine::new();
    let logic = a.compile(RULE).unwrap();
    assert!(!logic.compiled_on(&b));
    // It still evaluates there, as in every 5.x release.
    assert_eq!(b.session().eval_str(&logic, r#"{"x": 2}"#).unwrap(), "3");
}

#[test]
fn not_on_a_rebuilt_engine() {
    let engine = Engine::new();
    let logic = engine.compile(RULE).unwrap();
    assert!(!logic.compiled_on(&engine.to_builder().build()));
}

#[test]
fn every_compile_entry_point_records_its_engine() {
    let engine = Engine::new();
    let other = Engine::new();
    let compiled = [
        engine.compile(RULE).unwrap(),
        engine.compile_strict(RULE).unwrap(),
        engine.compile_checked(RULE).unwrap(),
        #[cfg(feature = "templating")]
        engine.compile_template(RULE).unwrap(),
        #[cfg(feature = "serde_json")]
        engine.compile(&serde_json::json!({"var": "x"})).unwrap(),
    ];
    for logic in &compiled {
        assert!(logic.compiled_on(&engine));
        assert!(!logic.compiled_on(&other));
    }
}

#[test]
fn an_unfolded_compile_records_it_too() {
    let engine = Engine::builder().with_constant_folding(false).build();
    assert!(engine.compile(RULE).unwrap().compiled_on(&engine));
}

#[test]
fn a_clone_keeps_its_engine() {
    let engine = Engine::new();
    let logic = engine.compile(RULE).unwrap();
    let copy = logic.clone();
    assert!(copy.compiled_on(&engine));
}

#[test]
fn a_shared_session_sees_its_engine() {
    let engine = Arc::new(Engine::new());
    let logic = engine.compile(RULE).unwrap();
    let session = SharedSession::new(Arc::clone(&engine));
    assert!(logic.compiled_on(session.engine()));
}

#[test]
fn the_top_level_compile_belongs_to_no_user_engine() {
    let logic = datalogic_rs::compile(RULE).unwrap();
    assert!(!logic.compiled_on(&Engine::new()));
}
