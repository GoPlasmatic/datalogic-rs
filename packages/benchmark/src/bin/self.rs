//! Single-engine benchmark — datalogic-rs alone, using the fast arena path
//! (compile once, persistent input arena, session-managed eval arena reset
//! between iterations). For cross-library comparison see `bin/compare.rs`.
//!
//! Why `Session` rather than a hand-rolled `Bump`:
//! - `Session` is the documented hot-loop API. Benchmarking it makes the
//!   reported numbers match what callers actually see in production.
//! - `evaluate_borrowed` returns a borrowed `&DataValue<'a>` tied to the session
//!   arena (no `OwnedDataValue::to_owned` deep-clone), preserving the
//!   "max perf, zero-copy result" measurement of the original benchmark.
//! - The session does not reset implicitly; the benchmark calls
//!   `session.reset()` after every `evaluate_borrowed` so peak memory stays
//!   bounded by the largest single evaluation (the bump pointer returns
//!   to chunk start without freeing chunks).
//!
//! Self-tuning arena sizing: the warm-up phase mirrors the timed-loop
//! shape exactly (same `evaluate_borrowed` + `reset` cadence), so the bump
//! grows to the suite's largest-single-eval high-water mark. We then
//! call `session.reset_with_capacity(session.allocated_bytes())` to drop
//! the warm-up's chunks and allocate one fresh chunk of exactly that
//! size — guaranteeing zero chunk-growth events during the timed inner
//! loop. The warmed size is printed alongside each suite's results.

use std::env;
use std::io::Write;
use std::time::Instant;

use bumpalo::Bump;
use datalogic_bench::{
    SuiteResult, load_index, load_suite, print_suite_line, print_summary, suites_root, write_report,
};
use datalogic_rs::{DataValue, Engine};

const ITERATIONS: u32 = 100_000;

fn benchmark_suite(engine: &Engine, suite_name: &str) -> Option<(SuiteResult, usize)> {
    let path = suites_root().join(suite_name);
    let cases = load_suite(&path)?;

    // Pre-compile every rule.
    let compiled: Vec<datalogic_rs::Logic> = cases
        .iter()
        .filter_map(|c| engine.compile(&c.rule_json).ok())
        .collect();

    if compiled.is_empty() {
        return None;
    }

    // Persistent arena holding parsed input data. Never reset, so the
    // &DataValue handles outlive every per-iteration session reset.
    let data_arena = Bump::new();
    let inputs: Vec<&DataValue<'_>> = cases
        .iter()
        .map(|c| {
            let av = DataValue::from_str(&c.data_json, &data_arena).expect("test data parses");
            &*data_arena.alloc(av)
        })
        .collect();

    // Session owns the eval arena. The session does not auto-reset; the
    // bench resets after every call (per-iteration) and pre-sizes the
    // arena from the warm-up's high-water mark before the timed loop.
    let mut session = engine.session();

    // Warm-up — same per-iteration `evaluate_borrowed` + `reset` shape as the
    // timed loop, so the bump grows to the suite's largest-single-eval
    // high-water mark. `Bump::reset` keeps the largest chunk; subsequent
    // iterations either fit (no growth) or trigger another doubling.
    for (rule, data) in compiled.iter().zip(inputs.iter()) {
        for _ in 0..ITERATIONS {
            let _ = session.evaluate_borrowed(rule, *data);
            session.reset();
        }
    }

    // Capture the warmed size and pre-size the arena for the timed run.
    // `Bump::reset` keeps the largest chunk allocated during warm-up,
    // but `reset_with_capacity` drops everything and creates one fresh
    // chunk of exactly that size — guaranteeing no chunk-growth events
    // during timing instead of relying on bumpalo's chunk-retention.
    let warmed_size = session.allocated_bytes();
    session.reset_with_capacity(warmed_size);

    let start = Instant::now();
    for (rule, data) in compiled.iter().zip(inputs.iter()) {
        for _ in 0..ITERATIONS {
            let _ = session.evaluate_borrowed(rule, *data);
            session.reset();
        }
    }
    std::hint::black_box((&inputs, &data_arena));
    let total_time = start.elapsed();
    let total_ops = ITERATIONS as u64 * compiled.len() as u64;

    Some((
        SuiteResult::new(
            suite_name.to_string(),
            compiled.len(),
            total_ops,
            total_time,
        ),
        warmed_size,
    ))
}

/// Format an arena byte count compactly for the per-suite line. Switches
/// to KB once the value would otherwise overflow the four-digit B column.
fn fmt_bytes(bytes: usize) -> String {
    if bytes < 10_000 {
        format!("{bytes} B")
    } else if bytes < 10_000_000 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let run_all = args.iter().any(|a| a == "--all");

    let engine = Engine::new();
    let version = env!("CARGO_PKG_VERSION");
    let label = format!("self-v{version}");

    if run_all {
        let suite_files = load_index();
        println!("Benchmarking all {} suites ({label})\n", suite_files.len());

        let mut results: Vec<SuiteResult> = Vec::new();
        for suite in &suite_files {
            print!("  {suite:<48}");
            std::io::stdout().flush().unwrap();
            match benchmark_suite(&engine, suite) {
                Some((r, warmed_size)) => {
                    println!(
                        "{:>4} tests | avg {:>8.2} ns/op | total {:>10.1?} | arena {:>9}",
                        r.test_count,
                        r.avg_op_ns,
                        r.total_time,
                        fmt_bytes(warmed_size)
                    );
                    results.push(r);
                }
                None => println!("  (skipped — no valid test cases)"),
            }
        }

        print_summary(&label, &results);
        let report_path = write_report(&label, ITERATIONS, &results);
        println!("\nReport saved to {}", report_path.display());
    } else {
        let suite = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "compatible.json".into());
        println!("Benchmark file: {suite} ({label})");
        match benchmark_suite(&engine, &suite) {
            Some((r, warmed_size)) => {
                println!("\n=== Benchmark Results ===");
                println!("Test cases:          {}", r.test_count);
                println!("Iterations per test: {ITERATIONS}");
                println!("Total operations:    {}", r.total_ops);
                println!("Total time:          {:.2?}", r.total_time);
                println!("Average op time:     {:.2} ns", r.avg_op_ns);
                println!(
                    "Arena bytes:         {} ({warmed_size} B)",
                    fmt_bytes(warmed_size)
                );
                print_suite_line(&r);
            }
            None => {
                eprintln!("No valid test cases found in {suite}");
                std::process::exit(1);
            }
        }
    }
}
