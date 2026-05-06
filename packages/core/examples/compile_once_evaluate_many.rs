//! Compile once, evaluate many — three throughput-friendly entry points.
//!
//! Run:
//!
//!     cargo run --example compile_once_evaluate_many

use bumpalo::Bump;
use datalogic_rs::Engine;
use datalogic_rs::datavalue::OwnedDataValue;

fn main() {
    let engine = Engine::new();
    let compiled = engine
        .compile(r#"{"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]}"#)
        .unwrap();

    // ----- (1) one-shot: parses, evaluates, returns a JSON string ----
    let r = engine
        .evaluate_str(
            r#"{"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]}"#,
            r#"{"score": 75}"#,
        )
        .unwrap();
    println!("[1] evaluate_str:    score=75 -> {r}");

    // ----- (2) Session: reuses an arena across many evaluations ------
    let mut session = engine.session();
    for score in [25, 60, 75] {
        let payload = format!(r#"{{"score": {score}}}"#);
        let r = session.evaluate_str(&compiled, &payload).unwrap();
        println!("[2] session.evaluate_str: score={score:>3} -> {r}");
    }

    // ----- (3) Engine::evaluate: caller-managed Bump, zero-copy result
    let arena = Bump::new();
    let r = engine
        .evaluate(&compiled, r#"{"score": 91}"#, &arena)
        .unwrap();
    println!(
        "[3] engine.evaluate: score=91 -> {}",
        r.as_str().unwrap_or("?")
    );

    // `Engine::evaluate` accepts any `EvalInput`: `&str` (above),
    // `&OwnedDataValue` (parse once, evaluate many), `DataValue<'a>`
    // already in the arena, or `&serde_json::Value` (with `compat`).
    let owned = OwnedDataValue::from_json(r#"{"score": 35}"#).unwrap();
    let r = engine.evaluate(&compiled, &owned, &arena).unwrap();
    println!(
        "[3] engine.evaluate (&OwnedDataValue) -> {}",
        r.as_str().unwrap_or("?")
    );
}
