// The full JSONLogic suite runner exercises every operator in `tests/suites/`
// — including the gated ones (templating / datetime / try-throw / ext-*). Gate
// behind `templating` because the runner unconditionally builds an engine with
// `Engine::builder().with_templating(true).build()` for the test cases that
// request it; in
// practice users running this runner will want `--all-features` to actually
// exercise every suite.
#![cfg(all(feature = "templating", feature = "serde_json"))]

use datalogic_rs::Engine;
use serde_json::{Value, json};

use std::env;
use std::fs;
use std::path::Path;

/// The engine flavours the suites exercise, keyed by the knobs a test case
/// can ask for: its `templating` flag and its optional
/// `template_key_escape` char. Engines are stateless across evaluations, so
/// each distinct flavour is built once on first use and shared by every
/// case that asks for it.
///
/// Lazy construction rather than a fixed set of pre-built flavours: it lets
/// a suite pick any escape char (and combine one with `templating: false`
/// to pin the backward-compat behaviour) without the harness needing to
/// know the list up front.
#[derive(Default)]
struct Engines {
    by_flavour: std::collections::HashMap<(bool, Option<char>), Engine>,
}

impl Engines {
    fn new() -> Self {
        Self::default()
    }

    fn select(&mut self, templating: bool, key_escape: Option<char>) -> &Engine {
        self.by_flavour
            .entry((templating, key_escape))
            .or_insert_with(|| {
                let mut builder = Engine::builder().with_templating(templating);
                if let Some(c) = key_escape {
                    builder = builder.with_template_key_escape(c);
                }
                builder.build()
            })
    }
}

/// Per-file pass/fail tally that owns the per-case `✓`/`✗` output lines,
/// so the outcome-classification arms in `run_test_file` don't each repeat
/// the println-then-increment fragment.
#[derive(Default)]
struct Recorder {
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl Recorder {
    /// `✓ Test {index}: {description}`, plus an optional note such as
    /// `(error as expected)`.
    fn pass(&mut self, index: usize, description: &str, note: Option<&str>) {
        match note {
            Some(note) => println!("✓ Test {index}: {description} {note}"),
            None => println!("✓ Test {index}: {description}"),
        }
        self.passed += 1;
    }

    /// `✗ Test {index}: {description}` followed by indented expected/got
    /// detail lines.
    fn fail(&mut self, index: usize, description: &str, details: &[String]) {
        println!("✗ Test {index}: {description}");
        for detail in details {
            println!("  {detail}");
        }
        self.failed += 1;
    }

    /// `⊘ Test {index}: {description} (needs `{op}`)` — the build lacks the
    /// feature this case's operator lives behind.
    fn skip(&mut self, index: usize, description: &str, op: &str) {
        println!("⊘ Test {index}: {description} (operator `{op}` not compiled in)");
        self.skipped += 1;
    }

    /// Single-line failure: `✗ Test {index}: {description} - {reason}`.
    fn fail_inline(&mut self, index: usize, description: &str, reason: &str) {
        println!("✗ Test {index}: {description} - {reason}");
        self.failed += 1;
    }
}

/// Operators that exist only behind a cargo feature, grouped by the feature
/// that gates them. Availability is looked up through [`feature_enabled`],
/// so the feature-to-`cfg!` mapping lives in one place.
///
/// The suite index is deliberately feature-agnostic: it lists every suite, and
/// a reduced-feature build simply cannot evaluate some of them. Without this,
/// `cargo test --no-default-features --features serde_json,templating,trace`
/// fails on `throw` and `switch` with "Unknown Operator" — which is why CI's
/// `feature-matrix` job only *builds* its legs instead of testing them.
///
/// Kept as an explicit table rather than derived from
/// `Engine::builtin_operator_names`, because that reports what *this* build
/// has and so cannot distinguish "gated off" from "misspelled". A genuine typo
/// in a suite stays absent from this table, so it still fails loudly instead of
/// being skipped. `gated_operator_table_matches_engine` guards the table
/// against drift.
const GATED_OPERATORS: &[(&str, &[&str])] = &[
    (
        "datetime",
        &[
            "datetime",
            "timestamp",
            "parse_date",
            "format_date",
            "date_diff",
            "now",
        ],
    ),
    ("error-handling", &["try", "throw"]),
    ("ext-array", &["sort", "slice", "group_by", "distinct"]),
    ("ext-control", &["exists", "??", "switch", "match", "type"]),
    ("ext-math", &["abs", "ceil", "floor"]),
    ("ext-object", &["keys", "values", "entries"]),
    (
        "ext-string",
        &[
            "length",
            "starts_with",
            "ends_with",
            "upper",
            "lower",
            "trim",
            "split",
        ],
    ),
    ("flagd", &["fractional", "sem_ver"]),
    (
        "tensor",
        &[
            "tensor",
            "zeros",
            "full",
            "scatter",
            "rle_expand",
            "one_hot",
            "stack",
            "concat",
            "unstack",
            "reshape",
            "transpose",
            "pad",
            "crop",
            "cast",
            "normalize",
            "argmax",
            "gather",
            "to_list",
            "shape",
            "dtype",
        ],
    ),
];

/// Every gated operator this build did *not* compile in.
fn absent_operators() -> impl Iterator<Item = &'static str> {
    GATED_OPERATORS
        .iter()
        .filter(|(feature, _)| !feature_enabled(feature))
        .flat_map(|(_, ops)| ops.iter().copied())
}

/// Whether this build has the named cargo feature. Backs a case's optional
/// `requires` field, for cases that need a feature for reasons the operator
/// walk cannot see — a duration string only coerces to a duration under
/// `datetime`, for instance, even though the rule is a plain `*` — and the
/// [`GATED_OPERATORS`] table.
fn feature_enabled(name: &str) -> bool {
    match name {
        "datetime" => cfg!(feature = "datetime"),
        "error-handling" => cfg!(feature = "error-handling"),
        "ext-array" => cfg!(feature = "ext-array"),
        "ext-control" => cfg!(feature = "ext-control"),
        "ext-math" => cfg!(feature = "ext-math"),
        "ext-object" => cfg!(feature = "ext-object"),
        "ext-string" => cfg!(feature = "ext-string"),
        "flagd" => cfg!(feature = "flagd"),
        "templating" => cfg!(feature = "templating"),
        "tensor" => cfg!(feature = "tensor"),
        "tensor-half" => cfg!(feature = "tensor-half"),
        other => panic!("unknown feature {other:?} in a case's `requires` list"),
    }
}

/// The first operator in `rule` that this build cannot evaluate, if any.
///
/// Walks single-key objects, which is how an operator invocation is spelled.
/// A multi-key object is a templating literal and never a call, and a key that
/// is not a known gated operator is left alone so unknown-operator cases still
/// assert.
fn absent_operator(rule: &Value) -> Option<&'static str> {
    match rule {
        Value::Object(map) => {
            if map.len() == 1 {
                let key = map.keys().next().expect("len checked");
                if let Some(name) = absent_operators().find(|name| name == key) {
                    return Some(name);
                }
            }
            map.values().find_map(absent_operator)
        }
        Value::Array(items) => items.iter().find_map(absent_operator),
        _ => None,
    }
}

/// The table must name real operators. Under `--all-features` every entry has
/// to be live, which catches a rename or a typo in the table itself; a *new*
/// gated operator nobody added here still surfaces the old way, as a loud
/// "Unknown Operator" failure under a reduced-feature build.
#[test]
fn gated_operator_table_matches_engine() {
    let engine = Engine::new();
    let live: std::collections::HashSet<&str> = engine.builtin_operator_names().collect();
    for (feature, ops) in GATED_OPERATORS {
        for name in *ops {
            assert_eq!(
                live.contains(name),
                feature_enabled(feature),
                "GATED_OPERATORS disagrees with the engine about `{name}` (feature `{feature}`)"
            );
        }
    }
}

#[test]
fn test_jsonlogic() {
    // Get test file from environment variable, or run all tests from index.json
    let test_file = env::var("JSONLOGIC_TEST_FILE");

    let mut engines = Engines::new();

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;

    match test_file {
        Ok(file) => {
            // Run single test file
            println!("Running tests from: {}", file);
            let (passed, failed, skipped) = run_test_file(&file, &mut engines);
            total_passed += passed;
            total_failed += failed;
            total_skipped += skipped;
        }
        Err(_) => {
            // Run all tests from index.json
            println!("No JSONLOGIC_TEST_FILE specified, running all tests from index.json\n");

            let index_path = "tests/suites/index.json";
            let index_contents = fs::read_to_string(index_path).expect("Failed to read index.json");

            let index: Vec<String> =
                serde_json::from_str(&index_contents).expect("Failed to parse index.json");

            for test_file in index {
                let test_path = format!("tests/suites/{}", test_file);

                // Check if file exists
                if !Path::new(&test_path).exists() {
                    println!("WARNING: Skipping {} (file not found)\n", test_file);
                    continue;
                }

                println!("\n=== Running tests from: {} ===", test_file);
                let (passed, failed, skipped) = run_test_file(&test_path, &mut engines);
                total_passed += passed;
                total_failed += failed;
                total_skipped += skipped;

                if skipped > 0 {
                    println!(
                        "  Results: {} passed, {} failed, {} skipped",
                        passed, failed, skipped
                    );
                } else {
                    println!("  Results: {} passed, {} failed", passed, failed);
                }
            }
        }
    }

    println!("\n========================================");
    println!(
        "TOTAL RESULTS: {} passed, {} failed, {} skipped",
        total_passed, total_failed, total_skipped
    );
    if total_skipped > 0 {
        println!(
            "({} cases need operators this build did not compile in)",
            total_skipped
        );
    }
    println!("========================================");

    if total_failed > 0 {
        panic!("Some tests failed!");
    }
}

/// Map an engine error onto the JSON shape the suites' `error` expectations
/// use: a thrown value serialises as itself; `InvalidArguments` /
/// `InvalidOperator` become `{"type": ...}` objects. `None` for error kinds
/// the suites don't encode.
fn error_expectation_json(error: &datalogic_rs::Error) -> Option<Value> {
    if let Some(thrown) = error.thrown_value() {
        return Some(serde_json::to_value(thrown).unwrap_or(Value::Null));
    }
    match &error.kind {
        datalogic_rs::ErrorKind::InvalidArguments(msg) => Some(json!({"type": msg})),
        datalogic_rs::ErrorKind::InvalidOperator(_) => Some(json!({"type": "Unknown Operator"})),
        _ => None,
    }
}

/// Shared error-vs-expectation bookkeeping for the compile-error and
/// eval-error arms of `run_test_file`: match `error` against the case's
/// `error` expectation (if any) and record the outcome.
fn record_error_case(
    rec: &mut Recorder,
    index: usize,
    description: &str,
    error: &datalogic_rs::Error,
    expected_error: Option<&Value>,
    compiling: bool,
) {
    let Some(expected_obj) = expected_error else {
        let reason = if compiling {
            format!("Compilation error: {error}")
        } else {
            format!("Unexpected evaluation error: {error}")
        };
        rec.fail_inline(index, description, &reason);
        return;
    };
    match error_expectation_json(error) {
        Some(actual) if &actual == expected_obj => {
            rec.pass(index, description, Some("(error as expected)"));
        }
        Some(actual) => rec.fail(
            index,
            description,
            &[
                format!("Expected error: {expected_obj:?}"),
                format!("Got error:      {actual:?}"),
            ],
        ),
        None => rec.fail(
            index,
            description,
            &[
                format!("Expected error: {expected_obj:?}"),
                if compiling {
                    format!("Got compilation error: {error:?}")
                } else {
                    format!("Got error:      {error:?}")
                },
            ],
        ),
    }
}

fn run_test_file(test_file: &str, engines: &mut Engines) -> (usize, usize, usize) {
    // Read and parse test file
    let contents = fs::read_to_string(test_file)
        .unwrap_or_else(|e| panic!("Failed to read test file {test_file}: {e}"));

    let test_cases: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|e| panic!("Failed to parse JSON from {test_file}: {e}"));

    let test_array = test_cases
        .as_array()
        .expect("Test file should contain an array of test cases");

    let mut rec = Recorder::default();

    for (index, test_case) in test_array.iter().enumerate() {
        // Skip string entries (they're usually section headers)
        if test_case.is_string() {
            println!("\n{}", test_case.as_str().unwrap());
            continue;
        }

        let test_obj = test_case
            .as_object()
            .unwrap_or_else(|| panic!("Test case {index} should be an object"));

        let description = test_obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        let rule = test_obj
            .get("rule")
            .unwrap_or_else(|| panic!("Test case {index} missing 'rule'"));

        // A reduced-feature build cannot evaluate every suite. Skip rather
        // than fail, and account for it so the run stays honest about what it
        // actually covered.
        if let Some(op) = absent_operator(rule) {
            rec.skip(index, description, op);
            continue;
        }
        // A case may also declare a feature it needs for value semantics
        // rather than for an operator name.
        if let Some(requires) = test_obj.get("requires") {
            let features = requires
                .as_array()
                .unwrap_or_else(|| panic!("Test case {index}: 'requires' must be an array"));
            if let Some(missing) = features
                .iter()
                .map(|f| {
                    f.as_str().unwrap_or_else(|| {
                        panic!("Test case {index}: 'requires' entries must be strings")
                    })
                })
                .find(|f| !feature_enabled(f))
            {
                rec.skip(index, description, missing);
                continue;
            }
        }

        let data = test_obj.get("data").cloned().unwrap_or(json!({}));

        // Pick the engine matching the case's templating flag and its
        // optional template-key escape char.
        let templating = test_obj
            .get("templating")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let key_escape = test_obj.get("template_key_escape").map(|v| {
            let s = v.as_str().unwrap_or_else(|| {
                panic!("Test case {index}: 'template_key_escape' must be a string")
            });
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => c,
                _ => panic!(
                    "Test case {index}: 'template_key_escape' must be exactly one character, got {s:?}"
                ),
            }
        });
        let engine = engines.select(templating, key_escape);

        // Each case asserts either a `result` or an `error` expectation.
        let expected_error = test_obj.get("error");
        let expected_result = test_obj.get("result");

        if expected_error.is_none() && expected_result.is_none() {
            panic!("Test case {index} missing 'result' or 'error'");
        }

        // Compile and evaluate
        match engine.compile(rule) {
            Ok(compiled) => match engine
                .session()
                .eval_into::<serde_json::Value, _>(&compiled, &data)
            {
                Ok(result) => {
                    if expected_error.is_some() {
                        rec.fail(
                            index,
                            description,
                            &[
                                format!("Expected error: {expected_error:?}"),
                                format!("Got result:     {result:?}"),
                            ],
                        );
                    } else if let Some(expected) = expected_result {
                        if &result == expected {
                            rec.pass(index, description, None);
                        } else {
                            rec.fail(
                                index,
                                description,
                                &[
                                    format!("Expected: {expected:?}"),
                                    format!("Got:      {result:?}"),
                                ],
                            );
                        }
                    }
                }
                Err(e) => {
                    record_error_case(&mut rec, index, description, &e, expected_error, false);
                }
            },
            Err(e) => record_error_case(&mut rec, index, description, &e, expected_error, true),
        }
    }

    (rec.passed, rec.failed, rec.skipped)
}
