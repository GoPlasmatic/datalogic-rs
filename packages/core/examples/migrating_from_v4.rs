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
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, EvaluationConfig, NanHandling, Result};
use serde_json::json;

fn main() {
    // ============================================================
    // 1. Construct & configure
    // ============================================================
    let cfg = EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue);

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

    // ============================================================
    // 4. The v4 `preserve` operator → `preserve_structure` mode
    // ============================================================
    // v4: a `{"preserve": {...}}` operator that returned its arg
    //     unchanged. Used as a way to embed object literals in rules
    //     (so the engine wouldn't try to parse the keys as operators).
    //
    // v5: that operator is removed. Literal scalars and arrays pass
    //     through inline already. Multi-key objects are an error by
    //     default — opt into the templating behaviour at the engine
    //     level via `Engine::builder().preserve_structure(true)` and
    //     drop the `preserve` wrapper from your rules.
    //
    // Equivalent rules:
    //
    //   v4: { "preserve": { "name": "Jane", "age": 25 } }
    //   v5: enable `preserve_structure(true)` and use the object
    //       directly: { "name": "Jane", "age": 25 }
    //
    // Operator values inside that object are still computed, so
    // templating with mixed literal/computed fields works:
    //
    //   { "name": "Jane", "isAdult": { ">=": [{ "var": "age" }, 18] } }
    //
    // See `examples/structured_objects.rs` and the proposal's D8 for
    // the full walkthrough.
    let engine = Engine::builder().preserve_structure(true).build();
    let templated = engine
        .evaluate_str(
            r#"{"name": "Jane", "isAdult": {">=": [{"var": "age"}, 18]}}"#,
            r#"{"age": 25}"#,
        )
        .unwrap();
    println!("\n[4] v5 preserve_structure -> {templated}");

    // ============================================================
    // 5. Configuration: struct literals → fluent setters
    // ============================================================
    // v5 marks `EvaluationConfig` and `NumericCoercionConfig` as
    // `#[non_exhaustive]` so future field adds in 5.x stay non-breaking.
    // External struct-expression construction is rejected, including
    // `..Default::default()`. Two paths work for callers:
    //
    //   (a) `Config::default()` + chained `with_*` setters (preferred —
    //       reads cleanly when only a few fields differ from defaults).
    //   (b) `Config::default()` + direct field mutation (fields stay
    //       `pub`, so `cfg.foo = bar` continues to compile).
    //
    // The setter on `EvaluationConfig` was also renamed in v5:
    // `with_nan_handling` → `with_arithmetic_nan_handling` so the
    // method name matches the field name. Same rename pattern across
    // all the new `with_*` setters.
    //
    // v4-style (broken in v5):
    //   let cfg = EvaluationConfig {
    //       arithmetic_nan_handling: NanHandling::IgnoreValue,
    //       ..Default::default()
    //   };
    //
    // v5 — fluent:
    let cfg_fluent =
        EvaluationConfig::default().with_arithmetic_nan_handling(NanHandling::IgnoreValue);
    let _engine = Engine::builder().config(cfg_fluent).build();

    // v5 — direct mutation (fields stay pub):
    let mut cfg_mut = EvaluationConfig::default();
    cfg_mut.arithmetic_nan_handling = NanHandling::IgnoreValue;
    let _engine = Engine::builder().config(cfg_mut).build();

    println!("\n[5] config: fluent + direct-mutation paths both work");
}

struct Double;

impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}
