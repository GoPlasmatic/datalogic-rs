//! Compile-phase: walk a `serde_json::Value` rule tree into the engine's
//! [`CompiledNode`] representation, with operator-specific specialisations
//! and (when an engine is available) optimisation + constant-folding passes.
//!
//! The entry points live here; the heavy lifting is split across
//! - [`node`] — the recursive `compile_node` dispatch.
//! - [`operator`] — `var` / `val` / `exists` / split-regex specialisations.
//! - [`missing`] — `missing` / `missing_some` static path pre-parsing.
//! - [`path`] — shared dot-path parsing.
//! - [`optimize`] — DCE, strength reduction, constant folding.

pub mod optimize;

mod missing;
mod node;
mod operator;
mod path;

use serde_json::Value;

use crate::node::{CompileCtx, CompiledLogic};
use crate::{DataLogic, Result};

impl CompiledLogic {
    /// Compiles a JSON value into a compiled logic structure.
    ///
    /// Performs basic compilation without static evaluation. For optimal
    /// runtime performance, prefer [`Self::compile_with_static_eval`] which
    /// also folds constant subtrees.
    pub fn compile(logic: &Value) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = node::compile_node(logic, None, false, &mut ctx)?;
        Ok(Self::new(root))
    }

    /// Compiles for tracing without static evaluation — keeps every operator
    /// node so the trace collector can step through each evaluation.
    #[cfg(feature = "trace")]
    pub fn compile_for_trace(logic: &Value, preserve_structure: bool) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = node::compile_node(logic, None, preserve_structure, &mut ctx)?;
        Ok(Self::new(root))
    }

    /// Compiles with static evaluation using the provided engine — runs
    /// optimisation and constant-folding passes during compilation.
    pub fn compile_with_static_eval(logic: &Value, engine: &DataLogic) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = node::compile_node(logic, Some(engine), engine.preserve_structure(), &mut ctx)?;
        Ok(Self::new(root))
    }
}
