//! Sharing a compiled rule across threads.
//!
//! `Engine` and `Logic` are both `Send + Sync`. Wrap them in `Arc` and
//! every worker can evaluate concurrently without locks. Each worker
//! gets its own `Session` so the per-thread arena is isolated.
//!
//! Run:
//!
//!     cargo run --example thread_safety

use datalogic_rs::Engine;
use std::sync::Arc;
use std::thread;

fn main() {
    let engine = Arc::new(Engine::new());
    let compiled = Arc::new(engine.compile(r#"{">": [{"var": "score"}, 50]}"#).unwrap());

    let workers: Vec<_> = (1..=4)
        .map(|i| {
            let engine = Arc::clone(&engine);
            let compiled = Arc::clone(&compiled);
            thread::spawn(move || {
                let mut session = engine.session();
                let payload = format!(r#"{{"score": {}}}"#, i * 25);
                let result = session.eval_str(&compiled, &payload).unwrap();
                (i, i * 25, result)
            })
        })
        .collect();

    for w in workers {
        let (id, score, result) = w.join().unwrap();
        println!("worker {id}: score={score:>3} -> {result}");
    }
}
