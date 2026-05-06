//! Compile-phase: walk an [`OwnedDataValue`] rule tree into the engine's
//! [`CompiledNode`] representation, with operator-specific specialisations
//! and (when an engine is available) optimisation + constant-folding passes.
//!
//! The entry points live here; the heavy lifting is split across
//! - [`walker`] — the recursive `compile_node` dispatch.
//! - [`operator`] — `var` / `val` / `exists` specialisations.
//! - [`missing`] — `missing` / `missing_some` static path pre-parsing.
//! - [`path_segments`] — shared dot-path parsing.
//! - [`optimize`] — DCE, strength reduction, constant folding.

mod optimize;

mod missing;
mod operator;
mod path_segments;
mod walker;

use datavalue::OwnedDataValue;

use crate::node::{CompileCtx, Logic};
use crate::{Engine, Result};

impl Logic {
    /// Compile an [`OwnedDataValue`] rule against `engine`, running the
    /// optimizer + constant-fold passes. Used by [`Engine::compile`].
    pub(crate) fn compile_with(logic: &OwnedDataValue, engine: &Engine) -> Result<Self> {
        Self::compile_inner(logic, engine, CompileCtx::new())
    }

    /// Compile with the optimizer + constant-fold passes disabled — every
    /// operator survives in the tree. Used internally by the trace one-shot
    /// path so traces have full operator coverage.
    #[cfg(feature = "trace")]
    pub(crate) fn compile_for_trace(logic: &OwnedDataValue, engine: &Engine) -> Result<Self> {
        Self::compile_inner(logic, engine, CompileCtx::no_fold())
    }

    #[inline]
    fn compile_inner(logic: &OwnedDataValue, engine: &Engine, mut ctx: CompileCtx) -> Result<Self> {
        let root =
            walker::compile_node(logic, Some(engine), engine.preserve_structure(), &mut ctx)?;
        Ok(Self::new(root))
    }
}
