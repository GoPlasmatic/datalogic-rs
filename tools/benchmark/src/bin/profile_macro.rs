//! Profiling harness: hammer one macro suite in a hot loop so a sampling
//! profiler (samply / Instruments) sees only that suite's evaluation path.
//!
//! Usage: `profile_macro <suite-substring> [seconds]` — e.g.
//! `profile_macro checkout 10`. Not a benchmark; prints a rough ns/op so
//! runs are comparable, but the point is the profile.

use std::env;
use std::time::{Duration, Instant};

use bumpalo::Bump;
use datalogic_bench::macro_suites::macro_suites;
use datalogic_rs::{DataValue, Engine, Logic};

fn main() {
    let args: Vec<String> = env::args().collect();
    let filter = args.get(1).map(String::as_str).unwrap_or("checkout");
    let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let engine = Engine::new();
    let suites = macro_suites();
    let suite = suites
        .iter()
        .find(|s| s.name.contains(filter))
        .unwrap_or_else(|| panic!("no macro suite matching {filter:?}"));

    let compiled: Vec<Logic> = suite
        .cases
        .iter()
        .map(|c| engine.compile(c.rule_json.as_str()).expect("rule compiles"))
        .collect();

    let data_arena = Bump::new();
    let inputs: Vec<&DataValue<'_>> = suite
        .cases
        .iter()
        .map(|c| {
            let av = DataValue::from_str(&c.data_json, &data_arena).expect("data parses");
            &*data_arena.alloc(av)
        })
        .collect();
    let pairs: Vec<(&Logic, &DataValue)> = compiled.iter().zip(inputs.iter().copied()).collect();

    let mut session = engine.session();
    // Warm up.
    for &(rule, data) in &pairs {
        std::hint::black_box(session.eval_borrowed(rule, data).ok());
        session.reset();
    }

    println!(
        "profiling {} ({} cases) for ~{seconds}s",
        suite.name,
        pairs.len()
    );
    let deadline = Duration::from_secs(seconds);
    let start = Instant::now();
    let mut ops: u64 = 0;
    while start.elapsed() < deadline {
        for _ in 0..64 {
            for &(rule, data) in &pairs {
                std::hint::black_box(session.eval_borrowed(rule, data).ok());
                session.reset();
                ops += 1;
            }
        }
    }
    let elapsed = start.elapsed();
    println!(
        "{} ops in {elapsed:.2?} → {:.1} ns/op",
        ops,
        elapsed.as_nanos() as f64 / ops as f64
    );
}
