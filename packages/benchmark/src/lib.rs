//! Shared benchmark harness for `datalogic-bench`.
//!
//! Both `bin/self.rs` (datalogic-rs alone, fast arena path) and
//! `bin/compare.rs` (cross-library, string-in/string-out apples-to-apples
//! interface) reuse the suite loader, summary printer, and JSON reporter
//! defined here. Engine-specific timing loops live in their respective
//! binaries because the inner loop differs (arena reuse vs string round-trip).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

/// Resolve a suite path relative to the workspace root, so the benchmark
/// works regardless of the caller's cwd.
pub fn suites_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("suites")
}

/// Where reports land. Gitignored.
pub fn output_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("output")
}

/// One (rule, data) pair lifted from a suite JSON file. Strings stay as
/// strings so subjects parse them with their own machinery.
pub struct SuiteCase {
    pub rule_json: String,
    pub data_json: String,
}

/// Load a suite file into reusable (rule, data) string pairs. Skips
/// section-header strings and entries without a `rule` field.
pub fn load_suite(file_path: &Path) -> Option<Vec<SuiteCase>> {
    let raw = fs::read_to_string(file_path).ok()?;
    let entries: Vec<Value> = serde_json::from_str(&raw).ok()?;

    let mut cases = Vec::new();
    for entry in entries {
        if entry.is_string() {
            continue;
        }
        let Value::Object(test_case) = entry else {
            continue;
        };
        let Some(rule) = test_case.get("rule") else {
            continue;
        };
        let data = test_case.get("data").cloned().unwrap_or(Value::Null);

        let Ok(rule_json) = serde_json::to_string(rule) else {
            continue;
        };
        let Ok(data_json) = serde_json::to_string(&data) else {
            continue;
        };
        cases.push(SuiteCase {
            rule_json,
            data_json,
        });
    }

    if cases.is_empty() { None } else { Some(cases) }
}

/// Read `suites/index.json` (the suite-of-suites index) and return its list.
pub fn load_index() -> Vec<String> {
    let index_path = suites_root().join("index.json");
    let raw = fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", index_path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", index_path.display()))
}

/// Aggregate per-suite numbers reported by both binaries.
pub struct SuiteResult {
    pub name: String,
    pub test_count: usize,
    pub total_ops: u64,
    pub total_time: Duration,
    /// Per-op average in nanoseconds. Stored as f64 (not Duration) because
    /// Duration's integer-ns granularity truncates the fraction at
    /// sub-nanosecond resolution — the exact range that distinguishes
    /// benchmark runs.
    pub avg_op_ns: f64,
}

impl SuiteResult {
    pub fn new(name: String, test_count: usize, total_ops: u64, total_time: Duration) -> Self {
        let avg_op_ns = if total_ops == 0 {
            0.0
        } else {
            total_time.as_nanos() as f64 / total_ops as f64
        };
        Self {
            name,
            test_count,
            total_ops,
            total_time,
            avg_op_ns,
        }
    }
}

/// Print a one-line per-suite summary.
pub fn print_suite_line(result: &SuiteResult) {
    println!(
        "{:<48} {:>4} tests | avg {:>8.2} ns/op | total {:>10.1?}",
        result.name, result.test_count, result.avg_op_ns, result.total_time
    );
}

/// Print the grand summary header for an `--all` run.
pub fn print_summary(label: &str, results: &[SuiteResult]) {
    let total_time: Duration = results.iter().map(|r| r.total_time).sum();
    let total_ops: u64 = results.iter().map(|r| r.total_ops).sum();
    let avg = if total_ops == 0 {
        0.0
    } else {
        total_time.as_nanos() as f64 / total_ops as f64
    };
    println!("\n=== Summary ({label}) ===");
    println!("Suites:              {}", results.len());
    println!("Total time:          {total_time:.2?}");
    println!("Total operations:    {total_ops}");
    println!("Average op time:     {avg:.2} ns");
}

/// Write a JSON report into `packages/benchmark/output/`.
pub fn write_report(label: &str, iterations: u32, results: &[SuiteResult]) -> PathBuf {
    let out_dir = output_root();
    fs::create_dir_all(&out_dir).expect("create output dir");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock went backwards")
        .as_secs();

    let total_time: Duration = results.iter().map(|r| r.total_time).sum();
    let total_ops: u64 = results.iter().map(|r| r.total_ops).sum();
    let avg = if total_ops == 0 {
        0.0
    } else {
        total_time.as_nanos() as f64 / total_ops as f64
    };

    let suite_entries: Vec<Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "suite": r.name,
                "test_count": r.test_count,
                "total_ops": r.total_ops,
                "total_time_ms": r.total_time.as_secs_f64() * 1000.0,
                "avg_op_time_ns": r.avg_op_ns,
            })
        })
        .collect();

    let report = serde_json::json!({
        "label": label,
        "timestamp": timestamp,
        "iterations_per_test": iterations,
        "summary": {
            "suites": results.len(),
            "total_time_ms": total_time.as_secs_f64() * 1000.0,
            "total_ops": total_ops,
            "avg_op_time_ns": avg,
        },
        "suites": suite_entries,
    });

    let path = out_dir.join(format!("report-{label}-{timestamp}.json"));
    fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).expect("write report");
    path
}
