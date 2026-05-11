//! Demonstrates the input shapes accepted by `Engine::evaluate` /
//! `Session::evaluate*` (the [`EvalInput`] trait) and the per-call cost
//! of each.
//!
//! The same compiled rule can be evaluated against:
//!
//! - **`&str`** — JSON parsed on every call.
//! - **`&serde_json::Value`** — deep-converted into the arena on every
//!   call; cheaper than re-serializing to a string and re-parsing, but
//!   still walks the whole input tree.
//! - **`&OwnedDataValue`** — deep-cloned into the arena on every call.
//! - **`DataValue<'a>` (by value)** — single arena alloc for the top
//!   node; the interior string/array slices already live in the arena.
//! - **`&'a DataValue<'a>` (by reference)** — pass-through; the
//!   evaluator borrows directly into the caller's arena. This is the
//!   genuinely zero-copy path: no allocation, no walk, no clone.
//!
//! Run with: `cargo run --example zero_copy_input --features compat`

use bumpalo::Bump;
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{DataValue, Engine};
use serde_json::json;

fn main() {
    let engine = Engine::new();
    let compiled = engine
        .compile(r#"{"==": [{"var": "status"}, "active"]}"#)
        .unwrap();

    // Shape 1: &str (JSON-parsed each call)
    let arena = Bump::new();
    let result = engine
        .evaluate(&compiled, r#"{"status": "active"}"#, &arena)
        .unwrap();
    println!("&str           -> {:?}", result.as_bool());

    // Shape 2: &serde_json::Value (deep-converted each call)
    let arena = Bump::new();
    let input_json = json!({"status": "active"});
    let result = engine.evaluate(&compiled, &input_json, &arena).unwrap();
    println!("&serde Value   -> {:?}", result.as_bool());

    // Shape 3: &OwnedDataValue (deep-cloned each call)
    let arena = Bump::new();
    let owned = OwnedDataValue::from_json(r#"{"status": "active"}"#).unwrap();
    let result = engine.evaluate(&compiled, &owned, &arena).unwrap();
    println!("&OwnedDataValue-> {:?}", result.as_bool());

    // Shape 4 / 5: DataValue<'a> and &DataValue<'a> — both arena-resident.
    // Build the input once, evaluate many times with no further allocation
    // for the input tree. The genuine zero-copy path.
    let arena = Bump::new();
    let input: &DataValue<'_> =
        arena.alloc(DataValue::from_str(r#"{"status": "active"}"#, &arena).unwrap());

    // 4: by value (one arena alloc for the top-level node).
    let result = engine.evaluate(&compiled, *input, &arena).unwrap();
    println!("DataValue      -> {:?}", result.as_bool());

    // 5: by reference (zero allocs — pure pass-through).
    let arena_before = arena.allocated_bytes();
    let result = engine.evaluate(&compiled, input, &arena).unwrap();
    let arena_after = arena.allocated_bytes();
    println!(
        "&DataValue     -> {:?}  (input bytes added: {})",
        result.as_bool(),
        arena_after - arena_before,
    );
    // The "input bytes added" line should report 0 — the &DataValue path
    // doesn't allocate for the input. Any growth on this call is from the
    // evaluator's own working set, not from re-materializing the input.
}
