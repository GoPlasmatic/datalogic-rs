//! v4 → v5 migration cheat sheet — runnable side by side.
//!
//! Each section pairs a v4-style call (kept compiling via the
//! [`compat::LegacyApi`] shim) with the v5-native equivalent. Both
//! produce the same result so you can verify the swap before deleting
//! the old code.
//!
//! Headline renames:
//!
//! - `DataLogic`        →  `Engine`
//! - `CompiledLogic`    →  `Logic`
//! - `Operator`         →  `CustomOperator`
//! - `evaluate_json`    →  `evaluate_str`
//! - `with_config(...)` →  `Engine::builder().config(...).build()`
//! - mutating `add_operator`  →  builder-only `add_operator`
//!
//! Run:
//!
//!     cargo run --example migrating_from_v4 --features compat

#![allow(deprecated)]

use bumpalo::Bump;
use datalogic_rs::compat::LegacyApi; // brings the v4 method names back into scope
use datalogic_rs::operator::ContextStack;
use datalogic_rs::{CustomOperator, DataValue, Engine, EvaluationConfig, NanHandling, Result};
use serde_json::json;

fn main() {
    // ============================================================
    // 1. Construct & configure
    // ============================================================
    let cfg = EvaluationConfig {
        arithmetic_nan_handling: NanHandling::IgnoreValue,
        ..Default::default()
    };

    // v4 — LegacyApi associated fn (deprecated):
    let v4 = Engine::with_config(cfg.clone());
    // v5 — fluent builder:
    let v5 = Engine::builder().config(cfg).build();

    let rule = r#"{"+": [1, "skipped", 2]}"#;
    println!("[1] v4: {}", v4.evaluate_json(rule, r#"{}"#).unwrap());
    println!("    v5: {}", v5.evaluate_str(rule, r#"{}"#).unwrap());

    // ============================================================
    // 2. One-shot evaluation
    // ============================================================
    let engine = Engine::new();
    let rule = r#"{"+": [{"var": "a"}, {"var": "b"}]}"#;
    let data = r#"{"a": 2, "b": 3}"#;

    // v4 — serde_json::Value boundary:
    let r_v4 = engine.evaluate_json(rule, data).unwrap();
    // v5 — string boundary (no serde_json needed):
    let r_v5_str = engine.evaluate_str(rule, data).unwrap();
    // v5 — serde_json boundary, when you need it (compat feature):
    let r_v5_serde = engine
        .evaluate_serde(
            &json!({"+": [{"var": "a"}, {"var": "b"}]}),
            &json!({"a": 2, "b": 3}),
        )
        .unwrap();

    println!("\n[2] v4 evaluate_json:    {r_v4}");
    println!("    v5 evaluate_str:     {r_v5_str}");
    println!("    v5 evaluate_serde:   {r_v5_serde}");

    // ============================================================
    // 3. Custom operators
    // ============================================================
    // v4: trait `Operator` with unevaluated args + `evaluator.evaluate(args[i], ctx)`.
    // v5: trait `CustomOperator` with PRE-evaluated `&DataValue<'a>` args.
    //     Registration moved to the builder — `add_operator` on a constructed
    //     `Engine` is gone.
    let engine = Engine::builder().add_operator("double", Double).build();
    let r = engine.evaluate_str(r#"{"double": 21}"#, r#"{}"#).unwrap();
    println!("\n[3] custom op double(21) -> {r}");
}

struct Double;

impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}
