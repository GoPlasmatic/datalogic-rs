//! Single-engine benchmark — datalogic-rs alone, using the fast arena path
//! (compile once, persistent input arena, session-managed eval arena reset
//! between iterations). For cross-library comparison see `bin/compare.rs`.
//!
//! Why `Session` rather than a hand-rolled `Bump`:
//! - `Session` is the documented hot-loop API. Benchmarking it makes the
//!   reported numbers match what callers actually see in production.
//! - `eval_borrowed` returns a borrowed `&DataValue<'a>` tied to the session
//!   arena (no `OwnedDataValue::to_owned` deep-clone), preserving the
//!   "max perf, zero-copy result" measurement of the original benchmark.
//! - The session does not reset implicitly; the benchmark calls
//!   `session.reset()` after every `eval_borrowed` so peak memory stays
//!   bounded by the largest single evaluation (the bump pointer returns
//!   to chunk start without freeing chunks).
//!
//! Self-tuning arena sizing: the warm-up phase mirrors the timed-loop
//! shape exactly (same `eval_borrowed` + `reset` cadence), so the bump
//! grows to the suite's largest-single-eval high-water mark. We then
//! call `session.reset_with_capacity(session.allocated_bytes())` to drop
//! the warm-up's chunks and allocate one fresh chunk of exactly that
//! size — guaranteeing zero chunk-growth events during the timed inner
//! loop. The warmed size is printed alongside each suite's results.
//!
//! Folded vs non-folded split: the whole-suite number (the headline,
//! comparable across report generations) is measured first, exactly as
//! before. Two additional passes with the same discipline then time the
//! constant-folded rules (`Logic::is_constant`) and the rest separately,
//! so folded rules (which measure literal-return overhead, not engine
//! work) can't silently flatter the data-dependent number.
//!
//! Macro tier (`--macro`): synthesized large-payload suites (1k/10k-element
//! arrays, 128-key objects, 48-level nesting, 10 KB strings, one realistic
//! eligibility rule) from `datalogic_bench::macro_suites`. Same timing
//! discipline, but the per-suite iteration count is scaled from a pilot
//! pass so one timed rep lands near ~250 ms; a fixed 100k iterations on a
//! 10k-element array would run for minutes per suite.

use std::env;
use std::io::Write;
use std::time::{Duration, Instant};

use bumpalo::Bump;
use datalogic_bench::{
    SuiteResult, geomean, load_index, load_suite, macro_suites::macro_suites, print_suite_line,
    print_summary, suites_root, write_report,
};
use datalogic_rs::{DataValue, Engine, Logic};

const ITERATIONS: u32 = 100_000;

/// Timed repetitions per suite. The median rep feeds `SuiteResult` (and
/// the report); min/max give the spread so a single noisy run is visible
/// instead of silently becoming the headline number.
const TIMED_REPS: usize = 3;

/// Target wall time for one timed rep of a macro-tier suite. The macro
/// runner pilots one evaluation per case and scales the iteration count
/// to land near this budget (mirrors the per-cell pilot in
/// `bin/compare.rs`).
const MACRO_TARGET_MS: u64 = 250;

/// Spread of the timed reps around the median, as half the min-max range
/// in percent. Small (under ~2%) means the median is trustworthy.
fn spread_pct(min: f64, median: f64, max: f64) -> f64 {
    if median == 0.0 {
        0.0
    } else {
        (max - min) / 2.0 / median * 100.0
    }
}

/// One timed pass over a fixed set of (rule, input) pairs.
struct PassTiming {
    /// Median rep wall time.
    total_time: Duration,
    total_ops: u64,
    avg_op_ns: f64,
    warmed_size: usize,
    spread: f64,
}

/// Warm-up, arena pre-size, then median-of-[`TIMED_REPS`] timed passes
/// over `pairs` at `iterations` evaluations per case per rep.
///
/// This is the single timed-loop shape shared by the whole-suite pass,
/// the folded/non-folded subset passes, and the macro tier (which passes
/// a pilot-scaled iteration count). Mechanics are unchanged from the
/// original whole-suite loop: `black_box` around every evaluation,
/// `session.reset()` per iteration, and `reset_with_capacity` to the
/// warm-up high-water mark before timing.
fn time_pass(engine: &Engine, pairs: &[(&Logic, &DataValue)], iterations: u32) -> PassTiming {
    // Session owns the eval arena. The session does not auto-reset; the
    // bench resets after every call (per-iteration) and pre-sizes the
    // arena from the warm-up's high-water mark before the timed loop.
    let mut session = engine.session();

    // Warm-up — same per-iteration `eval_borrowed` + `reset` shape as the
    // timed loop, so the bump grows to the largest-single-eval high-water
    // mark. `Bump::reset` keeps the largest chunk; subsequent iterations
    // either fit (no growth) or trigger another doubling.
    for &(rule, data) in pairs {
        for _ in 0..iterations {
            let _ = session.eval_borrowed(rule, data);
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

    // Median of TIMED_REPS timed passes. `black_box` the evaluation
    // result inside the loop so the optimizer can't elide the eval as
    // unused work (the result borrows the session arena and drops at
    // statement end, before the reset).
    let mut rep_times = Vec::with_capacity(TIMED_REPS);
    for _ in 0..TIMED_REPS {
        let start = Instant::now();
        for &(rule, data) in pairs {
            for _ in 0..iterations {
                std::hint::black_box(session.eval_borrowed(rule, data).ok());
                session.reset();
            }
        }
        rep_times.push(start.elapsed());
    }
    std::hint::black_box(pairs);
    rep_times.sort();
    let total_time = rep_times[TIMED_REPS / 2];
    let total_ops = iterations as u64 * pairs.len() as u64;
    let ns_of = |t: &Duration| t.as_nanos() as f64 / total_ops as f64;
    let spread = spread_pct(
        ns_of(&rep_times[0]),
        ns_of(&total_time),
        ns_of(&rep_times[TIMED_REPS - 1]),
    );
    PassTiming {
        avg_op_ns: ns_of(&total_time),
        total_time,
        total_ops,
        warmed_size,
        spread,
    }
}

/// A suite's whole-run numbers plus the whole-pass arena/spread details
/// printed on the per-suite line.
struct SuiteRun {
    result: SuiteResult,
    warmed_size: usize,
    spread: f64,
}

fn benchmark_suite(engine: &Engine, suite_name: &str) -> Option<SuiteRun> {
    let path = suites_root().join(suite_name);
    let cases = load_suite(&path)?;

    // Pre-compile every rule.
    let compiled: Vec<Logic> = cases
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

    let pairs: Vec<(&Logic, &DataValue)> = compiled.iter().zip(inputs.iter().copied()).collect();

    // Whole-suite pass: the headline number, comparable with reports
    // generated before the folded/non-folded split existed.
    let whole = time_pass(engine, &pairs, ITERATIONS);

    let mut result = SuiteResult::new(
        suite_name.to_string(),
        pairs.len(),
        whole.total_ops,
        whole.total_time,
    );

    // Subset passes: constant-folded rules vs the rest, same discipline.
    // The whole-suite number above is untouched by this split.
    let folded_pairs: Vec<(&Logic, &DataValue)> = pairs
        .iter()
        .copied()
        .filter(|(rule, _)| rule.is_constant())
        .collect();
    let non_folded_pairs: Vec<(&Logic, &DataValue)> = pairs
        .iter()
        .copied()
        .filter(|(rule, _)| !rule.is_constant())
        .collect();

    result.folded_count = Some(folded_pairs.len());
    if !folded_pairs.is_empty() {
        let pass = time_pass(engine, &folded_pairs, ITERATIONS);
        result.folded_avg_op_ns = Some(pass.avg_op_ns);
        result.folded_total_time = Some(pass.total_time);
    }
    if !non_folded_pairs.is_empty() {
        let pass = time_pass(engine, &non_folded_pairs, ITERATIONS);
        result.non_folded_avg_op_ns = Some(pass.avg_op_ns);
        result.non_folded_total_time = Some(pass.total_time);
    }

    std::hint::black_box((&pairs, &data_arena));

    Some(SuiteRun {
        result,
        warmed_size: whole.warmed_size,
        spread: whole.spread,
    })
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

/// Render the folded/non-folded segment of a per-suite line, e.g.
/// `| folded 20/32 @ 2.11 ns, rest @ 58.30 ns`. Timing halves are
/// omitted when the corresponding subset is empty.
fn fmt_split(r: &SuiteResult) -> String {
    let Some(folded_count) = r.folded_count else {
        return String::new();
    };
    let mut s = format!(" | folded {folded_count}/{}", r.test_count);
    if let Some(ns) = r.folded_avg_op_ns {
        s.push_str(&format!(" @ {ns:.2} ns"));
    }
    if let Some(ns) = r.non_folded_avg_op_ns {
        s.push_str(&format!(", rest @ {ns:.2} ns"));
    }
    s
}

/// Compute the pilot-scaled iteration count for a macro suite: one timed
/// rep should land near [`MACRO_TARGET_MS`].
fn macro_iterations(pilot: Duration) -> u32 {
    let target = Duration::from_millis(MACRO_TARGET_MS);
    if pilot.is_zero() {
        return 1_000_000;
    }
    let ratio = target.as_secs_f64() / pilot.as_secs_f64();
    (ratio.max(1.0) as u64).clamp(1, u32::MAX as u64) as u32
}

/// Run the synthesized macro suites. Prints per-suite lines and a macro
/// summary; does not write a report file (per-suite iteration counts
/// differ, so the numbers live on the printed lines).
fn run_macro_tier(engine: &Engine, label: &str) {
    let suites = macro_suites();
    println!("Macro tier: {} synthesized suites ({label})", suites.len());
    println!(
        "Per-suite iterations scaled from a pilot pass so one timed rep lands near \
         ~{MACRO_TARGET_MS} ms; median of {TIMED_REPS} reps, black_box + per-iteration reset \
         as in the micro suites.\n"
    );

    let mut results: Vec<SuiteResult> = Vec::new();
    for suite in &suites {
        print!("  {:<24}", suite.name);
        std::io::stdout().flush().unwrap();

        let compiled: Vec<Logic> = suite
            .cases
            .iter()
            .map(|c| {
                engine
                    .compile(c.rule_json.as_str())
                    .expect("macro rule compiles")
            })
            .collect();

        let data_arena = Bump::new();
        let inputs: Vec<&DataValue<'_>> = suite
            .cases
            .iter()
            .map(|c| {
                let av = DataValue::from_str(&c.data_json, &data_arena).expect("macro data parses");
                &*data_arena.alloc(av)
            })
            .collect();
        let pairs: Vec<(&Logic, &DataValue)> =
            compiled.iter().zip(inputs.iter().copied()).collect();

        // Sanity: every macro case must evaluate cleanly. A silently
        // erroring rule would time the error path and mean nothing.
        {
            let mut session = engine.session();
            for (idx, &(rule, data)) in pairs.iter().enumerate() {
                if let Err(e) = session.eval_borrowed(rule, data) {
                    panic!(
                        "macro case {idx} of {} failed to evaluate: {e:?}",
                        suite.name
                    );
                }
                session.reset();
            }
        }

        // Pilot: scale the iteration count from one measured pass. The
        // first (untimed) pass warms code paths and grows the pilot
        // session's arena, so the timed pilot pass doesn't overestimate
        // per-op cost from cold caches and chunk growth and land the reps
        // well short of the wall-time target.
        let iterations = {
            let mut session = engine.session();
            for &(rule, data) in &pairs {
                std::hint::black_box(session.eval_borrowed(rule, data).ok());
                session.reset();
            }
            let start = Instant::now();
            for &(rule, data) in &pairs {
                std::hint::black_box(session.eval_borrowed(rule, data).ok());
                session.reset();
            }
            macro_iterations(start.elapsed())
        };

        let pass = time_pass(engine, &pairs, iterations);
        std::hint::black_box((&pairs, &data_arena));

        println!(
            "{:>2} cases | iters {:>7} | avg {:>11.2} ns/op ±{:>4.1}% | total {:>8.1?} | arena {:>9}",
            pairs.len(),
            iterations,
            pass.avg_op_ns,
            pass.spread,
            pass.total_time,
            fmt_bytes(pass.warmed_size)
        );
        results.push(SuiteResult::new(
            suite.name.to_string(),
            pairs.len(),
            pass.total_ops,
            pass.total_time,
        ));
    }

    // Macro summary: geomean over per-suite avg ns/op (equal weight per
    // suite; a total-ops-weighted average would be dominated by the fast
    // suites, which get the most iterations from the pilot scaling).
    let total_time: Duration = results.iter().map(|r| r.total_time).sum();
    let avgs: Vec<f64> = results.iter().map(|r| r.avg_op_ns).collect();
    println!("\n=== Macro summary ({label}) ===");
    println!("Suites:              {}", results.len());
    println!("Total timed:         {total_time:.2?} (median reps, excludes warm-up/pilot)");
    println!(
        "Geomean op time:     {:.2} ns (per-suite avg)",
        geomean(&avgs)
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let run_all = args.iter().any(|a| a == "--all");
    let run_macro = args.iter().any(|a| a == "--macro");

    let engine = Engine::new();
    let version = env!("CARGO_PKG_VERSION");
    let label = format!("self-v{version}");

    if run_macro {
        run_macro_tier(&engine, &label);
    } else if run_all {
        let suite_files = load_index();
        println!("Benchmarking all {} suites ({label})\n", suite_files.len());

        let mut results: Vec<SuiteResult> = Vec::new();
        for suite in &suite_files {
            print!("  {suite:<48}");
            std::io::stdout().flush().unwrap();
            match benchmark_suite(&engine, suite) {
                Some(run) => {
                    println!(
                        "{:>4} tests | avg {:>8.2} ns/op ±{:>4.1}% | total {:>10.1?} | arena {:>9}{}",
                        run.result.test_count,
                        run.result.avg_op_ns,
                        run.spread,
                        run.result.total_time,
                        fmt_bytes(run.warmed_size),
                        fmt_split(&run.result)
                    );
                    results.push(run.result);
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
            Some(run) => {
                let r = &run.result;
                println!("\n=== Benchmark Results ===");
                println!("Test cases:          {}", r.test_count);
                println!("Iterations per test: {ITERATIONS}");
                println!("Timed reps (median): {TIMED_REPS}");
                println!("Total operations:    {}", r.total_ops);
                println!("Total time:          {:.2?}", r.total_time);
                println!(
                    "Average op time:     {:.2} ns (±{:.1}% across reps)",
                    r.avg_op_ns, run.spread
                );
                println!(
                    "Arena bytes:         {} ({} B)",
                    fmt_bytes(run.warmed_size),
                    run.warmed_size
                );
                if let Some(folded_count) = r.folded_count {
                    match r.folded_avg_op_ns {
                        Some(ns) => println!(
                            "Folded rules:        {folded_count}/{} (avg {ns:.2} ns/op)",
                            r.test_count
                        ),
                        None => println!("Folded rules:        0/{}", r.test_count),
                    }
                    if let Some(ns) = r.non_folded_avg_op_ns {
                        println!(
                            "Non-folded rules:    {}/{} (avg {ns:.2} ns/op)",
                            r.test_count - folded_count,
                            r.test_count
                        );
                    }
                }
                print_suite_line(r);
            }
            None => {
                eprintln!("No valid test cases found in {suite}");
                std::process::exit(1);
            }
        }
    }
}
