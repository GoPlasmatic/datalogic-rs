//! Single-engine benchmark — datalogic-rs alone, using the fast arena path
//! (compile once, persistent input arena, eval-arena reset between iterations).
//! For cross-library comparison see `bin/compare.rs`.

use std::env;
use std::io::Write;
use std::time::Instant;

use bumpalo::Bump;
use datalogic_bench::{
    SuiteResult, load_index, load_suite, print_suite_line, print_summary, suites_root, write_report,
};
use datalogic_rs::{DataValue, Engine};

const ITERATIONS: u32 = 100_000;

fn benchmark_suite(engine: &Engine, suite_name: &str) -> Option<SuiteResult> {
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
    // &DataValue handles outlive every per-iteration eval-arena reset.
    let data_arena = Bump::new();
    let inputs: Vec<&DataValue<'_>> = cases
        .iter()
        .map(|c| {
            let av = DataValue::from_str(&c.data_json, &data_arena).expect("test data parses");
            &*data_arena.alloc(av)
        })
        .collect();

    // Eval arena: reset between iterations so the bump pointer stays at
    // chunk start. Sized for typical per-call growth.
    let mut arena = Bump::with_capacity(64 * 1024);

    // Warm-up.
    for (rule, data) in compiled.iter().zip(inputs.iter()) {
        let _ = engine.evaluate(rule, *data, &arena);
    }
    arena.reset();

    let start = Instant::now();
    for (rule, data) in compiled.iter().zip(inputs.iter()) {
        for _ in 0..ITERATIONS {
            let _ = engine.evaluate(rule, *data, &arena);
            arena.reset();
        }
    }
    std::hint::black_box((&inputs, &data_arena));
    let total_time = start.elapsed();
    let total_ops = ITERATIONS as u64 * compiled.len() as u64;

    Some(SuiteResult::new(
        suite_name.to_string(),
        compiled.len(),
        total_ops,
        total_time,
    ))
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
                Some(r) => {
                    println!(
                        "{:>4} tests | avg {:>8.2} ns/op | total {:>10.1?}",
                        r.test_count, r.avg_op_ns, r.total_time
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
            Some(r) => {
                println!("\n=== Benchmark Results ===");
                println!("Test cases:          {}", r.test_count);
                println!("Iterations per test: {ITERATIONS}");
                println!("Total operations:    {}", r.total_ops);
                println!("Total time:          {:.2?}", r.total_time);
                println!("Average op time:     {:.2} ns", r.avg_op_ns);
                print_suite_line(&r);
            }
            None => {
                eprintln!("No valid test cases found in {suite}");
                std::process::exit(1);
            }
        }
    }
}
