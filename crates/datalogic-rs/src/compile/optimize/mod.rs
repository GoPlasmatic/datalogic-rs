//! Optimization passes for compiled logic trees.
//!
//! Two kinds of pass live here:
//!
//! - **Per-call rewrites**, run by [`optimize`] on each builtin call the
//!   walker builds, to a fixpoint: `dead_code::eliminate`,
//!   `constant_fold::fold` and `strength::reduce`. Each takes the node by
//!   value and returns `(node, changed)`; the ones that consult the
//!   engine's settings take `&Engine`.
//! - **Whole-tree passes**, run once by `Logic::compile_inner` on the
//!   finished tree: `cse::apply`. `constant_fold::fold_static_node`
//!   evaluates a fully static node at compile time for the walker.
//!
//! None of them run for a traced compile or on an engine built with
//! folding off.
//!
//! # Adding a per-call rewrite
//!
//! 1. Create a new file in this directory (e.g., `my_pass.rs`).
//! 2. Implement `fn my_pass(node: CompiledNode, ...) -> (CompiledNode, bool)`,
//!    returning whether it changed anything.
//! 3. Call it from [`optimize`] below. If its decision reads the engine's
//!    settings, fold its `changed` into the `observed` flag `optimize`
//!    returns, so the rule is compiled again on an engine whose settings
//!    differ.
//! 4. Run `cargo test --all-features`.

pub(super) mod constant_fold;
pub(super) mod cse;
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
/// Called from the walker for each builtin call unless folding is skipped (a
/// traced compile, or an engine built with folding off).
///
/// Passes are applied in order until none report a change or
/// [`MAX_FIXPOINT_ITERATIONS`] is reached. Per iteration:
/// 1. Dead code elimination (remove unreachable branches)
/// 2. Constant folding (the leading integer literals of `+` / `*`, adjacent `cat` strings)
/// 3. Strength reduction (double negation collapse, etc.), skipped under a
///    custom truthy evaluator
/// 4. Dead code elimination (cleanup pass — catches branches that
///    became unreachable from the strength-reduction output, so the
///    fixpoint converges in one iteration instead of two for compound
///    cases like `!!!x → BoolCast(!x)` whose new shape exposes a
///    constant predicate to a surrounding `if`).
///
/// Each pass returns `(node, changed)`; the loop exits as soon as all
/// passes in one iteration report `changed = false`.
///
/// Also returns whether any rewrite rested on the engine's settings: dead
/// code elimination decides by the engine's truthiness, and strength
/// reduction runs only for a built-in truthiness.
pub(super) fn optimize(node: CompiledNode, engine: &Engine) -> (CompiledNode, bool) {
    let mut node = node;
    let custom_truthy = matches!(
        engine.config().truthy_evaluator,
        crate::TruthyEvaluator::Custom(_)
    );
    let mut observed = false;
    for _ in 0..MAX_FIXPOINT_ITERATIONS {
        let mut any_changed = false;

        let (n, changed) = dead_code::eliminate(node, engine);
        node = n;
        any_changed |= changed;
        observed |= changed;

        let (n, changed) = constant_fold::fold(node);
        node = n;
        any_changed |= changed;

        // The truth compositions assume a boolean is its own truthiness,
        // which a custom truthy evaluator need not keep.
        if !custom_truthy {
            let (n, changed) = strength::reduce(node);
            node = n;
            any_changed |= changed;
            observed |= changed;
        }

        // Cleanup pass — collapse anything strength produced before
        // exiting the iteration, instead of leaving it to the next
        // round.
        let (n, changed) = dead_code::eliminate(node, engine);
        node = n;
        any_changed |= changed;
        observed |= changed;

        if !any_changed {
            break;
        }
    }
    (node, observed)
}
