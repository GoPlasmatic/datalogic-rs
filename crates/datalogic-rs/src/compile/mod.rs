//! Compile-phase: walk an [`OwnedDataValue`] rule tree into the engine's
//! [`CompiledNode`] representation, with operator-specific specialisations
//! and (when an engine is available) optimisation + constant-folding passes.
//!
//! The entry points live here; the heavy lifting is split across
//! - [`walker`] — the recursive `compile_node` dispatch.
//! - [`hooks`] — the compile hooks operator table rows declare.
//! - [`operator`] — `var` / `val` / `exists` specialisations.
//! - [`missing`] — `missing` / `missing_some` static path pre-parsing.
//! - [`path_segments`] — shared dot-path parsing.
//! - [`scope`] — the static frame model (which child positions run under a
//!   pushed context frame), shared by scope resolution and the CSE pass.
//! - [`optimize`] — DCE, strength reduction, constant folding.

mod optimize;

pub(crate) mod hooks;
mod missing;
mod operator;
mod path_segments;
pub(crate) mod scope;
mod walker;

pub(crate) use path_segments::parse_path_segments;

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
        Self::compile_in_mode(logic, engine, engine.is_templating_enabled())
    }

    /// [`Self::compile_with`] with the templating mode given by the caller
    /// instead of read from the engine. Backs [`Engine::compile_template`]
    /// and [`Engine::compile_strict`]; everything else (folding, escape,
    /// custom operators) still comes from `engine`.
    pub(crate) fn compile_in_mode(
        logic: &OwnedDataValue,
        engine: &Engine,
        templating: bool,
    ) -> Result<Self> {
        let ctx = if engine.constant_folding_enabled() {
            CompileCtx::new()
        } else {
            CompileCtx::no_fold()
        };
        Self::compile_inner(logic, engine, templating, ctx)
    }

    /// Compile with the optimizer + constant-fold passes disabled
    /// **regardless of the engine's setting** — every operator survives
    /// in the tree. Used internally by the trace one-shot path so traces
    /// have full operator coverage even when the engine has folding on.
    /// `place` records each node's pointer ([`Logic::pointer`]), for a
    /// `Logic` the caller keeps; a one-shot run drops it unread.
    #[cfg(feature = "trace")]
    /// `templating` is the mode to compile in, as for
    /// [`Self::compile_in_mode`].
    pub(crate) fn compile_for_trace(
        logic: &OwnedDataValue,
        engine: &Engine,
        templating: bool,
        place: bool,
    ) -> Result<Self> {
        let ctx = CompileCtx::no_fold();
        let ctx = if place { ctx.recording_pointers() } else { ctx };
        Self::compile_inner(logic, engine, templating, ctx)
    }

    #[inline]
    fn compile_inner(
        logic: &OwnedDataValue,
        engine: &Engine,
        templating: bool,
        mut ctx: CompileCtx,
    ) -> Result<Self> {
        let mut root = walker::compile_node(logic, Some(engine), templating, &mut ctx)?;
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
        // Static scope annotation. Runs after CSE (so wrappers are in place
        // and get walked transparently) and before `Logic::new`'s populate
        // pass. Unconditional, unlike folding and CSE: the runtime reads the
        // annotation, so the traced / no-fold path needs it too.
        let needs_ancestor_frames = scope::resolve(&mut root);
        let mut logic = Self::new(root, cse_slot_count, needs_ancestor_frames);
        logic.engine_id = engine.id();
        logic.pointers = ctx.take_pointers();
        Ok(logic)
    }
}
