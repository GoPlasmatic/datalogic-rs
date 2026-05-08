//! First-read example — the three pillars of `datalogic-rs` in one file.
//!
//! Run:
//!
//!     cargo run --example getting_started --features preserve

use datalogic_rs::Engine;

fn main() {
    // ============================================================
    // 1. BUSINESS RULES — encode access control / validation as JSON.
    // ============================================================
    let engine = Engine::new();

    let allowed = engine
        .evaluate_str(
            r#"{"and": [
                {">=": [{"var": "age"}, 18]},
                {"==": [{"var": "status"}, "active"]}
            ]}"#,
            r#"{"age": 25, "status": "active"}"#,
        )
        .unwrap();
    println!("[1] business rule: allowed = {allowed}");

    // ============================================================
    // 2. JSON TEMPLATES — preserve_structure mode lets object keys
    //    flow through to the output; operator values become computed
    //    fields.
    //
    //    `preserve_structure(true)` is the v5 replacement for the old
    //    v4 `{"preserve": ...}` operator and the heuristic templating
    //    behaviour. It must be enabled on the builder; without it,
    //    multi-key objects in a rule are treated as a parse error
    //    ("Unknown Operator"). For a deeper walkthrough see the
    //    `structured_objects` example.
    // ============================================================
    let engine = Engine::builder().preserve_structure(true).build();

    let shaped = engine
        .evaluate_str(
            r#"{
                "greeting": {"cat": ["Hello, ", {"var": "name"}, "!"]},
                "isAdult":  {">=": [{"var": "age"}, 18]}
            }"#,
            r#"{"name": "Jane", "age": 25}"#,
        )
        .unwrap();
    println!("[2] template:      {shaped}");

    // ============================================================
    // 3. EXPRESSION EVALUATION — formulas over data, no `eval()`.
    // ============================================================
    let engine = Engine::new();

    // Simple arithmetic: subtotal + tax + shipping.
    let total = engine
        .evaluate_str(
            r#"{"+": [{"var": "subtotal"}, {"var": "tax"}, {"var": "shipping"}]}"#,
            r#"{"subtotal": 100, "tax": 8.5, "shipping": 5}"#,
        )
        .unwrap();
    println!("[3a] total:        {total}");

    // Reduction: sum of items[].price.
    let cart_total = engine
        .evaluate_str(
            r#"{"reduce": [
                {"var": "items"},
                {"+": [{"var": "accumulator"}, {"var": "current.price"}]},
                0
            ]}"#,
            r#"{"items": [{"price": 29.99}, {"price": 49.99}, {"price": 19.99}]}"#,
        )
        .unwrap();
    println!("[3b] cart_total:   {cart_total}");
}
