//! Evaluating against an `&OwnedDataValue` must not copy the context (#76).
//!
//! A host that keeps its state in an owned value and evaluates many small
//! rules against it should pay for what each rule reads, not for the size
//! of the state. The owned value outlives the evaluation, so its strings,
//! object keys and tensor bytes can be borrowed: only the array and object
//! spines need building in the arena.
//!
//! The tests measure heap bytes allocated on the calling thread during one
//! evaluation, against two contexts of identical shape where one holds
//! leaves a thousand times longer. Whatever the entry point, the difference
//! must not grow with the leaves.

#![cfg(feature = "serde_json")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use bumpalo::Bump;
use datalogic_rs::Engine;
use datavalue::OwnedDataValue;

// ---------------------------------------------------------------------------
// Per-thread allocation counter
// ---------------------------------------------------------------------------

/// Counts bytes requested on the current thread. Thread-local so the test
/// harness running other tests in parallel cannot inflate a measurement.
struct Counting;

thread_local! {
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

fn note(bytes: usize) {
    // `try_with`: the slot is gone during thread teardown, and an allocation
    // there must still succeed.
    let _ = ALLOCATED.try_with(|c| c.set(c.get() + bytes));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size.saturating_sub(layout.size()));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Heap bytes `f` allocates on this thread.
fn allocated_by<T>(f: impl FnOnce() -> T) -> (usize, T) {
    let before = ALLOCATED.with(Cell::get);
    let out = f();
    (ALLOCATED.with(Cell::get) - before, out)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Items in the context's array and object. Both contexts have exactly this
/// many, so their spines cost the same.
const ITEMS: usize = 256;

/// Length of every string leaf in the large context. 256 items of 4 KiB
/// plus 256 keys of 4 KiB plus one 1 MiB blob: about 3 MiB of leaves.
const LONG: usize = 4096;

/// The headroom a difference may use: well under one long leaf, so a
/// single copied string fails the test.
const SLACK: usize = LONG / 2;

/// `{"k": 1, "blob": s, "list": [s; ITEMS], "map": {key_i: s; ITEMS}}` with
/// every string `leaf` bytes long (keys of `map` padded to the same length).
fn context(leaf: usize) -> OwnedDataValue {
    let s = |c: char| c.to_string().repeat(leaf);
    let list = (0..ITEMS).map(|_| OwnedDataValue::String(s('x'))).collect();
    let map = (0..ITEMS)
        .map(|i| {
            let key = format!("{i:0>width$}", width = leaf.max(4));
            (key, OwnedDataValue::String(s('y')))
        })
        .collect();
    OwnedDataValue::Object(vec![
        ("k".into(), OwnedDataValue::from(1i64)),
        ("blob".into(), OwnedDataValue::String(s('z').repeat(256))),
        ("list".into(), OwnedDataValue::Array(list)),
        ("map".into(), OwnedDataValue::Object(map)),
    ])
}

/// The rule every entry point runs: reads one small leaf.
const RULE: &str = r#"{"+": [{"var": "k"}, 1]}"#;

/// Assert that `run` allocates the same against both contexts, give or take
/// [`SLACK`], and that it returns the right answer on both.
fn assert_independent_of_leaf_size<T: std::fmt::Debug + PartialEq>(
    name: &str,
    expected: T,
    run: impl Fn(&OwnedDataValue) -> T,
) {
    let small = context(1);
    let large = context(LONG);
    // Warm up: lazily initialised statics (the top-level helpers' shared
    // engine, thread-local pools) must not land in either measurement.
    let _ = run(&small);
    let _ = run(&large);

    let (small_bytes, small_out) = allocated_by(|| run(&small));
    let (large_bytes, large_out) = allocated_by(|| run(&large));
    assert_eq!(small_out, expected, "{name}: wrong result (small context)");
    assert_eq!(large_out, expected, "{name}: wrong result (large context)");
    assert!(
        large_bytes <= small_bytes + SLACK,
        "{name} allocated {large_bytes} bytes against the large context and \
         {small_bytes} against the small one: it copies the context's leaves"
    );
}

fn two() -> OwnedDataValue {
    OwnedDataValue::from(2i64)
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

#[test]
fn the_fixture_contexts_differ_by_megabytes() {
    // Guards the guard: if the fixtures shrank, every test below would pass
    // whether or not the context is copied.
    let (bytes, _) = allocated_by(|| context(LONG));
    assert!(
        bytes > 2 * 1024 * 1024,
        "large context is only {bytes} bytes"
    );
}

#[test]
fn engine_evaluate() {
    let engine = Engine::new();
    let compiled = engine.compile(RULE).unwrap();
    assert_independent_of_leaf_size("Engine::evaluate", two(), |data| {
        let arena = Bump::new();
        engine.evaluate(&compiled, data, &arena).unwrap().to_owned()
    });
}

#[cfg(feature = "budget")]
#[test]
fn engine_evaluate_metered() {
    let engine = Engine::new();
    let compiled = engine.compile(RULE).unwrap();
    assert_independent_of_leaf_size("Engine::evaluate_metered", two(), |data| {
        let arena = Bump::new();
        engine
            .evaluate_metered(&compiled, data, &arena, u64::MAX)
            .unwrap()
            .value
            .to_owned()
    });
}

#[test]
fn session_eval() {
    let engine = Engine::new();
    let compiled = engine.compile(RULE).unwrap();
    assert_independent_of_leaf_size("Session::eval", two(), |data| {
        engine.session().eval(&compiled, data).unwrap()
    });
}

#[test]
fn session_eval_str() {
    let engine = Engine::new();
    let compiled = engine.compile(RULE).unwrap();
    assert_independent_of_leaf_size("Session::eval_str", "2".to_string(), |data| {
        engine.session().eval_str(&compiled, data).unwrap()
    });
}

#[test]
fn engine_eval() {
    let engine = Engine::new();
    assert_independent_of_leaf_size("Engine::eval", two(), |data| {
        engine.eval(RULE, data).unwrap()
    });
}

#[test]
fn engine_eval_str() {
    let engine = Engine::new();
    assert_independent_of_leaf_size("Engine::eval_str", "2".to_string(), |data| {
        engine.eval_str(RULE, data).unwrap()
    });
}

#[test]
fn engine_eval_into() {
    let engine = Engine::new();
    assert_independent_of_leaf_size("Engine::eval_into", serde_json::json!(2), |data| {
        engine
            .eval_into::<serde_json::Value, _, _>(RULE, data)
            .unwrap()
    });
}

#[test]
fn top_level_eval() {
    assert_independent_of_leaf_size("datalogic_rs::eval", two(), |data| {
        datalogic_rs::eval(RULE, data).unwrap()
    });
}

/// A traced run snapshots the scope into every step, by design: the
/// debugger shows it. So its cost does grow with the context, and the
/// baseline is the same traced run fed through `&ParsedData`, which hands
/// the evaluator an arena value with no conversion at all. Taking the data
/// as an owned value must add nothing that grows with its leaves on top.
#[cfg(feature = "trace")]
fn assert_trace_adds_no_copy(
    name: &str,
    run_owned: impl Fn(&Engine, &datalogic_rs::Logic, &OwnedDataValue) -> String,
) {
    let engine = Engine::new();
    let compiled = engine.compile(RULE).unwrap();
    let measure = |leaf: usize| {
        let owned = context(leaf);
        let parsed = datalogic_rs::ParsedData::from_json(&owned.to_string()).unwrap();
        let zero_copy = || {
            let arena = Bump::new();
            let run = engine.trace().eval_borrowed(&compiled, &parsed, &arena);
            run.result.unwrap().to_string()
        };
        let _ = (zero_copy(), run_owned(&engine, &compiled, &owned));
        let (baseline, a) = allocated_by(zero_copy);
        let (bytes, b) = allocated_by(|| run_owned(&engine, &compiled, &owned));
        assert_eq!((a.as_str(), b.as_str()), ("2", "2"), "{name}: wrong result");
        bytes.saturating_sub(baseline)
    };
    let (small, large) = (measure(1), measure(LONG));
    assert!(
        large <= small + SLACK,
        "{name} allocated {large} bytes over the zero-copy trace against the \
         large context and {small} against the small one: it copies the context's leaves"
    );
}

#[cfg(feature = "trace")]
#[test]
fn traced_eval() {
    assert_trace_adds_no_copy("TracedSession::eval", |engine, compiled, data| {
        engine
            .trace()
            .eval(compiled, data)
            .result
            .unwrap()
            .to_string()
    });
}

#[cfg(feature = "trace")]
#[test]
fn traced_eval_str() {
    assert_trace_adds_no_copy("TracedSession::eval_str", |engine, _, data| {
        engine.trace().eval_str(RULE, data).result.unwrap()
    });
}

#[cfg(feature = "trace")]
#[test]
fn traced_eval_into() {
    assert_trace_adds_no_copy("TracedSession::eval_into", |engine, _, data| {
        let run = engine
            .trace()
            .eval_into::<serde_json::Value, _, _>(RULE, data);
        run.result.unwrap().to_string()
    });
}

#[cfg(feature = "trace")]
#[test]
fn traced_eval_borrowed() {
    assert_trace_adds_no_copy("TracedSession::eval_borrowed", |engine, compiled, data| {
        let arena = Bump::new();
        let run = engine.trace().eval_borrowed(compiled, data, &arena);
        run.result.unwrap().to_string()
    });
}

// ---------------------------------------------------------------------------
// What the borrowed view must still get right
// ---------------------------------------------------------------------------

#[test]
fn a_rule_that_returns_a_leaf_returns_its_content() {
    // The result borrows from the context for as long as the arena lives;
    // converting it out must produce the full string, not a truncated or
    // dangling one.
    let engine = Engine::new();
    let compiled = engine.compile(r#"{"var": "list.3"}"#).unwrap();
    let large = context(LONG);
    let arena = Bump::new();
    let out = engine.evaluate(&compiled, &large, &arena).unwrap();
    assert_eq!(out.as_str().map(str::len), Some(LONG));
    assert_eq!(out.to_owned(), OwnedDataValue::String("x".repeat(LONG)));
}

#[test]
fn keys_and_unicode_survive_the_view() {
    let engine = Engine::new();
    let data: OwnedDataValue = r#"{"ключ": {"日本": ["é", "", "😀"]}, "": {"": 7}}"#
        .parse()
        .unwrap();
    for (rule, expected) in [
        (r#"{"var": "ключ.日本.2"}"#, r#""😀""#),
        (r#"{"var": "ключ.日本"}"#, r#"["é","","😀"]"#),
        (r#"{"val": ["", ""]}"#, "7"),
        (r#"{"var": "ключ"}"#, r#"{"日本":["é","","😀"]}"#),
    ] {
        let compiled = engine.compile(rule).unwrap();
        let arena = Bump::new();
        let out = engine.evaluate(&compiled, &data, &arena).unwrap();
        assert_eq!(out.to_string(), expected, "{rule}");
    }
}

#[test]
fn empty_containers_survive_the_view() {
    let engine = Engine::new();
    let data: OwnedDataValue = r#"{"a": [], "o": {}, "n": null, "nested": [[], {}]}"#
        .parse()
        .unwrap();
    for (rule, expected) in [
        (r#"{"var": "a"}"#, "[]"),
        (r#"{"var": "o"}"#, "{}"),
        (r#"{"var": "n"}"#, "null"),
        (r#"{"var": "nested"}"#, "[[],{}]"),
        (
            r#"{"var": ""}"#,
            r#"{"a":[],"o":{},"n":null,"nested":[[],{}]}"#,
        ),
    ] {
        let compiled = engine.compile(rule).unwrap();
        let arena = Bump::new();
        let out = engine.evaluate(&compiled, &data, &arena).unwrap();
        assert_eq!(out.to_string(), expected, "{rule}");
    }
}

#[cfg(feature = "tensor")]
#[test]
fn tensor_bytes_are_borrowed_not_copied() {
    let engine = Engine::new();
    let tensor_of = |n: usize| {
        let t = engine
            .eval(format!(r#"{{"zeros": [[{n}], "u8"]}}"#).as_str(), "null")
            .unwrap();
        OwnedDataValue::Object(vec![
            ("t".into(), t),
            ("k".into(), OwnedDataValue::from(1i64)),
        ])
    };
    let small = tensor_of(1);
    let large = tensor_of(4 * 1024 * 1024);
    let compiled = engine.compile(r#"{"shape": [{"var": "t"}]}"#).unwrap();
    let run = |data: &OwnedDataValue| {
        let arena = Bump::new();
        engine
            .evaluate(&compiled, data, &arena)
            .unwrap()
            .to_string()
    };
    let _ = (run(&small), run(&large));
    let (small_bytes, small_out) = allocated_by(|| run(&small));
    let (large_bytes, large_out) = allocated_by(|| run(&large));
    assert_eq!(small_out, "[1]");
    assert_eq!(large_out, format!("[{}]", 4 * 1024 * 1024));
    assert!(
        large_bytes <= small_bytes + SLACK,
        "a 4 MiB tensor in the context cost {large_bytes} bytes against {small_bytes}"
    );
}

#[cfg(feature = "datetime")]
#[test]
fn datetimes_survive_the_view() {
    let engine = Engine::new();
    let d = engine
        .eval(r#"{"datetime": "2026-10-01T12:00:00Z"}"#, "null")
        .unwrap();
    let data = OwnedDataValue::Object(vec![("d".into(), d)]);
    let compiled = engine
        .compile(r#"{"format_date": [{"var": "d"}, "%Y-%m-%d"]}"#)
        .unwrap();
    let arena = Bump::new();
    let out = engine.evaluate(&compiled, &data, &arena).unwrap();
    assert_eq!(out.to_string(), r#""2026-10-01""#);
}
