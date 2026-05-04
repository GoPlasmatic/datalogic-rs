//! Backward-compatibility surface for v4.x callers.
//!
//! Everything in this module carries `#[deprecated(since = "5.0.0", note =
//! "use the v5 API; the `compat` module will be removed in 5.1")]` so that
//! downstream code keeps compiling while emitting migration warnings.
//!
//! What lives here:
//! - **Renamed types** — `ArenaValue`, `ArenaContextStack`, `ArenaOperator`
//!   re-exported as deprecated aliases for `DataValue`, `DataContextStack`,
//!   `DataOperator`.
//! - **Old constructors / eval methods** — `with_preserve_structure`,
//!   `with_config`, `evaluate_owned`, `evaluate_ref`, `evaluate`, etc, exposed
//!   as a [`DataLogicLegacyExt`] trait that bridges through the v5 API.
//!
//! The module is gated by the `compat` feature, which is on by default.
//! Drop it (`default-features = false`) for a fully serde_json-free build.
//!
//! ```ignore
//! // Old code (4.x):
//! use datalogic_rs::{ArenaValue, ArenaOperator};
//!
//! // Migration target:
//! use datalogic_rs::{DataValue, DataOperator};
//! ```
#![allow(deprecated)]

use std::sync::Arc;

use serde_json::Value;

use crate::{CompiledLogic, DataLogic, EvaluationConfig, Result, StructuredError};

// ---- Type / trait aliases ------------------------------------------------

/// Deprecated re-exports of compile-internal types that were public in 4.x.
/// These are not part of the v5 public surface — they're surfaced here only
/// so existing `crate::CompiledNode` / `crate::OpCode` / etc. paths keep
/// resolving for downstream code that mid-tree-inspected the rule.
#[deprecated(
    since = "5.0.0",
    note = "compile-internal types are no longer part of the public API; the `compat` module will be removed in 5.1"
)]
pub use crate::node::{CompiledNode, MetadataHint, PathSegment, ReduceHint};
#[deprecated(
    since = "5.0.0",
    note = "compile-internal types are no longer part of the public API; the `compat` module will be removed in 5.1"
)]
pub use crate::opcode::OpCode;

/// Deprecated alias for [`crate::DataValue`].
#[deprecated(
    since = "5.0.0",
    note = "use `DataValue`; the `compat` module will be removed in 5.1"
)]
pub type ArenaValue<'a> = crate::DataValue<'a>;

/// Deprecated alias for [`crate::DataContextStack`].
#[deprecated(
    since = "5.0.0",
    note = "use `DataContextStack`; the `compat` module will be removed in 5.1"
)]
pub type ArenaContextStack<'a> = crate::DataContextStack<'a>;

/// Deprecated alias for [`crate::DataOperator`]. The trait method
/// `evaluate_arena` was renamed to `evaluate` — old impls that used the
/// `evaluate_arena` name need a one-line rename.
#[deprecated(
    since = "5.0.0",
    note = "use `DataOperator` and rename `evaluate_arena` -> `evaluate`; the `compat` module will be removed in 5.1"
)]
pub trait ArenaOperator: Send + Sync {
    /// Forward to the v5 [`crate::DataOperator::evaluate`] signature.
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a crate::DataValue<'a>],
        actx: &mut crate::DataContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::DataValue<'a>>;
}

// Bridge: every ArenaOperator IS-A DataOperator (forward `evaluate_arena`
// → `evaluate`). Lets old custom operators keep compiling.
impl<T: ArenaOperator + ?Sized> crate::DataOperator for T {
    #[inline]
    fn evaluate<'a>(
        &self,
        args: &[&'a crate::DataValue<'a>],
        actx: &mut crate::DataContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::DataValue<'a>> {
        ArenaOperator::evaluate_arena(self, args, actx, arena)
    }
}

// ---- DataLogic legacy extension trait ------------------------------------

/// Deprecated 4.x methods on [`DataLogic`] that were dropped in v5 in favour
/// of [`DataLogic::builder`] and the collapsed `evaluate*` API. All shims
/// here are `#[deprecated]` and will be removed in 5.1.
///
/// `serde_json::Value` is the boundary type for every shim — internally each
/// method bridges through `OwnedDataValue` / `DataValue` (using
/// [`crate::value::owned_from_serde`] and [`crate::value::owned_to_serde`]).
pub trait DataLogicLegacyExt: Sized {
    // ---- Constructors ----

    /// Deprecated: use `DataLogic::builder().preserve_structure(true).build()`.
    #[deprecated(
        since = "5.0.0",
        note = "use `DataLogic::builder().preserve_structure(true).build()`"
    )]
    #[cfg(feature = "preserve")]
    fn with_preserve_structure() -> Self;

    /// Deprecated: use `DataLogic::builder().config(...).build()`.
    #[deprecated(
        since = "5.0.0",
        note = "use `DataLogic::builder().config(...).build()`"
    )]
    fn with_config(config: EvaluationConfig) -> Self;

    /// Deprecated: use `DataLogic::builder().config(...).preserve_structure(...).build()`.
    #[deprecated(
        since = "5.0.0",
        note = "use `DataLogic::builder().config(...).preserve_structure(...).build()`"
    )]
    #[cfg(feature = "preserve")]
    fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self;

    // ---- Compile entry ----

    /// Deprecated: use `DataLogic::compile(&str)` for the v5 entry, or
    /// `DataLogic::compile_serde_value(&Value)` for the direct serde_json
    /// boundary.
    #[deprecated(
        since = "5.0.0",
        note = "use `DataLogic::compile(&str)` or `compile_serde_value(&Value)`"
    )]
    fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>>;

    // ---- Evaluate methods ----

    /// Deprecated: use `DataLogic::evaluate(&CompiledLogic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value>;

    /// Deprecated: use `DataLogic::evaluate(&CompiledLogic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    fn evaluate_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value>;

    /// Deprecated: use `DataLogic::evaluate(&CompiledLogic, &DataValue, &Bump)`.
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value>;

    /// Deprecated: use `DataLogic::evaluate_str(&str, &str)` (returns
    /// `String`) or `DataLogic::evaluate_value(&Value, &Value)` (returns
    /// `Value`).
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_str(&str, &str)` or `evaluate_value(&Value, &Value)`"
    )]
    fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value>;

    /// Deprecated: a v5 structured-error API will land in 5.1.
    #[deprecated(
        since = "5.0.0",
        note = "the v5 structured-error API lands in 5.1; this method will be removed at that time"
    )]
    fn evaluate_structured(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, StructuredError>;

    /// Deprecated: a v5 structured-error API will land in 5.1.
    #[deprecated(
        since = "5.0.0",
        note = "the v5 structured-error API lands in 5.1; this method will be removed at that time"
    )]
    fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, StructuredError>;

    /// Deprecated: use `DataLogicBuilder::add_operator`.
    #[deprecated(
        since = "5.0.0",
        note = "use `DataLogic::builder().add_operator(name, op).build()`"
    )]
    fn add_arena_operator(&mut self, name: String, operator: Box<dyn crate::DataOperator>);
}

impl DataLogicLegacyExt for DataLogic {
    #[cfg(feature = "preserve")]
    fn with_preserve_structure() -> Self {
        DataLogic::builder().preserve_structure(true).build()
    }

    fn with_config(config: EvaluationConfig) -> Self {
        DataLogic::builder().config(config).build()
    }

    #[cfg(feature = "preserve")]
    fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self {
        DataLogic::builder()
            .config(config)
            .preserve_structure(preserve_structure)
            .build()
    }

    fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        DataLogic::compile_serde_value(self, logic)
    }

    fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        DataLogic::evaluate_arc_value(self, compiled, data)
    }

    fn evaluate_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        DataLogic::evaluate_ref(self, compiled, data)
    }

    fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        DataLogic::evaluate_owned(self, compiled, data)
    }

    fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        DataLogic::evaluate_json(self, logic, data)
    }

    fn evaluate_structured(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, StructuredError> {
        DataLogic::evaluate_structured(self, compiled, data)
    }

    fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, StructuredError> {
        DataLogic::evaluate_json_structured(self, logic, data)
    }

    fn add_arena_operator(&mut self, name: String, operator: Box<dyn crate::DataOperator>) {
        DataLogic::add_operator(self, name, operator)
    }
}
