//! Execution tracing — see every step the engine takes.
//!
//! `engine.trace().evaluate_str(...)` returns a `TracedRun` whose
//! `steps` field logs each evaluated node along with its result (or
//! error). Use it to debug rules that return something unexpected.
//!
//! Run:
//!
//!     cargo run --example tracing --features trace

use datalogic_rs::Engine;

fn main() {
    let engine = Engine::new();

    // ----- (1) success -----------------------------------------------
    let run = engine.trace().evaluate_str(
        r#"{"if": [{">": [{"var": "age"}, 18]}, "adult", "minor"]}"#,
        r#"{"age": 21}"#,
    );

    println!("[1] result: {}", run.result.as_ref().unwrap());
    println!("    steps:  {}", run.steps.len());
    for step in &run.steps {
        let r = step
            .result
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<error>".into());
        println!("    #{}  node {} -> {}", step.step_id, step.node_id, r);
    }

    // ----- (2) failure — error is structured ------------------------
    let run = engine
        .trace()
        .evaluate_str(r#"{"+": [{"var": "x"}, "not-a-number"]}"#, r#"{"x": 1}"#);

    let err = run.result.unwrap_err();
    println!("\n[2] failure");
    println!("    tag: {}", err.tag());
    println!("    operator: {:?}", err.operator());
    println!("    steps:    {} recorded before failure", run.steps.len());
}
