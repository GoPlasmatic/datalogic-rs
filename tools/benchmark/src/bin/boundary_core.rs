//! Boundary-benchmark runner for the Rust core (`runtime: "rust-core"`).
//!
//! Measures the shared-floor tiers from BINDINGS-OVERHEAD.md section 1
//! ("The shared floor: what the current contract costs before any FFI")
//! on the core crate directly — compile once, then per call:
//!
//! | mode                              | per-call work                                  |
//! |-----------------------------------|------------------------------------------------|
//! | eval-preparsed                    | evaluate only (data pre-parsed, arena reused)  |
//! | parse-eval                        | + parse data JSON                              |
//! | parse-eval-serialize              | + serialize result to a JSON `String`          |
//! | parse-eval-serialize-fresharena   | same, but fresh `Bump` arena per call          |
//! | serde-value-in-out                | `serde_json::Value` in, `Value` out            |
//! | parseddata-eval                   | core `ParsedData` handle, evaluate only        |
//!
//! Timing discipline (appendix "Methodology notes" of BINDINGS-OVERHEAD.md):
//! warmup 2,000 iterations, pilot pass sizing N so one timed sample lands
//! near ~250 ms, median of 5 samples, results consumed via `black_box` /
//! length sink. Emits one JSON line per (mode, workload) to stdout:
//!
//! ```json
//! {"runtime": "rust-core", "mode": "parse-eval", "workload": "simple", "ns_op": 106.7}
//! ```
//!
//! Usage:
//! ```bash
//! cargo run --release -p datalogic-bench --bin boundary_core -- \
//!     [workloads-dir] [--modes=a,b] [--workloads=x,y]
//! ```
//! `workloads-dir` defaults to `tools/benchmark/boundary/workloads/`.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use bumpalo::Bump;
use datalogic_rs::{DataValue, Engine, Logic, ParsedData};

const RUNTIME: &str = "rust-core";
/// Warmup iterations (native runtime tier: 2,000 per the methodology).
const WARMUP: u64 = 2_000;
/// Target wall time for one timed sample.
const TARGET_SAMPLE_NS: f64 = 250e6;
/// Timed samples per (mode, workload); the median is reported.
const SAMPLES: usize = 5;
/// The pilot doubles its batch until one batch takes at least this long,
/// so the per-op estimate isn't dominated by timer quantization.
const PILOT_MIN_NS: u128 = 10_000_000;

const MODES: &[&str] = &[
    "eval-preparsed",
    "parse-eval",
    "parse-eval-serialize",
    "parse-eval-serialize-fresharena",
    "serde-value-in-out",
    "parseddata-eval",
];

struct Workload {
    name: &'static str,
    rule: String,
    data: String,
    expected: String,
}

fn load_workloads(dir: &Path) -> Vec<Workload> {
    ["simple", "eligibility", "array100"]
        .iter()
        .map(|name| {
            let read = |suffix: &str| -> String {
                let path = dir.join(format!("{name}.{suffix}.json"));
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
            };
            Workload {
                name,
                rule: read("rule"),
                data: read("data"),
                expected: read("expected"),
            }
        })
        .collect()
}

/// Warmup + pilot + median-of-[`SAMPLES`] timed samples over `batch`,
/// where `batch(n)` runs `n` iterations and returns a sink value that the
/// caller's loop accumulated (consumed here through `black_box` so the
/// whole batch can't be elided). Returns median ns/op.
fn measure<F: FnMut(u64) -> u64>(mut batch: F) -> f64 {
    black_box(batch(WARMUP));

    // Pilot: double the batch size until a batch takes >= PILOT_MIN_NS.
    let mut n: u64 = 32;
    let per_op = loop {
        let t = Instant::now();
        black_box(batch(n));
        let elapsed = t.elapsed().as_nanos();
        if elapsed >= PILOT_MIN_NS {
            break elapsed as f64 / n as f64;
        }
        n = n.saturating_mul(2);
    };

    let iters = ((TARGET_SAMPLE_NS / per_op).round() as u64).max(1);
    let mut samples: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let t = Instant::now();
            black_box(batch(iters));
            t.elapsed().as_nanos() as f64 / iters as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[SAMPLES / 2]
}

fn emit(mode: &str, workload: &str, ns_op: f64) {
    // Plain formatting keeps the line schema obvious; 3 decimals is well
    // below run-to-run noise.
    println!(
        "{{\"runtime\": \"{RUNTIME}\", \"mode\": \"{mode}\", \"workload\": \"{workload}\", \"ns_op\": {ns_op:.3}}}"
    );
}

/// Abort loudly when a mode's one-time verification pass doesn't produce
/// the checked-in expected result. Never time a wrong answer.
fn verify(mode: &str, workload: &str, got: &str, expected: &str) {
    if got != expected {
        eprintln!(
            "boundary_core: verification failed for mode={mode} workload={workload}\n  expected: {expected}\n  got:      {got}"
        );
        std::process::exit(1);
    }
}

/// Run one (mode, workload) measurement: mode-specific setup, a one-time
/// correctness check against the checked-in expected result, then the
/// warmup/pilot/median-of-5 loop. Preparsed modes keep the data in its
/// own long-lived arena while the eval arena resets per call.
fn run_mode(engine: &Engine, rule: &Logic, w: &Workload, mode: &str) -> f64 {
    match mode {
        "eval-preparsed" => {
            // Input arena holds the parsed data for the whole run; the
            // eval arena is reset per call (the "arena reused" tier).
            let data_arena = Bump::new();
            let stable: &str = data_arena.alloc_str(&w.data);
            let dv = DataValue::from_str(stable, &data_arena).expect("data parse");
            let data_ref: &DataValue = data_arena.alloc(dv);

            let out = {
                let check_arena = Bump::new();
                engine
                    .evaluate(rule, data_ref, &check_arena)
                    .expect("eval")
                    .to_string()
            };
            verify(mode, w.name, &out, &w.expected);

            let mut arena = Bump::with_capacity(1 << 20);
            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    arena.reset();
                    let out = engine.evaluate(rule, data_ref, &arena).unwrap();
                    sink = sink.wrapping_add(black_box(out) as *const _ as u64);
                }
                sink
            })
        }
        "parse-eval" => {
            let out = {
                let arena = Bump::new();
                let dv = DataValue::from_str(&w.data, &arena).expect("data parse");
                engine.evaluate(rule, dv, &arena).expect("eval").to_string()
            };
            verify(mode, w.name, &out, &w.expected);

            let mut arena = Bump::with_capacity(1 << 20);
            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    arena.reset();
                    let dv = DataValue::from_str(&w.data, &arena).unwrap();
                    let out = engine.evaluate(rule, dv, &arena).unwrap();
                    sink = sink.wrapping_add(black_box(out) as *const _ as u64);
                }
                sink
            })
        }
        "parse-eval-serialize" => {
            let out = {
                let arena = Bump::new();
                let dv = DataValue::from_str(&w.data, &arena).expect("data parse");
                engine.evaluate(rule, dv, &arena).expect("eval").to_string()
            };
            verify(mode, w.name, &out, &w.expected);

            let mut arena = Bump::with_capacity(1 << 20);
            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    arena.reset();
                    let dv = DataValue::from_str(&w.data, &arena).unwrap();
                    let out = engine.evaluate(rule, dv, &arena).unwrap();
                    let s = out.to_string();
                    sink = sink.wrapping_add(black_box(&s).len() as u64);
                }
                sink
            })
        }
        "parse-eval-serialize-fresharena" => {
            let out = {
                let arena = Bump::new();
                let dv = DataValue::from_str(&w.data, &arena).expect("data parse");
                engine.evaluate(rule, dv, &arena).expect("eval").to_string()
            };
            verify(mode, w.name, &out, &w.expected);

            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    let arena = Bump::new();
                    let dv = DataValue::from_str(&w.data, &arena).unwrap();
                    let out = engine.evaluate(rule, dv, &arena).unwrap();
                    let s = out.to_string();
                    sink = sink.wrapping_add(black_box(&s).len() as u64);
                }
                sink
            })
        }
        "serde-value-in-out" => {
            // The object bridge: a resident `serde_json::Value` in (deep-
            // converted into the arena per call, same as the bindings'
            // object paths), `serde_json::Value` out.
            let sv: serde_json::Value = serde_json::from_str(&w.data).expect("data parse");
            let expected: serde_json::Value =
                serde_json::from_str(&w.expected).expect("expected parse");

            let out: serde_json::Value = {
                let arena = Bump::new();
                let out = engine.evaluate(rule, &sv, &arena).expect("eval");
                serde_json::to_value(out).expect("to_value")
            };
            if out != expected {
                eprintln!(
                    "boundary_core: verification failed for mode={mode} workload={} (serde value mismatch)",
                    w.name
                );
                std::process::exit(1);
            }

            let mut arena = Bump::with_capacity(1 << 20);
            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    arena.reset();
                    let out = engine.evaluate(rule, &sv, &arena).unwrap();
                    let v = serde_json::to_value(out).unwrap();
                    sink = sink.wrapping_add(black_box(&v) as *const _ as u64);
                }
                sink
            })
        }
        "parseddata-eval" => {
            // The core-side analog of the C ABI's data handle: parse once
            // into a self-contained `ParsedData`, evaluate only per call.
            let parsed = ParsedData::from_json(&w.data).expect("data parse");

            let out = {
                let arena = Bump::new();
                engine
                    .evaluate(rule, &parsed, &arena)
                    .expect("eval")
                    .to_string()
            };
            verify(mode, w.name, &out, &w.expected);

            let mut arena = Bump::with_capacity(1 << 20);
            measure(|n| {
                let mut sink = 0u64;
                for _ in 0..n {
                    arena.reset();
                    let out = engine.evaluate(rule, &parsed, &arena).unwrap();
                    sink = sink.wrapping_add(black_box(out) as *const _ as u64);
                }
                sink
            })
        }
        other => panic!("unknown mode: {other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir: Option<PathBuf> = None;
    let mut mode_filter: Option<Vec<String>> = None;
    let mut workload_filter: Option<Vec<String>> = None;
    for arg in &args {
        if let Some(v) = arg.strip_prefix("--modes=") {
            mode_filter = Some(v.split(',').map(str::to_string).collect());
        } else if let Some(v) = arg.strip_prefix("--workloads=") {
            workload_filter = Some(v.split(',').map(str::to_string).collect());
        } else {
            dir = Some(PathBuf::from(arg));
        }
    }
    let dir = dir.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("boundary/workloads")
    });

    let workloads = load_workloads(&dir);
    let engine = Engine::new();

    for w in &workloads {
        if let Some(f) = &workload_filter {
            if !f.iter().any(|x| x == w.name) {
                continue;
            }
        }
        let rule = engine.compile(w.rule.as_str()).expect("rule compile");
        for mode in MODES {
            if let Some(f) = &mode_filter {
                if !f.iter().any(|x| x == mode) {
                    continue;
                }
            }
            let ns = run_mode(&engine, &rule, w, mode);
            emit(mode, w.name, ns);
        }
    }
}
