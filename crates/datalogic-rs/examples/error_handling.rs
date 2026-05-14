//! Structured error handling.
//!
//! `Error` carries a `kind`, the offending operator, and a node-id
//! breadcrumb so failures point at where they happened. `Error::wrap`
//! folds any `std::error::Error` into the same shape.
//!
//! Run:
//!
//!     cargo run --example error_handling --features error-handling

use datalogic_rs::{Engine, Error};

fn main() {
    let engine = Engine::new();

    // ----- (1) Structured failure -----------------------------------
    // Adding a string to a number raises a Thrown { type: "NaN" }.
    let err = engine
        .eval_str(r#"{"+": ["text", 1]}"#, r#"{}"#)
        .unwrap_err();
    println!("[1] failed evaluation");
    println!("    tag: {}", err.tag());
    println!("    operator: {:?}", err.operator());
    println!("    node_ids: {:?}", err.node_ids());
    println!("    display:  {err}");

    // ----- (2) Throw + read the payload (feature = error-handling) --
    #[cfg(feature = "error-handling")]
    {
        // `throw` wraps a string as `{type: <string>}`. To carry richer
        // structured data, look it up from the input via `val`.
        let err = engine
            .eval_str(
                r#"{"throw": {"val": "err"}}"#,
                r#"{"err": {"type": "NOT_FOUND", "user_id": 42}}"#,
            )
            .unwrap_err();
        println!("\n[2] thrown payload");
        println!("    {:?}", err.thrown_value().unwrap());
    }

    // ----- (3) Recover with `try` (feature = error-handling) --------
    #[cfg(feature = "error-handling")]
    {
        // `try` returns the first arg that succeeds; here a divide by
        // zero is coalesced to 0.
        let r = engine
            .eval_str(
                r#"{"try": [{"/": [{"var": "n"}, {"var": "d"}]}, 0]}"#,
                r#"{"n": 10, "d": 0}"#,
            )
            .unwrap();
        println!("\n[3] try recovers from divide-by-zero -> {r}");
    }

    // ----- (4) Wrap a foreign error with Error::wrap ----------------
    let err: Error = "abc".parse::<i32>().map_err(Error::wrap).unwrap_err();
    println!("\n[4] wrapped foreign error");
    println!("    tag: {}", err.tag());
    println!("    display:  {err}");
}
