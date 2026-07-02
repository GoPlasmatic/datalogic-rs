//! Regression test for the compile-time nesting depth guard.
//!
//! The JSON string parser caps nesting depth, but a rule built
//! programmatically and handed to `compile` via `IntoLogic`
//! (`&serde_json::Value`) skips the parser. Without a guard in the compile
//! walker, a deeply-nested programmatic rule would overflow the stack in
//! `compile_node` (and later in dispatch / `Drop`).

#![cfg(feature = "serde_json")]

use datalogic_rs::Engine;
use serde_json::json;

#[test]
fn test_deeply_nested_rule_errors_instead_of_overflowing() {
    let engine = Engine::new();

    // 300 levels exceeds MAX_COMPILE_DEPTH (256), so compilation must return
    // an error rather than overflow the stack.
    let mut deep = json!(true);
    for _ in 0..300 {
        deep = json!({ "!": [deep] });
    }
    assert!(
        engine.compile(&deep).is_err(),
        "expected deep nesting to be rejected at compile time"
    );

    // A modestly-nested rule (well under the cap) still compiles fine.
    let mut shallow = json!(true);
    for _ in 0..10 {
        shallow = json!({ "!": [shallow] });
    }
    assert!(
        engine.compile(&shallow).is_ok(),
        "a shallow rule must still compile"
    );
}
