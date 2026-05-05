//! Backward-compatibility surface for v4.x callers.
//!
//! Every legacy entry point lives on the [`LegacyApi`] trait. Bringing it into
//! scope (`use datalogic_rs::compat::LegacyApi;`) is the one-line migration
//! marker — the import itself signals "this file uses the deprecated API",
//! and grepping for it tells you exactly which files to revisit before
//! upgrading. All trait methods are individually `#[deprecated]` so the
//! compiler keeps reminding you per call site.
//!
//! Every method is implemented in terms of the v5 surface (`compile`,
//! `evaluate`, `evaluate_str`, `evaluate_serde`, `with_trace`) — there is
//! no separate code path. The trait is purely a thin ergonomic shim that
//! lets 4.x callers keep compiling.
//!
//! What lives here:
//! - **Renamed types** — `ArenaValue`, `ArenaContextStack`, `ArenaOperator`
//!   re-exported as deprecated aliases for `DataValue`, `ContextStack`,
//!   `Operator`.
//! - **Constructors** — `with_preserve_structure`, `with_config`,
//!   `with_config_and_structure`.
//! - **Compile entries** — `compile(&Value)`, `compile_serde_value(&Value)`.
//! - **Evaluate entries** — `evaluate(Arc<Value>)`, `evaluate_arc_value`,
//!   `evaluate_ref(&Value)`, `evaluate_owned(Value)`, `evaluate_json(&str)`,
//!   `evaluate_structured`, `evaluate_json_structured`,
//!   `evaluate_json_with_trace`, `evaluate_json_with_trace_structured`.
//! - **Operator registration** — no shim. v5 makes operator registration
//!   builder-only; callers must use
//!   `Engine::builder().add_operator(name, op).build()`.
//!
//! Compile-internal types (`CompiledNode`, `OpCode`, `MetadataHint`,
//! `PathSegment`, `ReduceHint`) were public in 4.x but are intentionally not
//! re-exported here. Any code that walked the compiled tree was already
//! broken by the arena rewrite — there is no v5-compatible shim for it.
//!
//! The module is gated by the `compat` feature, which is on by default. Drop
//! it (`default-features = false`) for a fully serde_json-free build.
//!
//! ```ignore
//! // 4.x:
//! use datalogic_rs::{ArenaValue, ArenaOperator};
//! engine.evaluate_json(rule, data)?;
//!
//! // Migration target:
//! use datalogic_rs::{DataValue, Operator};
//! engine.evaluate_str(rule, data)?;
//! // Or, to keep the 4.x signatures briefly:
//! use datalogic_rs::compat::LegacyApi;
//! engine.evaluate_json(rule, data)?;
//! ```
#![allow(deprecated)]

use std::sync::Arc;

use serde_json::Value;

use crate::{Engine, Error, EvaluationConfig, Logic, Result};

#[cfg(feature = "trace")]
use crate::trace::TracedResult;

// ---- Type / trait aliases ------------------------------------------------

// `CompiledNode`, `OpCode`, `MetadataHint`, `PathSegment`, `ReduceHint` were
// public in 4.x but are compile-internal in v5. They are not re-exported here:
// any 4.x code that walked the compiled tree was already broken by the arena
// rewrite, so keeping the names compiling adds no real migration value.

/// Deprecated alias for [`crate::DataValue`].
#[deprecated(
    since = "5.0.0",
    note = "use `DataValue`; the `compat` module will be removed in 5.1"
)]
pub type ArenaValue<'a> = crate::DataValue<'a>;

/// Deprecated alias for [`crate::ContextStack`].
#[deprecated(
    since = "5.0.0",
    note = "use `ContextStack`; the `compat` module will be removed in 5.1"
)]
pub type ArenaContextStack<'a> = crate::ContextStack<'a>;

/// Deprecated alias for [`crate::Operator`]. The trait method
/// `evaluate_arena` was renamed to `evaluate` — old impls that used the
/// `evaluate_arena` name need a one-line rename.
#[deprecated(
    since = "5.0.0",
    note = "use `Operator` and rename `evaluate_arena` -> `evaluate`; the `compat` module will be removed in 5.1"
)]
pub trait ArenaOperator: Send + Sync {
    /// Forward to the v5 [`crate::Operator::evaluate`] signature.
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a crate::DataValue<'a>],
        ctx: &mut crate::ContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::DataValue<'a>>;
}

// Bridge: every ArenaOperator IS-A Operator (forward `evaluate_arena`
// → `evaluate`). Lets old custom operators keep compiling.
impl<T: ArenaOperator + ?Sized> crate::Operator for T {
    #[inline]
    fn evaluate<'a>(
        &self,
        args: &[&'a crate::DataValue<'a>],
        ctx: &mut crate::ContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::DataValue<'a>> {
        ArenaOperator::evaluate_arena(self, args, ctx, arena)
    }
}

// ---- LegacyApi extension trait ------------------------------------------
//
// One-stop shop for every deprecated entry point that used to live on the
// inherent `impl Engine`. Bringing this trait into scope unlocks the 4.x
// surface; not bringing it in scope keeps the v5 inherent API clean.

/// Deprecated 4.x methods on [`Engine`]. Bring this trait into scope to
/// access the legacy surface; remove the import to discover what you need to
/// migrate. Every method is `#[deprecated]` and slated for removal in 5.1.
///
/// Each method's `note` field documents the v5 replacement. Common patterns:
///
/// | Old (4.x)                                                 | New (v5)                                            |
/// |-----------------------------------------------------------|-----------------------------------------------------|
/// | `engine.evaluate_json(logic, data)?`                      | `engine.evaluate_str(logic, data)?`                 |
/// | `engine.evaluate_owned(&compiled, value)?`                | `engine.evaluate_serde(&logic, &value)?`            |
/// | `engine.evaluate_json_with_trace(logic, data)?`           | `engine.with_trace().evaluate_str(logic, data)`     |
/// | `engine.evaluate_json_structured(logic, data)?`           | `engine.evaluate_str(logic, data)?` (Error is structured) |
/// | `Engine::with_config(cfg)`                             | `Engine::builder().config(cfg).build()`          |
pub trait LegacyApi: Sized {
    // ---- Constructors ----

    /// Deprecated: use `Engine::builder().preserve_structure(true).build()`.
    #[deprecated(
        since = "5.0.0",
        note = "use `Engine::builder().preserve_structure(true).build()`"
    )]
    #[cfg(feature = "preserve")]
    fn with_preserve_structure() -> Self;

    /// Deprecated: use `Engine::builder().config(...).build()`.
    #[deprecated(since = "5.0.0", note = "use `Engine::builder().config(...).build()`")]
    fn with_config(config: EvaluationConfig) -> Self;

    /// Deprecated: use `Engine::builder().config(...).preserve_structure(...).build()`.
    #[deprecated(
        since = "5.0.0",
        note = "use `Engine::builder().config(...).preserve_structure(...).build()`"
    )]
    #[cfg(feature = "preserve")]
    fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self;

    // ---- Compile entries ----

    /// Deprecated: use `Engine::compile(&str)` for the v5 entry, or
    /// `compile_serde_value(&Value)` for the direct serde_json boundary.
    #[deprecated(
        since = "5.0.0",
        note = "use `Engine::compile(&str)` or `compile_serde_value(&Value)`"
    )]
    fn compile(&self, logic: &Value) -> Result<Arc<Logic>>;

    /// Deprecated: use `Engine::compile(&str)` (parses to v5 types) or
    /// `evaluate_serde(&Value, &Value)` for one-shot evaluation.
    #[deprecated(
        since = "5.0.0",
        note = "use `Engine::compile(&str)` or `evaluate_serde(&Value, &Value)` for one-shot"
    )]
    fn compile_serde_value(&self, logic: &Value) -> Result<Arc<Logic>>;

    // ---- Evaluate entries ----

    /// Deprecated: use `Engine::evaluate(&Logic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&Logic, &DataValue, &Bump)` or `evaluate_serde(&Value, &Value)`"
    )]
    fn evaluate(&self, compiled: &Logic, data: Arc<Value>) -> Result<Value>;

    /// Deprecated: use `Engine::evaluate(&Logic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&Logic, &DataValue, &Bump)` or `evaluate_serde(&Value, &Value)`"
    )]
    fn evaluate_arc_value(&self, compiled: &Logic, data: Arc<Value>) -> Result<Value>;

    /// Deprecated: use `Engine::evaluate(&Logic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&Logic, &DataValue, &Bump)` or `evaluate_serde(&Value, &Value)`"
    )]
    fn evaluate_ref(&self, compiled: &Logic, data: &Value) -> Result<Value>;

    /// Deprecated: use `Engine::evaluate(&Logic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&Logic, &DataValue, &Bump)` or `evaluate_serde(&Value, &Value)`"
    )]
    fn evaluate_owned(&self, compiled: &Logic, data: Value) -> Result<Value>;

    /// Deprecated: use `Engine::evaluate_str(&str, &str)` (returns
    /// `String`) or `Engine::evaluate_serde(&Value, &Value)` (returns
    /// `Value`).
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_str(&str, &str)` or `evaluate_serde(&Value, &Value)`"
    )]
    fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value>;

    /// Deprecated: today every error returned by `evaluate*` already carries
    /// `operator`/`path` — call `evaluate_serde` and read `Error` directly.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_serde` — `Error` now carries `operator`/`path` directly"
    )]
    fn evaluate_structured(
        &self,
        compiled: &Logic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, Error>;

    /// Deprecated: today every error returned by `evaluate_str` already
    /// carries `operator`/`path`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_str` / `evaluate_serde` — `Error` now carries `operator`/`path` directly"
    )]
    fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, Error>;

    // ---- Trace entries ----

    /// Deprecated: use [`crate::Engine::with_trace`] +
    /// [`crate::TracedSession::evaluate_str`].
    #[cfg(feature = "trace")]
    #[deprecated(
        since = "5.0.0",
        note = "use `engine.with_trace().evaluate_str(logic, data)` (returns TracedRun)"
    )]
    fn evaluate_json_with_trace(&self, logic: &str, data: &str) -> Result<TracedResult>;

    /// Deprecated: use [`crate::Engine::with_trace`] +
    /// [`crate::TracedSession::evaluate_str`] — `TracedRun.result` already
    /// carries the merged structured `Error` on failure.
    #[cfg(feature = "trace")]
    #[deprecated(
        since = "5.0.0",
        note = "use `engine.with_trace().evaluate_str(logic, data)`"
    )]
    fn evaluate_json_with_trace_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<TracedResult, Error>;
}

impl LegacyApi for Engine {
    // ---- Constructors ----

    #[cfg(feature = "preserve")]
    fn with_preserve_structure() -> Self {
        Engine::builder().preserve_structure(true).build()
    }

    fn with_config(config: EvaluationConfig) -> Self {
        Engine::builder().config(config).build()
    }

    #[cfg(feature = "preserve")]
    fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self {
        Engine::builder()
            .config(config)
            .preserve_structure(preserve_structure)
            .build()
    }

    // ---- Compile entries ----

    fn compile(&self, logic: &Value) -> Result<Arc<Logic>> {
        LegacyApi::compile_serde_value(self, logic)
    }

    fn compile_serde_value(&self, logic: &Value) -> Result<Arc<Logic>> {
        let owned = crate::value::owned_from_serde(logic);
        Ok(Arc::new(Logic::compile_with(&owned, self)?))
    }

    // ---- Evaluate entries ----

    fn evaluate(&self, compiled: &Logic, data: Arc<Value>) -> Result<Value> {
        self.run_to_value(compiled, &data)
    }

    fn evaluate_arc_value(&self, compiled: &Logic, data: Arc<Value>) -> Result<Value> {
        self.run_to_value(compiled, &data)
    }

    fn evaluate_ref(&self, compiled: &Logic, data: &Value) -> Result<Value> {
        self.run_to_value(compiled, data)
    }

    fn evaluate_owned(&self, compiled: &Logic, data: Value) -> Result<Value> {
        self.run_to_value(compiled, &data)
    }

    fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        self.evaluate_serde(&logic_value, &data_value)
    }

    fn evaluate_structured(
        &self,
        compiled: &Logic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, Error> {
        // Pre-merge this had a separate code path. Today every public
        // `evaluate*` already populates operator+path on failure, so this
        // is just `run_to_value` — same shape, error already carries the
        // structured fields.
        self.run_to_value(compiled, &data)
    }

    fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, Error> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        self.evaluate_serde(&logic_value, &data_value)
    }

    // ---- Trace entries ----

    #[cfg(feature = "trace")]
    fn evaluate_json_with_trace(&self, logic: &str, data: &str) -> Result<TracedResult> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);
        let owned = crate::value::owned_from_serde(&logic_value);
        let compiled = Arc::new(crate::Logic::compile_for_trace(&owned, self)?);
        Ok(self.run_trace(&compiled, data_arc))
    }

    #[cfg(feature = "trace")]
    fn evaluate_json_with_trace_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<TracedResult, Error> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        let data_arc = Arc::new(data_value);
        let owned = crate::value::owned_from_serde(&logic_value);
        let compiled = Arc::new(crate::Logic::compile_for_trace(&owned, self)?);
        Ok(self.run_trace(&compiled, data_arc))
    }
}
