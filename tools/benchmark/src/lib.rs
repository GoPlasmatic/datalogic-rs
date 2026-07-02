//! Shared benchmark harness for `datalogic-bench`.
//!
//! Both `bin/self.rs` (datalogic-rs alone, fast arena path) and
//! `bin/compare.rs` (cross-library, string-in/string-out apples-to-apples
//! interface) reuse the suite loader, summary printer, and JSON reporter
//! defined here. Engine-specific timing loops live in their respective
//! binaries because the inner loop differs (arena reuse vs string round-trip).

pub mod macro_suites;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

/// Resolve a suite path relative to the workspace root, so the benchmark
/// works regardless of the caller's cwd.
pub fn suites_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("crates")
        .join("datalogic-rs")
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
/// section-header strings and entries without a `rule` field. Includes
/// negative-test cases (those with an `error` field) — used by `bin/self.rs`
/// where every Rust call is the same engine and error-path cost doesn't
/// skew comparisons. Cross-library callers should prefer
/// [`load_suite_for_compare`].
pub fn load_suite(file_path: &Path) -> Option<Vec<SuiteCase>> {
    load_suite_inner(file_path, false)
}

/// Load a suite file into reusable (rule, data) string pairs, **dropping
/// negative-test cases** (those with an `error` field instead of
/// `result`). Cross-library benchmarks include subjects whose error
/// paths differ wildly in cost (e.g. richly-formatted `Display` impls
/// vs cheap return-null), so including negative cases would penalise
/// the verbose ones unfairly. The matrix runner in `bin/compare.rs`
/// uses this variant.
pub fn load_suite_for_compare(file_path: &Path) -> Option<Vec<SuiteCase>> {
    load_suite_inner(file_path, true)
}

fn load_suite_inner(file_path: &Path, drop_error_cases: bool) -> Option<Vec<SuiteCase>> {
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
        if drop_error_cases && test_case.contains_key("error") && !test_case.contains_key("result")
        {
            continue;
        }
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
    /// Number of rules the compiler constant-folded to a literal
    /// (`Logic::is_constant`). `None` when the run didn't classify (the
    /// whole-suite headline number is unaffected either way).
    pub folded_count: Option<usize>,
    /// Median avg ns/op over just the folded rules; `None` when the suite
    /// has no folded rules (or the split wasn't measured).
    pub folded_avg_op_ns: Option<f64>,
    /// Median total wall time of the folded-subset pass.
    pub folded_total_time: Option<Duration>,
    /// Median avg ns/op over just the non-folded rules; `None` when every
    /// rule folded (or the split wasn't measured).
    pub non_folded_avg_op_ns: Option<f64>,
    /// Median total wall time of the non-folded-subset pass.
    pub non_folded_total_time: Option<Duration>,
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
            folded_count: None,
            folded_avg_op_ns: None,
            folded_total_time: None,
            non_folded_avg_op_ns: None,
            non_folded_total_time: None,
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

    // Geomeans give each suite equal weight (the arithmetic average above
    // is dominated by big suites) and split the folded and non-folded rule
    // populations so constant-folded rules can't flatter the number for
    // genuinely data-dependent work.
    let (overall, folded, non_folded) = summary_geomeans(results);
    println!(
        "Geomean op time:     {:.2} ns (per-suite avg, {} suites)",
        geomean(&overall),
        overall.len()
    );
    if !folded.is_empty() {
        println!(
            "Geomean folded:      {:.2} ns ({} suites with folded rules)",
            geomean(&folded),
            folded.len()
        );
    }
    if !non_folded.is_empty() {
        println!(
            "Geomean non-folded:  {:.2} ns ({} suites with non-folded rules)",
            geomean(&non_folded),
            non_folded.len()
        );
    }
}

/// Per-suite avg ns/op vectors feeding the three summary geomeans:
/// (overall, folded-only, non-folded-only). Suites without the relevant
/// subset simply don't contribute to that geomean.
fn summary_geomeans(results: &[SuiteResult]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let overall: Vec<f64> = results.iter().map(|r| r.avg_op_ns).collect();
    let folded: Vec<f64> = results.iter().filter_map(|r| r.folded_avg_op_ns).collect();
    let non_folded: Vec<f64> = results
        .iter()
        .filter_map(|r| r.non_folded_avg_op_ns)
        .collect();
    (overall, folded, non_folded)
}

/// Write a JSON report into `tools/benchmark/output/`.
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
            // Additive folded/non-folded split fields: absent (not null)
            // when the run didn't measure the corresponding subset, so
            // pre-split reports and post-split reports stay diffable on
            // the shared fields.
            let mut entry = serde_json::json!({
                "suite": r.name,
                "test_count": r.test_count,
                "total_ops": r.total_ops,
                "total_time_ms": r.total_time.as_secs_f64() * 1000.0,
                "avg_op_time_ns": r.avg_op_ns,
            });
            if let Some(count) = r.folded_count {
                entry["folded_test_count"] = count.into();
            }
            if let Some(ns) = r.folded_avg_op_ns {
                entry["folded_avg_op_time_ns"] = ns.into();
            }
            if let Some(t) = r.folded_total_time {
                entry["folded_total_time_ms"] = (t.as_secs_f64() * 1000.0).into();
            }
            if let Some(ns) = r.non_folded_avg_op_ns {
                entry["non_folded_avg_op_time_ns"] = ns.into();
            }
            if let Some(t) = r.non_folded_total_time {
                entry["non_folded_total_time_ms"] = (t.as_secs_f64() * 1000.0).into();
            }
            entry
        })
        .collect();

    let (overall, folded, non_folded) = summary_geomeans(results);
    let mut summary = serde_json::json!({
        "suites": results.len(),
        "total_time_ms": total_time.as_secs_f64() * 1000.0,
        "total_ops": total_ops,
        "avg_op_time_ns": avg,
        "geomean_avg_op_time_ns": geomean(&overall),
    });
    if !folded.is_empty() {
        summary["geomean_folded_avg_op_time_ns"] = geomean(&folded).into();
        summary["geomean_folded_suites"] = folded.len().into();
    }
    if !non_folded.is_empty() {
        summary["geomean_non_folded_avg_op_time_ns"] = geomean(&non_folded).into();
        summary["geomean_non_folded_suites"] = non_folded.len().into();
    }

    let report = serde_json::json!({
        "label": label,
        "timestamp": timestamp,
        "iterations_per_test": iterations,
        "summary": summary,
        "suites": suite_entries,
    });

    let path = out_dir.join(format!("report-{label}-{timestamp}.json"));
    fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).expect("write report");
    path
}

// ============================================================
// Cross-library matrix support (used by `bin/compare.rs`).
// ============================================================

/// One measured run of a subject against a suite. The cross-library
/// runner does median-of-three by collecting three of these and picking
/// the middle ns/op.
#[derive(Clone, Copy, Debug)]
pub struct SubjectRun {
    pub elapsed: Duration,
    pub iterations: u32,
    pub ok_count: u64,
    pub err_count: u64,
}

impl SubjectRun {
    /// Average ns per op across **all** evaluations (success + error). The
    /// matrix uses this rather than ns-per-success because (a) we can't
    /// always tell from a black-box subject which evaluation failed, and
    /// (b) keeping the denominator total-evals matches what the timed
    /// loop actually executed.
    pub fn avg_op_ns(&self) -> f64 {
        let total_ops = self.ok_count + self.err_count;
        if total_ops == 0 {
            return 0.0;
        }
        self.elapsed.as_nanos() as f64 / total_ops as f64
    }
}

/// One cell in the matrix output. `Value` carries the median ns/op plus
/// a flag for "this subject errored on some-but-not-all cases" (rendered
/// with a trailing `*` and a footnote).
#[derive(Clone, Debug)]
pub enum MatrixCell {
    /// Subject ran. `partial = true` when some cases errored but the
    /// subject was not fully blocked.
    Value { ns_per_op: f64, partial: bool },
    /// Subject ran but errored on (effectively) every case. Renders as
    /// `ERR`.
    Error,
    /// Subject was unavailable (Cargo feature off, runtime missing,
    /// suite couldn't be precompiled). Renders as `—`.
    Unavailable,
}

/// One row in the matrix — a suite's per-subject cells.
pub struct MatrixRow {
    pub suite: String,
    pub test_count: usize,
    pub cells: Vec<MatrixCell>,
}

/// Geometric mean over the finite, positive values in `xs`. Empty or
/// all-non-finite input returns NaN. Used for the bottom-of-matrix
/// aggregation row — geomean is the convention for cross-library
/// benchmarks because a single slow suite doesn't dominate the total
/// the way it does with arithmetic mean.
pub fn geomean(xs: &[f64]) -> f64 {
    let logs: Vec<f64> = xs
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x > 0.0)
        .map(f64::ln)
        .collect();
    if logs.is_empty() {
        return f64::NAN;
    }
    let mean_log = logs.iter().sum::<f64>() / logs.len() as f64;
    mean_log.exp()
}

/// Arithmetic mean over the finite values in `xs`. Empty input returns NaN.
pub fn arith_mean(xs: &[f64]) -> f64 {
    let vals: Vec<f64> = xs.iter().copied().filter(|x| x.is_finite()).collect();
    if vals.is_empty() {
        return f64::NAN;
    }
    vals.iter().sum::<f64>() / vals.len() as f64
}

/// Render the matrix as a markdown table to stdout.
///
/// Layout: `Suite` column on the left, then one column per subject in
/// `subject_names` order. Right-aligned numeric cells. Two aggregation
/// rows at the bottom (`arithmetic mean`, `geometric mean`) computed
/// over `MatrixCell::Value` cells per column.
///
/// `target_wall_time_ms` and `samples_per_cell` go into the header so
/// the reader knows the timing budget the cells were measured against.
pub fn render_matrix(
    subject_names: &[&str],
    rows: &[MatrixRow],
    target_wall_time_ms: u32,
    samples_per_cell: u32,
) {
    // Column widths — start from header text, grow to fit the widest cell.
    let suite_col_header = "Suite";
    let suite_col_width = rows
        .iter()
        .map(|r| r.suite.len())
        .chain(std::iter::once(suite_col_header.len()))
        .chain(std::iter::once("geometric mean".len()))
        .max()
        .unwrap_or(8);

    let mut col_widths: Vec<usize> = subject_names.iter().map(|n| n.len()).collect();
    for row in rows {
        for (i, cell) in row.cells.iter().enumerate() {
            let w = format_cell(cell).len();
            if w > col_widths[i] {
                col_widths[i] = w;
            }
        }
    }
    // Aggregation rows can also widen the columns.
    let agg_values = aggregations(subject_names.len(), rows);
    for (i, w) in col_widths.iter_mut().enumerate() {
        for (mean, _) in &agg_values {
            let s = format_aggregate(mean[i]);
            if s.len() > *w {
                *w = s.len();
            }
        }
    }

    println!(
        "\n=== Cross-Library Matrix — avg ns/op (median of {samples_per_cell}, ~{target_wall_time_ms}ms target/cell, {n} suites) ===\n",
        n = rows.len()
    );

    // Header
    print!("| {:<w$} ", suite_col_header, w = suite_col_width);
    for (name, w) in subject_names.iter().zip(col_widths.iter()) {
        print!("| {:>w$} ", name, w = *w);
    }
    println!("|");

    // Separator (markdown alignment hints — left for first col, right for the rest).
    print!("|{:-<w$}", "", w = suite_col_width + 2);
    for w in &col_widths {
        // ":------:" pattern ends with a colon for right-align; `{:->w$}` fills with `-`.
        print!("|{:->w$}:", "", w = *w + 1);
    }
    println!("|");

    // Body
    let mut any_partial = false;
    for row in rows {
        print!("| {:<w$} ", row.suite, w = suite_col_width);
        for (cell, w) in row.cells.iter().zip(col_widths.iter()) {
            let s = format_cell(cell);
            if matches!(cell, MatrixCell::Value { partial: true, .. }) {
                any_partial = true;
            }
            print!("| {:>w$} ", s, w = *w);
        }
        println!("|");
    }

    // Aggregation rows.
    let labels = ["arithmetic mean", "geometric mean"];
    for ((mean_row, _), label) in agg_values.iter().zip(labels.iter()) {
        print!("| {:<w$} ", label, w = suite_col_width);
        for (v, w) in mean_row.iter().zip(col_widths.iter()) {
            print!("| {:>w$} ", format_aggregate(*v), w = *w);
        }
        println!("|");
    }

    if any_partial {
        println!("\n* partial coverage — subject errored on some cases in this suite.");
    }
}

fn format_cell(cell: &MatrixCell) -> String {
    match cell {
        MatrixCell::Value { ns_per_op, partial } => {
            let suffix = if *partial { "*" } else { "" };
            format!("{:.1}{}", ns_per_op, suffix)
        }
        MatrixCell::Error => "ERR".to_string(),
        MatrixCell::Unavailable => "—".to_string(),
    }
}

fn format_aggregate(v: f64) -> String {
    if v.is_finite() {
        format!("{:.1}", v)
    } else {
        "—".to_string()
    }
}

/// Returns one (per-subject means vec, dummy) tuple per aggregation row,
/// in the order `[arithmetic_mean, geometric_mean]`. The dummy second
/// element is reserved for future use (e.g. confidence intervals);
/// keeping the shape lets callers iterate uniformly with row labels.
fn aggregations(num_subjects: usize, rows: &[MatrixRow]) -> [(Vec<f64>, ()); 2] {
    let mut arith = vec![f64::NAN; num_subjects];
    let mut geo = vec![f64::NAN; num_subjects];
    for j in 0..num_subjects {
        let col_values: Vec<f64> = rows
            .iter()
            .filter_map(|r| match r.cells.get(j) {
                Some(MatrixCell::Value { ns_per_op, .. }) => Some(*ns_per_op),
                _ => None,
            })
            .collect();
        arith[j] = arith_mean(&col_values);
        geo[j] = geomean(&col_values);
    }
    [(arith, ()), (geo, ())]
}

/// A pairwise shared-suite comparison between two matrix columns.
///
/// The per-column mean rows at the bottom of the matrix average over
/// whatever suites each column happened to complete, so when subjects
/// error on different suites, dividing two column geomeans compares
/// incomparable suite sets. A `PairRatio` avoids that: it is the geomean
/// of per-suite ratios computed **only** over suites where both subjects
/// have finite cells, so every contributing number pairs like with like.
#[derive(Clone, Copy, Debug)]
pub struct PairRatio {
    /// Column index of the baseline subject (denominator).
    pub base_idx: usize,
    /// Column index of the compared subject (numerator).
    pub other_idx: usize,
    /// Geomean of per-suite `other ns/op ÷ base ns/op`. Greater than 1
    /// means `other` is slower than `base` on the shared suites.
    pub ratio: f64,
    /// Number of suites where both subjects produced a finite cell.
    pub shared_suites: usize,
}

/// Compute [`PairRatio`]s for every unordered subject pair (the baseline
/// is always the lower column index, so with the datalogic-rs reference
/// in column 0 the first block of pairs reads "subject vs reference").
/// Pairs with zero shared suites are omitted.
pub fn pairwise_shared_ratios(num_subjects: usize, rows: &[MatrixRow]) -> Vec<PairRatio> {
    let finite = |cell: Option<&MatrixCell>| match cell {
        Some(MatrixCell::Value { ns_per_op, .. }) if ns_per_op.is_finite() && *ns_per_op > 0.0 => {
            Some(*ns_per_op)
        }
        _ => None,
    };

    let mut out = Vec::new();
    for base in 0..num_subjects {
        for other in (base + 1)..num_subjects {
            let ratios: Vec<f64> = rows
                .iter()
                .filter_map(|r| {
                    let base_ns = finite(r.cells.get(base))?;
                    let other_ns = finite(r.cells.get(other))?;
                    Some(other_ns / base_ns)
                })
                .collect();
            if ratios.is_empty() {
                continue;
            }
            out.push(PairRatio {
                base_idx: base,
                other_idx: other,
                ratio: geomean(&ratios),
                shared_suites: ratios.len(),
            });
        }
    }
    out
}

/// Print the pairwise shared-suite ratio table produced by
/// [`pairwise_shared_ratios`]. Each line is normalised so the printed
/// multiplier is >= 1 and the direction is spelled out ("slower than").
pub fn render_pairwise_ratios(subject_names: &[&str], ratios: &[PairRatio]) {
    if ratios.is_empty() {
        return;
    }
    let name_width = subject_names.iter().map(|n| n.len()).max().unwrap_or(12);
    println!("\n=== Pairwise shared-suite ratios ===\n");
    println!(
        "Geomean of per-suite ns/op ratios, computed only over suites where both\n\
         subjects have finite cells. The per-column mean rows above cover different\n\
         suite subsets when a subject errors; these ratios never mix subsets.\n"
    );
    for pr in ratios {
        // Normalise so the multiplier reads naturally regardless of which
        // side of the pair is faster on the shared set.
        let (slow, fast, mult) = if pr.ratio >= 1.0 {
            (pr.other_idx, pr.base_idx, pr.ratio)
        } else {
            (pr.base_idx, pr.other_idx, 1.0 / pr.ratio)
        };
        println!(
            "  {:<name_width$}  {:>7.1}x slower than {:<name_width$} over {:>2} shared suites",
            subject_names[slow], mult, subject_names[fast], pr.shared_suites
        );
    }
}

/// Write the cross-library matrix (cells, per-column means, pairwise
/// shared-suite ratios) as a JSON report into `tools/benchmark/output/`.
/// The `self` report format is untouched; this is a separate
/// `report-compare-<timestamp>.json` file.
pub fn write_matrix_report(
    subject_names: &[&str],
    rows: &[MatrixRow],
    ratios: &[PairRatio],
    target_wall_time_ms: u32,
    samples_per_cell: u32,
) -> PathBuf {
    let out_dir = output_root();
    fs::create_dir_all(&out_dir).expect("create output dir");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock went backwards")
        .as_secs();

    let cell_json = |cell: &MatrixCell| match cell {
        MatrixCell::Value { ns_per_op, partial } => serde_json::json!({
            "avg_op_time_ns": ns_per_op,
            "partial": partial,
        }),
        MatrixCell::Error => serde_json::json!({ "status": "error" }),
        MatrixCell::Unavailable => serde_json::json!({ "status": "unavailable" }),
    };

    let suite_entries: Vec<Value> = rows
        .iter()
        .map(|row| {
            let cells: serde_json::Map<String, Value> = subject_names
                .iter()
                .zip(row.cells.iter())
                .map(|(name, cell)| ((*name).to_string(), cell_json(cell)))
                .collect();
            serde_json::json!({
                "suite": row.suite,
                "test_count": row.test_count,
                "cells": cells,
            })
        })
        .collect();

    let [(arith, _), (geo, _)] = aggregations(subject_names.len(), rows);
    let f64_or_null = |v: f64| -> Value { if v.is_finite() { v.into() } else { Value::Null } };
    let column_means: serde_json::Map<String, Value> = subject_names
        .iter()
        .enumerate()
        .map(|(j, name)| {
            let finite_suites = rows
                .iter()
                .filter(|r| matches!(r.cells.get(j), Some(MatrixCell::Value { .. })))
                .count();
            (
                (*name).to_string(),
                serde_json::json!({
                    "arithmetic_mean_ns": f64_or_null(arith[j]),
                    "geometric_mean_ns": f64_or_null(geo[j]),
                    "finite_suites": finite_suites,
                }),
            )
        })
        .collect();

    let ratio_entries: Vec<Value> = ratios
        .iter()
        .map(|pr| {
            serde_json::json!({
                "base": subject_names[pr.base_idx],
                "other": subject_names[pr.other_idx],
                "geomean_ratio_other_over_base": pr.ratio,
                "shared_suites": pr.shared_suites,
            })
        })
        .collect();

    let report = serde_json::json!({
        "label": "compare",
        "timestamp": timestamp,
        "target_ms_per_cell": target_wall_time_ms,
        "samples_per_cell": samples_per_cell,
        "subjects": subject_names,
        "suites": suite_entries,
        "column_means": column_means,
        "pairwise_shared_ratios": ratio_entries,
    });

    let path = out_dir.join(format!("report-compare-{timestamp}.json"));
    fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).expect("write report");
    path
}
