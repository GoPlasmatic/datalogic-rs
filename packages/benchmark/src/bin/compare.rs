//! Cross-library benchmark — runs the same JSONLogic suites against
//! multiple engines through a uniform string-in / string-out interface.
//!
//! Each engine is a `Subject`. Add new ones behind a Cargo feature in
//! `Cargo.toml` (e.g. `subject-jsonlogic-rs`) so the default build only
//! pulls in datalogic-rs. The string interface is the only thing every
//! JSONLogic implementation can sustain — language- or arena-specific
//! fast paths belong in `bin/self.rs`.

use std::env;
use std::io::Write;
use std::time::Instant;

use datalogic_bench::{
    SuiteResult, load_index, load_suite, print_summary, suites_root, write_report,
};
use datalogic_rs::Engine;

const ITERATIONS: u32 = 10_000;

trait Subject {
    fn name(&self) -> &'static str;
    /// Evaluate `rule_json` against `data_json` and return a stringified
    /// result. The caller doesn't compare results across subjects — it
    /// only times them — so subjects may differ in stringification, but
    /// must complete without panicking on the bundled suites.
    fn evaluate(&self, rule_json: &str, data_json: &str) -> Result<String, String>;
}

struct DatalogicRs {
    engine: Engine,
}

impl DatalogicRs {
    fn new() -> Self {
        Self {
            engine: Engine::new(),
        }
    }
}

impl Subject for DatalogicRs {
    fn name(&self) -> &'static str {
        "datalogic-rs"
    }

    fn evaluate(&self, rule_json: &str, data_json: &str) -> Result<String, String> {
        self.engine
            .evaluate_str(rule_json, data_json)
            .map_err(|e| format!("{e:?}"))
    }
}

// Future subject impls go here behind feature flags. Example skeleton:
//
//   #[cfg(feature = "subject-jsonlogic-rs")]
//   struct JsonLogicRs;
//   #[cfg(feature = "subject-jsonlogic-rs")]
//   impl Subject for JsonLogicRs {
//       fn name(&self) -> &'static str { "jsonlogic-rs" }
//       fn evaluate(&self, rule: &str, data: &str) -> Result<String, String> { ... }
//   }

fn subjects() -> Vec<Box<dyn Subject>> {
    vec![Box::new(DatalogicRs::new())]
}

fn benchmark_suite(subject: &dyn Subject, suite_name: &str) -> Option<SuiteResult> {
    let path = suites_root().join(suite_name);
    let cases = load_suite(&path)?;

    // Warm-up.
    for c in &cases {
        let _ = subject.evaluate(&c.rule_json, &c.data_json);
    }

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        for c in &cases {
            let _ = subject.evaluate(&c.rule_json, &c.data_json);
        }
    }
    let total_time = start.elapsed();
    let total_ops = ITERATIONS as u64 * cases.len() as u64;

    Some(SuiteResult::new(
        suite_name.to_string(),
        cases.len(),
        total_ops,
        total_time,
    ))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let run_all = args.iter().any(|a| a == "--all");
    let suite_arg = args
        .iter()
        .find(|a| !a.starts_with("--") && *a != &args[0])
        .cloned();

    let suite_files: Vec<String> = if run_all {
        load_index()
    } else {
        vec![suite_arg.unwrap_or_else(|| "compatible.json".into())]
    };

    let subjects = subjects();
    println!(
        "Comparing {} subject(s) across {} suite(s) ({} iterations each)\n",
        subjects.len(),
        suite_files.len(),
        ITERATIONS
    );

    for subject in &subjects {
        println!("=== {} ===", subject.name());
        let mut results: Vec<SuiteResult> = Vec::new();
        for suite in &suite_files {
            print!("  {suite:<48}");
            std::io::stdout().flush().unwrap();
            match benchmark_suite(subject.as_ref(), suite) {
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
        let label = format!("compare-{}", subject.name());
        print_summary(&label, &results);
        let report_path = write_report(&label, ITERATIONS, &results);
        println!("Report saved to {}\n", report_path.display());
    }
}
