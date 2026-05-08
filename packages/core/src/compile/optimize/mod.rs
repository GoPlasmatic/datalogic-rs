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

pub(super) mod constant_fold;
pub(super) mod dead_code;
mod helpers;
pub(super) mod strength;

#[cfg(test)]
mod test_helpers;

use crate::Engine;
use crate::node::CompiledNode;

/// Maximum number of fixpoint iterations for the optimiser pipeline.
///
/// Three passes (dead code / constant fold / strength reduction) can feed each
/// other: folding exposes new dead branches; strength reduction can expose new
/// constants. A small cap is enough to catch the compounds we've seen in practice
/// (1–2 iterations after the per-iteration cleanup pass below) while bounding
/// worst-case compile time.
const MAX_FIXPOINT_ITERATIONS: usize = 4;

/// Run all optimization passes on a compiled node tree until a fixpoint.
///
/// This is the main entry point for the optimization pipeline.
/// Called from `compile_node` when an engine is provided (i.e., not in trace mode).
///
/// Passes are applied in order until none report a change or
/// [`MAX_FIXPOINT_ITERATIONS`] is reached. Per iteration:
/// 1. Dead code elimination (remove unreachable branches)
/// 2. Constant folding (fold static args in commutative ops, pre-coerce numeric strings)
/// 3. Strength reduction (double negation collapse, etc.)
/// 4. Dead code elimination (cleanup pass — catches branches that
///    became unreachable from the strength-reduction output, so the
///    fixpoint converges in one iteration instead of two for compound
///    cases like `!!!x → BoolCast(!x)` whose new shape exposes a
///    constant predicate to a surrounding `if`).
///
/// Each pass returns `(node, changed)`; the loop exits as soon as all
/// passes in one iteration report `changed = false`.
pub(super) fn optimize(node: CompiledNode, engine: &Engine) -> CompiledNode {
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

        // Cleanup pass — collapse anything strength produced before
        // exiting the iteration, instead of leaving it to the next
        // round.
        let (n, changed) = dead_code::eliminate(node, engine);
        node = n;
        any_changed |= changed;

        if !any_changed {
            break;
        }
    }
    node
}
