//! Optimization passes for compiled logic trees.
//!
//! Each pass is a pure function that transforms a `CompiledNode` tree,
//! producing an equivalent but more efficient tree. Passes are composable
//! and independently testable.
//!
//! # Adding a new optimization
//!
//! 1. Create a new file in this directory (e.g., `my_pass.rs`)
//! 2. Implement a `pub fn optimize(node: CompiledNode, ...) -> CompiledNode`
//! 3. Add `OptimizedNode` variants as needed in the `node.rs` `OptimizedNode` enum
//! 4. Call the pass from `optimize()` below
//! 5. Run `cargo test` — no changes needed in engine.rs or trace.rs

pub mod constant_fold;
pub mod dead_code;
mod helpers;
pub mod strength;

use crate::DataLogic;
use crate::node::CompiledNode;

/// Run all optimization passes on a compiled node tree.
///
/// This is the main entry point for the optimization pipeline.
/// Called from `compile_node` when an engine is provided (i.e., not in trace mode).
///
/// Passes are applied in order:
/// 1. Dead code elimination (remove unreachable branches)
/// 2. Constant folding (fold static args in commutative ops, pre-coerce numeric strings)
/// 3. Arity specialization (binary arithmetic, comparison-with-literal)
/// 4. Strength reduction (double negation collapse, etc.)
pub fn optimize(node: CompiledNode, engine: &DataLogic) -> CompiledNode {
    let node = dead_code::eliminate(node, engine);
    let node = constant_fold::fold(node, engine);
    strength::reduce(node)
}
