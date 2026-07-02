//! Regression tests for context-frame hygiene on error paths.

#![cfg(all(feature = "serde_json", feature = "error-handling"))]

use datalogic_rs::Engine;
use serde_json::json;

/// A throwing body on the `map` "bridge" path (scalar input) must still pop
/// its per-iteration context frame. If the frame leaks, a `try` that catches
/// the error keeps evaluating with a corrupted stack, so a later `var`
/// resolves against the stale scalar (`5`) instead of the root data.
///
/// Before the fix `map_bridge_single` used `run_iter_body(...)?` directly,
/// skipping the `ctx.pop()` on the error path.
#[test]
fn test_map_bridge_frame_not_leaked_under_try() {
    let engine = Engine::new();
    let logic = json!({
        "cat": [
            {"try": [{"map": [5, {"throw": "e"}]}, "caught"]},
            "-",
            {"var": "name"}
        ]
    });
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({"name": "root"}))
        .unwrap();
    // The trailing `{"var": "name"}` must still see the root object. A leaked
    // frame would resolve it against the scalar `5` (→ null) instead.
    assert_eq!(result, json!("caught-root"));

    // The catch arm still receives the thrown error object as its context:
    // `{"throw": "boom"}` yields `{"type": "boom"}`, so `{"var": "type"}`
    // resolves to "boom" — confirming the fix didn't disturb the error frame.
    let logic = json!({
        "try": [{"map": [5, {"throw": "boom"}]}, {"var": "type"}]
    });
    let result = engine
        .eval_into::<serde_json::Value, _, _>(&logic, &json!({}))
        .unwrap();
    assert_eq!(result, json!("boom"));
}
