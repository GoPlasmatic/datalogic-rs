//! Cross-library benchmark **matrix**.
//!
//! Each row is one operator suite; each column is one "subject" — either
//! an API tier of `datalogic-rs`, a competing Rust crate (gated behind a
//! Cargo feature), or a JS/WASM library run through a Node subprocess.
//! Cells hold the median-of-3 avg ns/op the subject achieved on that
//! suite, with a per-cell wall-time budget so slow subjects don't drag
//! `--all` runs into the tens of minutes.
//!
//! `--macro` swaps the file suites for the synthesized macro tier
//! (`datalogic_bench::macro_suites`, the same large-payload suites
//! `bin/self.rs --macro` times). The cases reach subjects through the
//! identical in-memory protocol, so no suite files are written; the
//! report lands in `report-compare-macro-<timestamp>.json`.
//!
//! datalogic-rs is represented by a single column, `dlrs:engine` —
//! precompiled `Logic` + pre-parsed inputs in a `Bump` + `Engine::evaluate`
//! with a caller-owned arena that resets between iterations. This is the
//! tier that compares apples-to-apples with other libraries' precompile
//! APIs (e.g. `json-logic-engine:compiled`, `dlrs:wasm:compiled`). The
//! convenience-API tiers (`Engine::eval_str`, `Session::eval_borrowed`)
//! are intentionally not in the matrix — their numbers measure
//! API-shape costs (parse cost, session reset cost), not engine cost.
//! For those, see `bin/self.rs`.
//!
//! Adding a subject:
//! - **Native Rust crate**: add a Cargo feature in `tools/benchmark/Cargo.toml`
//!   gating the dep, write a `Subject` impl below behind `#[cfg(feature = "...")]`,
//!   push it into `subjects()`.
//! - **JS / WASM library**: add the dep in `tools/benchmark/runners/package.json`,
//!   add a dispatch entry in `runners/node-runner.js`, push a `NodeSubject`
//!   into `subjects()`.

use std::env;
use std::hint::black_box;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use bumpalo::Bump;
use datalogic_bench::{
    MatrixCell, MatrixRow, SubjectRun, SuiteCase, load_index, load_suite_for_compare,
    macro_suites::macro_suites, pairwise_shared_ratios, render_matrix, render_pairwise_ratios,
    suites_root, write_matrix_report,
};
use datalogic_rs::{DataValue, Engine, Logic};

const TARGET_MS_PER_CELL: u32 = 200;
const SAMPLES_PER_CELL: u32 = 3;
/// Above this fraction of failed evaluations, the cell renders as `ERR`
/// instead of `<n>*`. Catches "subject doesn't support this suite at all"
/// without blacklisting suites whose negative cases got through the
/// `load_suite_for_compare` filter.
const ERR_THRESHOLD: f64 = 0.5;

// ============================================================
// Subject trait
// ============================================================

trait Subject {
    fn name(&self) -> &'static str;

    /// Pilot one iteration to estimate per-op cost, scale to a timed
    /// loop hitting roughly `target_wall_time`, run [`SAMPLES_PER_CELL`]
    /// samples, return the median. `None` = subject can't run this
    /// suite (precompile failed, runtime missing, etc).
    fn run_suite(&mut self, cases: &[SuiteCase], target_wall_time: Duration) -> Option<SubjectRun>;
}

/// Compute a sane iteration count from the warm-up duration.
fn pick_iterations(pilot: Duration, target: Duration) -> u32 {
    if pilot.is_zero() {
        return 1_000_000;
    }
    let ratio = target.as_secs_f64() / pilot.as_secs_f64();
    let iters = ratio.max(1.0) as u64;
    iters.clamp(1, u32::MAX as u64) as u32
}

/// Median of three subject runs by avg ns/op. Returns `None` when fewer
/// than three samples succeeded.
fn median_of_three(samples: Vec<SubjectRun>) -> Option<SubjectRun> {
    if samples.len() < 3 {
        return None;
    }
    let mut sorted = samples;
    sorted.sort_by(|a, b| {
        a.avg_op_ns()
            .partial_cmp(&b.avg_op_ns())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(sorted[1])
}

// ============================================================
// datalogic-rs subject
// ============================================================

struct DlrsEngine {
    engine: Engine,
}

impl DlrsEngine {
    fn new() -> Self {
        Self {
            engine: Engine::new(),
        }
    }
}

impl Subject for DlrsEngine {
    fn name(&self) -> &'static str {
        "dlrs:engine"
    }

    fn run_suite(&mut self, cases: &[SuiteCase], target: Duration) -> Option<SubjectRun> {
        // Pre-compile once; each median sample reuses the same `Logic` set.
        let compiled: Vec<Logic> = cases
            .iter()
            .map(|c| self.engine.compile(c.rule_json.as_str()))
            .collect::<datalogic_rs::Result<_>>()
            .ok()?;

        // Persistent input arena — never reset, so `&DataValue` handles
        // outlive every per-iteration eval-arena reset below.
        let data_arena = Bump::new();
        let inputs: Vec<&DataValue<'_>> = cases
            .iter()
            .map(|c| {
                let av = DataValue::from_str(c.data_json.as_str(), &data_arena).ok()?;
                Some(&*data_arena.alloc(av))
            })
            .collect::<Option<_>>()?;

        // Pilot.
        let mut eval_arena = Bump::new();
        let pilot_start = Instant::now();
        for (rule, data) in compiled.iter().zip(inputs.iter()) {
            match self.engine.evaluate(rule, *data, &eval_arena) {
                Ok(v) => {
                    black_box(v);
                }
                Err(e) => {
                    black_box(e);
                }
            }
        }
        let iters = pick_iterations(pilot_start.elapsed(), target);
        eval_arena.reset();

        let mut samples = Vec::with_capacity(SAMPLES_PER_CELL as usize);
        for _ in 0..SAMPLES_PER_CELL {
            let mut ok = 0u64;
            let mut err = 0u64;
            let start = Instant::now();
            for _ in 0..iters {
                for (rule, data) in compiled.iter().zip(inputs.iter()) {
                    match self.engine.evaluate(rule, *data, &eval_arena) {
                        Ok(v) => {
                            black_box(v);
                            ok += 1;
                        }
                        Err(e) => {
                            black_box(e);
                            err += 1;
                        }
                    }
                }
                // Batch-style reset between iterations: borrows from
                // `eval_arena` end at the close of the inner loop body,
                // so this is safe. Models the "long-lived arena, reset
                // between batches" caller pattern.
                eval_arena.reset();
            }
            samples.push(SubjectRun {
                elapsed: start.elapsed(),
                iterations: iters,
                ok_count: ok,
                err_count: err,
            });
        }
        median_of_three(samples)
    }
}

// ============================================================
// jsonlogic-rs subject (gated)
// ============================================================

#[cfg(feature = "subject-jsonlogic-rs")]
struct JsonLogicRs;

#[cfg(feature = "subject-jsonlogic-rs")]
impl Subject for JsonLogicRs {
    fn name(&self) -> &'static str {
        "jsonlogic-rs"
    }

    fn run_suite(&mut self, cases: &[SuiteCase], target: Duration) -> Option<SubjectRun> {
        let parsed: Vec<(serde_json::Value, serde_json::Value)> = cases
            .iter()
            .map(|c| {
                let rule: serde_json::Value = serde_json::from_str(&c.rule_json).ok()?;
                let data: serde_json::Value = serde_json::from_str(&c.data_json).ok()?;
                Some((rule, data))
            })
            .collect::<Option<_>>()?;

        let pilot_start = Instant::now();
        for (rule, data) in &parsed {
            let _ = jsonlogic_rs::apply(rule, data);
        }
        let iters = pick_iterations(pilot_start.elapsed(), target);

        let mut samples = Vec::with_capacity(SAMPLES_PER_CELL as usize);
        for _ in 0..SAMPLES_PER_CELL {
            let mut ok = 0u64;
            let mut err = 0u64;
            let start = Instant::now();
            for _ in 0..iters {
                for (rule, data) in &parsed {
                    match jsonlogic_rs::apply(rule, data) {
                        Ok(v) => {
                            black_box(v);
                            ok += 1;
                        }
                        Err(e) => {
                            black_box(e);
                            err += 1;
                        }
                    }
                }
            }
            samples.push(SubjectRun {
                elapsed: start.elapsed(),
                iterations: iters,
                ok_count: ok,
                err_count: err,
            });
        }
        median_of_three(samples)
    }
}

// ============================================================
// Node subprocess subjects
// ============================================================

/// Path to the node-runner.js script. Resolved relative to the bench crate
/// root so `cargo run` from any cwd works.
fn node_runner_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("runners")
        .join("node-runner.js")
}

fn node_runner_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runners")
}

/// Has `node` on PATH?
fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Has the named npm dep been installed under `runners/node_modules/`?
fn node_dep_installed(package: &str) -> bool {
    node_runner_dir()
        .join("node_modules")
        .join(package)
        .join("package.json")
        .exists()
}

struct NodeSubject {
    display_name: &'static str,
    library: &'static str,
}

impl NodeSubject {
    fn new(display_name: &'static str, library: &'static str) -> Self {
        Self {
            display_name,
            library,
        }
    }
}

impl Subject for NodeSubject {
    fn name(&self) -> &'static str {
        self.display_name
    }

    fn run_suite(&mut self, cases: &[SuiteCase], target: Duration) -> Option<SubjectRun> {
        // Pre-parse once on the Rust side, send both parsed and raw forms.
        // The runner picks whichever its library prefers (apply() takes
        // parsed objects; our wasm `evaluate` takes JSON strings).
        let case_payload: Option<Vec<serde_json::Value>> = cases
            .iter()
            .map(|c| {
                let rule: serde_json::Value = serde_json::from_str(&c.rule_json).ok()?;
                let data: serde_json::Value = serde_json::from_str(&c.data_json).ok()?;
                Some(serde_json::json!({
                    "rule": rule,
                    "data": data,
                    "rule_str": c.rule_json,
                    "data_str": c.data_json,
                }))
            })
            .collect();
        let case_payload = case_payload?;

        let payload = serde_json::json!({
            "library": self.library,
            "target_ms": target.as_millis() as u64,
            "samples": SAMPLES_PER_CELL,
            "cases": case_payload,
        });
        let payload_bytes = serde_json::to_vec(&payload).ok()?;

        let mut child = Command::new("node")
            .arg(node_runner_path())
            .current_dir(node_runner_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        child.stdin.as_mut()?.write_all(&payload_bytes).ok()?;
        // Close stdin so the runner reads EOF.
        drop(child.stdin.take());

        let output = child.wait_with_output().ok()?;
        if !output.status.success() {
            eprintln!(
                "  ! node-runner ({}) failed: {}",
                self.library,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return None;
        }

        // Runner writes a single JSON line: { elapsed_ns, iterations, ok_count, err_count }.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let last_line = stdout.lines().last().unwrap_or("").trim();
        let parsed: serde_json::Value = serde_json::from_str(last_line).ok()?;
        let elapsed_ns = parsed.get("elapsed_ns")?.as_u64()?;
        let iterations = parsed.get("iterations")?.as_u64()? as u32;
        let ok_count = parsed.get("ok_count")?.as_u64()?;
        let err_count = parsed.get("err_count")?.as_u64()?;
        Some(SubjectRun {
            elapsed: Duration::from_nanos(elapsed_ns),
            iterations,
            ok_count,
            err_count,
        })
    }
}

// ============================================================
// Subject registry + missing-runtime detection
// ============================================================

struct SubjectAvailability {
    subjects: Vec<Box<dyn Subject>>,
    missing: Vec<&'static str>,
}

fn build_subjects() -> SubjectAvailability {
    let mut subjects: Vec<Box<dyn Subject>> = vec![Box::new(DlrsEngine::new())];
    let mut missing: Vec<&'static str> = Vec::new();

    // jsonlogic-rs (Rust, Cargo-feature gated). Always present when the
    // feature is on; absence is a build-time choice, not a runtime miss.
    #[cfg(feature = "subject-jsonlogic-rs")]
    {
        subjects.push(Box::new(JsonLogicRs));
    }

    // Node subjects — runtime-detected. If `node` is missing or the
    // backing npm package isn't in `runners/node_modules/`, we mark the
    // subject as missing so `--all` mode can hard-fail by default.
    //
    // The `library` slot is the LIBS dispatch key in node-runner.js, not
    // necessarily an npm package name — `json-logic-engine-compiled` is
    // a separate dispatch entry that pre-compiles rules but maps onto the
    // same npm package as `json-logic-engine`.
    let node_ok = node_available();
    let node_subjects: &[(&'static str, &'static str, &'static str)] = &[
        // (display name, LIBS dispatch key, npm package to probe)
        (
            "dlrs:wasm:compiled",
            "@goplasmatic/datalogic-wasm-compiled",
            "@goplasmatic/datalogic-wasm",
        ),
        ("json-logic-js", "json-logic-js", "json-logic-js"),
        (
            "json-logic-engine",
            "json-logic-engine",
            "json-logic-engine",
        ),
        (
            "json-logic-engine:compiled",
            "json-logic-engine-compiled",
            "json-logic-engine",
        ),
    ];
    for (display, lib, pkg) in node_subjects {
        if node_ok && node_dep_installed(pkg) {
            subjects.push(Box::new(NodeSubject::new(display, lib)));
        } else {
            missing.push(display);
        }
    }

    SubjectAvailability { subjects, missing }
}

// ============================================================
// Matrix runner
// ============================================================

/// Run one named case set (from a suite file or synthesized in code)
/// against every subject and collect the row of cells.
fn run_cases(
    subjects: &mut [Box<dyn Subject>],
    suite_name: &str,
    cases: &[SuiteCase],
) -> MatrixRow {
    let target = Duration::from_millis(TARGET_MS_PER_CELL as u64);

    print!("  {suite_name:<40}");
    std::io::stdout().flush().ok();

    let mut cells = Vec::with_capacity(subjects.len());
    for subject in subjects.iter_mut() {
        let cell = match subject.run_suite(cases, target) {
            Some(run) => {
                let total = run.ok_count + run.err_count;
                let err_frac = if total == 0 {
                    0.0
                } else {
                    run.err_count as f64 / total as f64
                };
                if err_frac > ERR_THRESHOLD {
                    MatrixCell::Error
                } else {
                    MatrixCell::Value {
                        ns_per_op: run.avg_op_ns(),
                        partial: run.err_count > 0,
                    }
                }
            }
            None => MatrixCell::Unavailable,
        };
        cells.push(cell);
    }
    println!(" done");

    MatrixRow {
        suite: suite_name.to_string(),
        test_count: cases.len(),
        cells,
    }
}

fn run_one_suite(subjects: &mut [Box<dyn Subject>], suite_name: &str) -> Option<MatrixRow> {
    let path = suites_root().join(suite_name);
    let cases = load_suite_for_compare(&path)?;
    Some(run_cases(subjects, suite_name, &cases))
}

// ============================================================
// CLI
// ============================================================

struct CliArgs {
    suites: Vec<String>,
    allow_missing_subjects: bool,
    /// Run the synthesized macro suites (`macro_suites::macro_suites`)
    /// instead of file suites. Same subjects, same per-cell budget; the
    /// cases go to subjects through the identical in-memory protocol
    /// (`SuiteCase` slices, stdin JSON for Node), so nothing is written
    /// to disk.
    macro_mode: bool,
}

fn parse_args() -> CliArgs {
    let mut suites: Vec<String> = Vec::new();
    let mut all = false;
    let mut allow_missing = false;
    let mut macro_mode = false;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--all" => all = true,
            "--allow-missing-subjects" => allow_missing = true,
            "--macro" => macro_mode = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            s if s.starts_with("--") => {
                eprintln!("unknown flag: {s}");
                print_help();
                std::process::exit(2);
            }
            s => suites.push(s.to_string()),
        }
    }
    if macro_mode {
        if all || !suites.is_empty() {
            eprintln!("--macro selects the synthesized macro suites; drop --all / suite names");
            std::process::exit(2);
        }
    } else if all {
        suites = load_index();
    } else if suites.is_empty() {
        suites.push("compatible.json".to_string());
    }
    CliArgs {
        suites,
        allow_missing_subjects: allow_missing,
        macro_mode,
    }
}

fn print_help() {
    println!(
        "datalogic-bench compare — cross-library matrix\n\n\
         Usage:\n\
         \x20 cargo run --release -p datalogic-bench --bin compare [--features <flags>] -- [SUITE]...\n\
         \x20 cargo run --release -p datalogic-bench --bin compare -- --all\n\n\
         Flags:\n\
         \x20 --all                        Run every suite from tests/suites/index.json\n\
         \x20 --macro                      Run the synthesized macro suites (1k/10k arrays,\n\
         \x20                              128-key object, deep nesting, 10 KB strings,\n\
         \x20                              eligibility) instead of file suites; report saved\n\
         \x20                              as report-compare-macro-<timestamp>.json\n\
         \x20 --allow-missing-subjects     Skip Node subjects whose runtime isn't installed\n\
         \x20                              (default is hard-fail when any are missing)\n\
         \x20 -h, --help                   This help text\n\n\
         Cargo features:\n\
         \x20 --features subject-jsonlogic-rs   Add the `jsonlogic-rs` Rust crate column\n\n\
         Node subjects (auto-detected when `runners/node_modules/<pkg>` is installed):\n\
         \x20 - dlrs:wasm        @goplasmatic/datalogic-wasm via Node\n\
         \x20 - json-logic-js    jwadhams/json-logic-js via Node\n\n\
         To set up Node subjects:\n\
         \x20 cd bindings/wasm && ./build.sh\n\
         \x20 cd tools/benchmark/runners && npm install"
    );
}

fn main() {
    let args = parse_args();
    let SubjectAvailability {
        mut subjects,
        missing,
    } = build_subjects();

    if !missing.is_empty() {
        if args.allow_missing_subjects {
            eprintln!(
                "warning: skipping {n} unavailable subject(s): {names}",
                n = missing.len(),
                names = missing.join(", ")
            );
        } else {
            eprintln!(
                "error: {n} subject(s) unavailable: {names}\n\n\
                 To install Node subjects:\n  \
                 cd bindings/wasm && ./build.sh\n  \
                 cd tools/benchmark/runners && npm install\n\n\
                 Or pass --allow-missing-subjects to render the matrix without them.",
                n = missing.len(),
                names = missing.join(", ")
            );
            std::process::exit(2);
        }
    }

    let macro_set = if args.macro_mode {
        macro_suites()
    } else {
        Vec::new()
    };
    let n_suites = if args.macro_mode {
        macro_set.len()
    } else {
        args.suites.len()
    };

    let subject_names: Vec<&str> = subjects.iter().map(|s| s.name()).collect();
    println!(
        "Running {n_suites} {kind}suite(s) × {n_subjects} subject(s) ({samples} samples × ~{target}ms each)",
        kind = if args.macro_mode { "macro " } else { "" },
        n_subjects = subjects.len(),
        samples = SAMPLES_PER_CELL,
        target = TARGET_MS_PER_CELL,
    );

    let mut rows: Vec<MatrixRow> = Vec::new();
    if args.macro_mode {
        for suite in &macro_set {
            rows.push(run_cases(&mut subjects, suite.name, &suite.cases));
        }
    } else {
        for suite_name in &args.suites {
            match run_one_suite(&mut subjects, suite_name) {
                Some(row) => rows.push(row),
                None => println!("  {suite_name:<40} (skipped — no valid cases)"),
            }
        }
    }

    if rows.is_empty() {
        eprintln!("no rows to render — all suites were empty");
        std::process::exit(1);
    }

    render_matrix(&subject_names, &rows, TARGET_MS_PER_CELL, SAMPLES_PER_CELL);

    // Pairwise shared-suite ratios. The per-column mean rows in the matrix
    // aggregate whatever suites each column completed, so when subjects
    // ERR on different suites, dividing two column geomeans compares
    // incomparable suite sets. These ratios are computed per pair, only
    // over suites where both subjects have finite cells.
    let ratios = pairwise_shared_ratios(subject_names.len(), &rows);
    render_pairwise_ratios(&subject_names, &ratios);

    let label = if args.macro_mode {
        "compare-macro"
    } else {
        "compare"
    };
    let report_path = write_matrix_report(
        label,
        &subject_names,
        &rows,
        &ratios,
        TARGET_MS_PER_CELL,
        SAMPLES_PER_CELL,
    );
    println!("\nReport saved to {}", report_path.display());
}
