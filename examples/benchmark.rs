use datalogic_rs::DataLogic;
use serde_json::Value;
use std::env;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

const ITERATIONS: u32 = 100_000;
const SUITES_DIR: &str = "tests/suites";

struct SuiteResult {
    name: String,
    test_count: usize,
    total_ops: u32,
    total_time: Duration,
    avg_op_time: Duration,
}

fn benchmark_suite(engine: &DataLogic, file_path: &str) -> Option<SuiteResult> {
    let response = fs::read_to_string(file_path).ok()?;
    let json_data: Vec<Value> = serde_json::from_str(&response).ok()?;

    let mut test_cases = Vec::new();
    for entry in json_data {
        if entry.is_string() {
            continue;
        }
        if let Value::Object(test_case) = entry {
            if let Some(logic) = test_case.get("rule") {
                let data = test_case.get("data").cloned().unwrap_or(Value::Null);
                let data_arc = Arc::new(data);
                if let Ok(compiled) = engine.compile(logic) {
                    test_cases.push((compiled, data_arc));
                }
            }
        }
    }

    if test_cases.is_empty() {
        return None;
    }

    // Warm-up
    for (compiled_logic, data) in &test_cases {
        let _ = engine.evaluate(compiled_logic, data.clone());
    }

    let start = Instant::now();
    for (compiled_logic, data) in &test_cases {
        for _ in 0..ITERATIONS {
            let _ = engine.evaluate(compiled_logic, data.clone());
        }
    }
    let total_time = start.elapsed();
    let total_ops = ITERATIONS * test_cases.len() as u32;
    let avg_op_time = total_time / total_ops;

    Some(SuiteResult {
        name: file_path.to_string(),
        test_count: test_cases.len(),
        total_ops,
        total_time,
        avg_op_time,
    })
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let run_all = args.iter().any(|a| a == "--all");

    let engine = DataLogic::new();
    let version = env!("CARGO_PKG_VERSION");

    if run_all {
        let index_path = format!("{SUITES_DIR}/index.json");
        let index_content = fs::read_to_string(&index_path).expect("Failed to read index.json");
        let suite_files: Vec<String> =
            serde_json::from_str(&index_content).expect("Failed to parse index.json");

        println!(
            "Benchmarking all {} suites (v{version})\n",
            suite_files.len()
        );

        let mut results: Vec<SuiteResult> = Vec::new();
        let mut grand_total_time = Duration::ZERO;
        let mut grand_total_ops: u64 = 0;

        for suite_file in &suite_files {
            let path = format!("{SUITES_DIR}/{suite_file}");
            print!("  {suite_file:<45}");
            std::io::stdout().flush().unwrap();

            match benchmark_suite(&engine, &path) {
                Some(result) => {
                    println!(
                        "{:>4} tests | avg {:>8.0?}/op | total {:>10.0?}",
                        result.test_count, result.avg_op_time, result.total_time
                    );
                    grand_total_time += result.total_time;
                    grand_total_ops += result.total_ops as u64;
                    results.push(result);
                }
                None => println!("  (skipped - no valid test cases)"),
            }
        }

        let grand_avg = if grand_total_ops > 0 {
            grand_total_time / grand_total_ops as u32
        } else {
            Duration::ZERO
        };

        println!("\n=== Summary (v{version}) ===");
        println!("Suites:              {}", results.len());
        println!("Total time:          {grand_total_time:.2?}");
        println!("Total operations:    {grand_total_ops}");
        println!("Average op time:     {grand_avg:.0?}");

        // Write report
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let report_path = format!("benchmarks/report-v{version}-{timestamp}.json");
        fs::create_dir_all("benchmarks").expect("Failed to create benchmarks directory");

        let suite_entries: Vec<Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "suite": r.name,
                    "test_count": r.test_count,
                    "total_ops": r.total_ops,
                    "total_time_ms": r.total_time.as_secs_f64() * 1000.0,
                    "avg_op_time_ns": r.avg_op_time.as_nanos(),
                })
            })
            .collect();

        let report = serde_json::json!({
            "version": version,
            "timestamp": timestamp,
            "iterations_per_test": ITERATIONS,
            "summary": {
                "suites": results.len(),
                "total_time_ms": grand_total_time.as_secs_f64() * 1000.0,
                "total_ops": grand_total_ops,
                "avg_op_time_ns": grand_avg.as_nanos(),
            },
            "suites": suite_entries,
        });

        let report_json =
            serde_json::to_string_pretty(&report).expect("Failed to serialize report");
        fs::write(&report_path, &report_json).expect("Failed to write report");
        println!("\nReport saved to {report_path}");
    } else {
        // Single suite mode
        let file_path = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| format!("{SUITES_DIR}/compatible.json"));

        println!("Benchmark file: {file_path} (v{version})");

        match benchmark_suite(&engine, &file_path) {
            Some(result) => {
                println!("\n=== Benchmark Results ===");
                println!("Test cases:          {}", result.test_count);
                println!("Iterations per test: {ITERATIONS}");
                println!("Total operations:    {}", result.total_ops);
                println!("Total time:          {:.2?}", result.total_time);
                println!("Average op time:     {:.0?}", result.avg_op_time);
            }
            None => {
                eprintln!("No valid test cases found in {file_path}");
                std::process::exit(1);
            }
        }
    }
}
