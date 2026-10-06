//! Read projection (spike S1 in proposal-v6.md): what one evaluation costs
//! when a rule reads a few fields of a large owned or `serde_json`
//! context, with and without projection.
//!
//! "projected" evaluates on the engine that compiled the rule, which views
//! only the paths the rule reads. "whole" evaluates the same compiled rule
//! on a second engine built the same way, which views the whole input (a
//! rule from another engine is never projected), so both columns run the
//! same code apart from the view.
//!
//! ```bash
//! cargo run --release -p datalogic-bench --bin projection
//! ```

use std::hint::black_box;

use bumpalo::Bump;
use datalogic_bench::measure;
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{Engine, EvalInput, Logic};
use serde_json::{Value, json};

/// A context of about `bytes` of JSON: many small records, plus the
/// fields the rules read.
fn context(bytes: usize) -> Value {
    let record = json!({"id": 1, "name": "item", "tags": ["a", "b"], "price": 9.5, "meta": {"x": 1, "y": "z"}});
    let per = record.to_string().len() + 12;
    let mut items = serde_json::Map::new();
    for i in 0..(bytes / per).max(1) {
        items.insert(format!("r{i}"), record.clone());
    }
    json!({
        "records": items,
        "user": {"id": 7, "name": "ada", "tier": "gold"},
        "order": {"total": 120, "lines": [{"qty": 2, "price": 10}, {"qty": 1, "price": 100}]},
    })
}

const RULES: &[(&str, &str)] = &[
    ("one field", r#"{"var": "user.name"}"#),
    (
        "three fields",
        r#"{"and": [{"==": [{"var": "user.tier"}, "gold"]}, {">": [{"var": "order.total"}, 100]}, {"!=": [{"var": "user.id"}, 0]}]}"#,
    ),
    (
        "small array",
        r#"{"reduce": [{"var": "order.lines"}, {"+": [{"var": "accumulator"}, {"*": [{"var": "current.qty"}, {"var": "current.price"}]}]}, 0]}"#,
    ),
];

/// Median ns per call of `eval` on a freshly reset arena, timed by the
/// shared harness ([`measure`]).
fn time(arena: &mut Bump, mut eval: impl FnMut(&Bump) -> u64) -> f64 {
    measure(|n| {
        let mut sink = 0u64;
        for _ in 0..n {
            arena.reset();
            sink = sink.wrapping_add(eval(arena));
        }
        sink
    })
}

/// Evaluate `logic` on `engine` over `data`, as a sink value for [`time`].
fn eval<'a>(engine: &Engine, logic: &'a Logic, data: impl EvalInput<'a>, arena: &'a Bump) -> u64 {
    black_box(engine.evaluate(logic, data, arena).unwrap()) as *const _ as u64
}

fn human(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{ns:.0} ns")
    }
}

fn main() {
    let compiling = Engine::new();
    let other = Engine::new();
    println!(
        "{:<10} {:<14} {:<7} {:>12} {:>12} {:>9}",
        "context", "rule", "input", "projected", "whole", "speedup"
    );
    for (label, bytes) in [("1 KB", 1_000), ("100 KB", 100_000), ("8 MB", 8_000_000)] {
        let serde_ctx = context(bytes);
        let owned_ctx = OwnedDataValue::from_json(&serde_ctx.to_string()).unwrap();
        for (name, rule) in RULES {
            let logic = compiling.compile(*rule).unwrap();
            let mut arena = Bump::with_capacity(1 << 16);
            // Both columns must agree.
            let a = compiling
                .evaluate(&logic, &owned_ctx, &arena)
                .unwrap()
                .to_string();
            let b = other
                .evaluate(&logic, &owned_ctx, &arena)
                .unwrap()
                .to_string();
            assert_eq!(a, b, "{name}");
            let p = time(&mut arena, |a| eval(&compiling, &logic, &owned_ctx, a));
            let w = time(&mut arena, |a| eval(&other, &logic, &owned_ctx, a));
            println!(
                "{label:<10} {name:<14} {:<7} {:>12} {:>12} {:>8.1}x",
                "owned",
                human(p),
                human(w),
                w / p
            );
            let p = time(&mut arena, |a| eval(&compiling, &logic, &serde_ctx, a));
            let w = time(&mut arena, |a| eval(&other, &logic, &serde_ctx, a));
            println!(
                "{label:<10} {name:<14} {:<7} {:>12} {:>12} {:>8.1}x",
                "serde",
                human(p),
                human(w),
                w / p
            );
        }
    }
}
