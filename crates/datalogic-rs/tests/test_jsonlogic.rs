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

/// The two engine flavours the suites exercise. Engines are stateless
/// across evaluations, so both are built once and shared by every test
/// case; each case picks one via its `templating` flag.
struct Engines {
    plain: Engine,
    templating: Engine,
}

impl Engines {
    fn new() -> Self {
        Self {
            plain: Engine::new(),
            templating: Engine::builder().with_templating(true).build(),
        }
    }

    fn select(&self, templating: bool) -> &Engine {
        if templating {
            &self.templating
        } else {
            &self.plain
        }
    }
}

/// Per-file pass/fail tally that owns the per-case `✓`/`✗` output lines,
/// so the outcome-classification arms in `run_test_file` don't each repeat
/// the println-then-increment fragment.
#[derive(Default)]
struct Recorder {
    passed: usize,
    failed: usize,
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

    /// Single-line failure: `✗ Test {index}: {description} - {reason}`.
    fn fail_inline(&mut self, index: usize, description: &str, reason: &str) {
        println!("✗ Test {index}: {description} - {reason}");
        self.failed += 1;
    }
}

#[test]
fn test_jsonlogic() {
    // Get test file from environment variable, or run all tests from index.json
    let test_file = env::var("JSONLOGIC_TEST_FILE");

    let engines = Engines::new();

    let mut total_passed = 0;
    let mut total_failed = 0;

    match test_file {
        Ok(file) => {
            // Run single test file
            println!("Running tests from: {}", file);
            let (passed, failed) = run_test_file(&file, &engines);
            total_passed += passed;
            total_failed += failed;
        }
        Err(_) => {
            // Run all tests from index.json
            println!("No JSONLOGIC_TEST_FILE specified, running all tests from index.json\n");

            let index_path = "tests/suites/index.json";
            let index_contents = fs::read_to_string(index_path).expect("Failed to read index.json");

            let index: Vec<String> =
                serde_json::from_str(&index_contents).expect("Failed to parse index.json");

            for test_file in index {
                // Suites under `flagd/` exercise operators registered
                // only under `--features flagd`. Without the feature
                // the operator names parse as `InvalidOperator` and the
                // suite would spuriously fail; skip explicitly so the
                // index can stay feature-agnostic.
                if test_file.starts_with("flagd/") && !cfg!(feature = "flagd") {
                    println!(
                        "WARNING: Skipping {} (requires `flagd` feature)\n",
                        test_file
                    );
                    continue;
                }

                let test_path = format!("tests/suites/{}", test_file);

                // Check if file exists
                if !Path::new(&test_path).exists() {
                    println!("WARNING: Skipping {} (file not found)\n", test_file);
                    continue;
                }

                println!("\n=== Running tests from: {} ===", test_file);
                let (passed, failed) = run_test_file(&test_path, &engines);
                total_passed += passed;
                total_failed += failed;

                println!("  Results: {} passed, {} failed", passed, failed);
            }
        }
    }

    println!("\n========================================");
    println!(
        "TOTAL RESULTS: {} passed, {} failed",
        total_passed, total_failed
    );
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

fn run_test_file(test_file: &str, engines: &Engines) -> (usize, usize) {
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

        let data = test_obj.get("data").cloned().unwrap_or(json!({}));

        // Pick the engine matching the case's templating flag.
        let templating = test_obj
            .get("templating")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let engine = engines.select(templating);

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

    (rec.passed, rec.failed)
}
