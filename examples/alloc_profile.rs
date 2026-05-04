//! Allocation-counting profiler — measures allocs/free/bytes per evaluate().
//! Used to validate the arena POC measurement criteria from ARENA_RFC.md.
#![allow(deprecated)]

use datalogic_rs::DataLogic;
use serde_json::Value;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

struct CountingAllocator;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREES.fetch_add(1, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: CountingAllocator = CountingAllocator;

fn snapshot() -> (u64, u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
        FREES.load(Ordering::Relaxed),
    )
}

fn measure(name: &str, engine: &DataLogic, rule: Value, data: Value, iters: u32) {
    let compiled = engine.compile_serde_value(&rule).unwrap();
    let data_arc = Arc::new(data);
    for _ in 0..1000 {
        let _ = engine.evaluate_arc_value(&compiled, data_arc.clone());
    }
    let (a0, b0, f0) = snapshot();
    let t0 = Instant::now();
    for _ in 0..iters {
        let _ = engine.evaluate_arc_value(&compiled, data_arc.clone());
    }
    let elapsed = t0.elapsed();
    let (a1, b1, f1) = snapshot();
    let ns_per = elapsed.as_nanos() as f64 / iters as f64;
    let alloc_per = (a1 - a0) as f64 / iters as f64;
    let free_per = (f1 - f0) as f64 / iters as f64;
    let bytes_per = (b1 - b0) as f64 / iters as f64;
    println!(
        "  {:<40} {:>7.1} ns | {:>5.1} alloc | {:>5.1} free | {:>6.0} B",
        name, ns_per, alloc_per, free_per, bytes_per
    );
}

fn main() {
    let engine = DataLogic::new();
    let iters = 200_000;
    println!("Allocation profile — {} iters/case", iters);
    println!(
        "  {:<40} {:>10} | {:>11} | {:>11} | {:>8}",
        "Case", "ns/op", "allocs/op", "frees/op", "bytes/op"
    );
    println!("  {}", "-".repeat(90));

    // ---- Cases that should not regress (unchanged dispatch path) ----
    measure(
        "const true",
        &engine,
        serde_json::json!(true),
        Value::Null,
        iters,
    );
    measure(
        "var: a",
        &engine,
        serde_json::json!({"var": "a"}),
        serde_json::json!({"a": 1}),
        iters,
    );
    measure(
        "+ (2 ints)",
        &engine,
        serde_json::json!({"+": [{"var":"a"},{"var":"b"}]}),
        serde_json::json!({"a":1,"b":2}),
        iters,
    );
    measure(
        "if/=== (true str branch)",
        &engine,
        serde_json::json!({"if": [{"===": [{"var":"x"}, "yes"]}, 1, 0]}),
        serde_json::json!({"x":"yes"}),
        iters,
    );

    // ---- Filter ALONE (POC target: ≥1.5× over baseline 638 ns) ----
    measure(
        "filter == on field, 10 [ARENA]",
        &engine,
        serde_json::json!({"filter": [{"var":"xs"}, {"===": [{"var":"k"}, 1]}]}),
        serde_json::json!({"xs": (1..=10).map(|i| serde_json::json!({"k": i % 2})).collect::<Vec<_>>()}),
        iters,
    );

    // ---- Length on raw data (sanity) ----
    measure(
        "length(xs) [ARENA]",
        &engine,
        serde_json::json!({"length": {"var":"xs"}}),
        serde_json::json!({"xs": (0..10).collect::<Vec<_>>()}),
        iters,
    );

    // ---- COMPOSITION test (POC target: ≥2.0× over baseline) ----
    // length(filter(...)) — filter result lives in arena, length reads slice len.
    measure(
        "length(filter(==)), 10 [ARENA]",
        &engine,
        serde_json::json!({"length": {"filter": [{"var":"xs"}, {"===": [{"var":"k"}, 1]}]}}),
        serde_json::json!({"xs": (1..=10).map(|i| serde_json::json!({"k": i % 2})).collect::<Vec<_>>()}),
        iters,
    );

    // Larger array to amplify the composition win
    measure(
        "length(filter(==)), 100 [ARENA]",
        &engine,
        serde_json::json!({"length": {"filter": [{"var":"xs"}, {"===": [{"var":"k"}, 1]}]}}),
        serde_json::json!({"xs": (1..=100).map(|i| serde_json::json!({"k": i % 2})).collect::<Vec<_>>()}),
        iters,
    );

    // ---- Phase 3 operators ----
    measure(
        "map +1, 10 ints",
        &engine,
        serde_json::json!({"map": [{"var":"xs"}, {"+": [{"var":""}, 1]}]}),
        serde_json::json!({"xs":[1,2,3,4,5,6,7,8,9,10]}),
        iters,
    );
    measure(
        "map field-extract, 10 objs [ARENA]",
        &engine,
        serde_json::json!({"map": [{"var":"xs"}, {"var":"k"}]}),
        serde_json::json!({"xs": (1..=10).map(|i| serde_json::json!({"k": i, "other": "noise"})).collect::<Vec<_>>()}),
        iters,
    );
    measure(
        "all >0, 10 ints [ARENA]",
        &engine,
        serde_json::json!({"all": [{"var":"xs"}, {">": [{"var":""}, 0]}]}),
        serde_json::json!({"xs":[1,2,3,4,5,6,7,8,9,10]}),
        iters,
    );
    measure(
        "some ==5, 10 ints [ARENA]",
        &engine,
        serde_json::json!({"some": [{"var":"xs"}, {"===": [{"var":""}, 5]}]}),
        serde_json::json!({"xs":[1,2,3,4,5,6,7,8,9,10]}),
        iters,
    );
    measure(
        "reduce sum, 10 ints [ARENA]",
        &engine,
        serde_json::json!({"reduce": [{"var":"xs"}, {"+":[{"var":"current"},{"var":"accumulator"}]}, 0]}),
        serde_json::json!({"xs":[1,2,3,4,5,6,7,8,9,10]}),
        iters,
    );

    // ---- Phase 3 COMPOSITION ----
    measure(
        "length(map(filter)), 100 [ARENA]",
        &engine,
        serde_json::json!({"length": {"map": [{"filter": [{"var":"xs"}, {"===": [{"var":"k"}, 1]}]}, {"var":"k"}]}}),
        serde_json::json!({"xs": (1..=100).map(|i| serde_json::json!({"k": i % 2, "noise": "x"})).collect::<Vec<_>>()}),
        iters,
    );
    measure(
        "all(filter()), 100 [ARENA]",
        &engine,
        serde_json::json!({"all": [{"filter": [{"var":"xs"}, {"===": [{"var":"k"}, 1]}]}, {">": [{"var":"k"}, 0]}]}),
        serde_json::json!({"xs": (1..=100).map(|i| serde_json::json!({"k": i % 2})).collect::<Vec<_>>()}),
        iters,
    );

    // ---- Phase 5 array-consumer ops ----
    measure(
        "max(xs) on 10 nums [ARENA]",
        &engine,
        serde_json::json!({"max": {"var":"xs"}}),
        serde_json::json!({"xs": [3, 1, 4, 1, 5, 9, 2, 6, 5, 3]}),
        iters,
    );
    measure(
        "+(xs) sum 10 nums [ARENA]",
        &engine,
        serde_json::json!({"+": {"var":"xs"}}),
        serde_json::json!({"xs": [1,2,3,4,5,6,7,8,9,10]}),
        iters,
    );
    // chained.json complex pattern: max(map(filter(people, eng), salary))
    measure(
        "max(map(filter())), 10 ppl [ARENA]",
        &engine,
        serde_json::json!({"max": {"map": [{"filter": [{"var":"people"}, {"===": [{"var":"dept"}, "eng"]}]}, {"var":"salary"}]}}),
        serde_json::json!({"people": [
            {"name":"a", "dept":"eng", "salary": 100},
            {"name":"b", "dept":"sales", "salary": 200},
            {"name":"c", "dept":"eng", "salary": 150},
            {"name":"d", "dept":"sales", "salary": 175},
            {"name":"e", "dept":"eng", "salary": 125},
            {"name":"f", "dept":"sales", "salary": 90},
            {"name":"g", "dept":"eng", "salary": 200},
            {"name":"h", "dept":"sales", "salary": 110},
            {"name":"i", "dept":"eng", "salary": 175},
            {"name":"j", "dept":"sales", "salary": 80}
        ]}),
        iters,
    );
    // sum-after-filter — common business pattern
    measure(
        "+(map(filter(), amt)), 10 [ARENA]",
        &engine,
        serde_json::json!({"+": {"map": [{"filter": [{"var":"items"}, {"===": [{"var":"k"}, 1]}]}, {"var":"amt"}]}}),
        serde_json::json!({"items": (1..=10).map(|i| serde_json::json!({"k": i % 2, "amt": i * 10})).collect::<Vec<_>>()}),
        iters,
    );

    // ---- Sanity: should not regress ----
    measure(
        "sort by field, 10",
        &engine,
        serde_json::json!({"sort": [{"var":"xs"}, true, {"var":"k"}]}),
        serde_json::json!({"xs": (1..=10).rev().map(|i| serde_json::json!({"k": i})).collect::<Vec<_>>()}),
        iters,
    );
}
