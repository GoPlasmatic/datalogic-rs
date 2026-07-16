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
    /// Compile an [`OwnedDataValue`] rule against `engine`. Honours the
    /// engine's [`crate::EngineBuilder::with_constant_folding`] flag —
    /// folding on (default) runs the optimizer + constant-fold passes;
    /// off skips them so every operator survives in the tree. Used by
    /// [`Engine::compile`].
    pub(crate) fn compile_with(logic: &OwnedDataValue, engine: &Engine) -> Result<Self> {
        let ctx = if engine.constant_folding_enabled() {
            CompileCtx::new()
        } else {
            CompileCtx::no_fold()
        };
        Self::compile_inner(logic, engine, ctx)
    }

    /// Compile with the optimizer + constant-fold passes disabled
    /// **regardless of the engine's setting** — every operator survives
    /// in the tree. Used internally by the trace one-shot path so traces
    /// have full operator coverage even when the engine has folding on.
    #[cfg(feature = "trace")]
    pub(crate) fn compile_for_trace(logic: &OwnedDataValue, engine: &Engine) -> Result<Self> {
        Self::compile_inner(logic, engine, CompileCtx::no_fold())
    }

    #[inline]
    fn compile_inner(logic: &OwnedDataValue, engine: &Engine, mut ctx: CompileCtx) -> Result<Self> {
        let mut root = walker::compile_node(
            logic,
            Some(engine),
            engine.is_templating_enabled(),
            &mut ctx,
        )?;
        // CSE runs once over the finished tree, after the per-node fixpoint
        // optimizer (folded shapes are final) and before `Logic::new`'s
        // populate pass (so hints are derived through the wrappers). Gated
        // like folding — traced/no-fold compiles produce zero `Cse` nodes —
        // and skipped under a `Custom` truthy evaluator, whose opaque
        // closure's call count would become observable through memoization.
        let cse_slot_count = if ctx.skip_fold()
            || matches!(
                engine.config().truthy_evaluator,
                crate::TruthyEvaluator::Custom(_)
            ) {
            0
        } else {
            optimize::cse::apply(&mut root)
        };
        Ok(Self::new(root, cse_slot_count))
    }
}
