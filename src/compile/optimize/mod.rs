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
//! 3. Call the pass from `optimize()` below
//! 4. Run `cargo test` — no changes needed in engine.rs or trace.rs

pub mod constant_fold;
pub mod dead_code;
mod helpers;
pub mod strength;

#[cfg(test)]
mod test_helpers;

use crate::DataLogic;
use crate::node::CompiledNode;

/// Maximum number of fixpoint iterations for the optimiser pipeline.
///
/// Three passes (dead code / constant fold / strength reduction) can feed each
/// other: folding exposes new dead branches; strength reduction can expose new
/// constants. A small cap is enough to catch the compounds we've seen in practice
/// (2–3 iterations) while bounding worst-case compile time.
const MAX_FIXPOINT_ITERATIONS: usize = 4;

/// Run all optimization passes on a compiled node tree until a fixpoint.
///
/// This is the main entry point for the optimization pipeline.
/// Called from `compile_node` when an engine is provided (i.e., not in trace mode).
///
/// Passes are applied in order until no pass reports a change or
/// [`MAX_FIXPOINT_ITERATIONS`] is reached:
/// 1. Dead code elimination (remove unreachable branches)
/// 2. Constant folding (fold static args in commutative ops, pre-coerce numeric strings)
/// 3. Strength reduction (double negation collapse, etc.)
///
/// Each pass returns `(node, changed)`; the loop exits as soon as all three
/// passes in one iteration report `changed = false`.
pub fn optimize(node: CompiledNode, engine: &DataLogic) -> CompiledNode {
    let mut node = node;
    for _ in 0..MAX_FIXPOINT_ITERATIONS {
        let mut any_changed = false;

        let (n, changed) = dead_code::eliminate(node, engine);
        node = n;
        any_changed |= changed;

        let (n, changed) = constant_fold::fold(node, engine);
        node = n;
        any_changed |= changed;

        let (n, changed) = strength::reduce(node);
        node = n;
        any_changed |= changed;

        if !any_changed {
            break;
        }
    }
    node
}
