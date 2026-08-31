//! Static scope model — the compile-time answer to "how many context frames
//! are pushed around this node when it evaluates?".
//!
//! # Invariant SD
//!
//! For any node that is *actually evaluated*, [`crate::arena::ContextStack::depth`]
//! at the moment of evaluation equals that node's static lexical frame depth,
//! as computed by [`frames_pushed_for_child`] accumulated from the root.
//!
//! This holds even though frames are pushed conditionally at runtime, because
//! every conditional push is "push iff the body runs":
//!
//! - An **empty collection** never reaches a `step_*` on its `IterGuard`, so no
//!   frame is pushed — but the body never runs either, so nothing observes it.
//! - A **scalar "bridge" input** (`map_bridge_single`) pushes exactly one
//!   `Indexed` frame, matching the array case.
//! - A **`try` catch arm** runs under the caught-error frame exactly when
//!   `args.len() >= 2`: `evaluate_try` returns early for shorter forms and
//!   never reaches the catch path, and for the multi-arg form the arm loop
//!   guarantees an error is in hand by the time the last arm runs. Its other
//!   early return — a literal catch arm — reads no context, so the skipped
//!   push is unobservable.
//! - The **filter invariant fast path** pushes a synthetic null frame
//!   precisely to *preserve* this invariant while skipping the per-item frame.
//!
//! # Single source of truth
//!
//! Two consumers depend on knowing which child positions run under a frame:
//! this module's scope resolution, and the CSE pass (which uses it to skip
//! wrapping subtrees that could never hit the depth-gated runtime memo).
//! Both call [`frames_pushed_for_child`] so the two can never drift.

use crate::node::{CompiledNode, ScopeBinding};
use crate::opcode::OpCode;

/// Number of context frames pushed around child `index` of `opcode` when that
/// child executes. `len` is the operator's total argument count.
///
/// Returns `0` for every position that evaluates at the operator's own depth,
/// and `1` for the positions that run under a pushed frame:
///
/// - **iterator bodies** — `args[1]` of `filter` / `map` / `all` / `some` /
///   `none` / `reduce`, which run under a per-item frame. Note `reduce`'s
///   `args[2]` (the initial accumulator) evaluates once *outside* the
///   iteration frames and is therefore `0`.
/// - **key expressions** — `args[2]` of `sort` (its `args[1]` is the scalar
///   direction flag) and `args[1]` of `group_by` / `distinct`.
/// - **the catch arm** — the last argument of a multi-arg `try`.
///
/// `min` / `max` / `merge` consume `args[0]` as an iterable but have no body
/// position and push nothing, so they are absent here by design.
///
/// Adding an operator that pushes a frame **must** register its argument
/// position here, or variable references beneath it resolve against the wrong
/// frame. The debug oracle in `operators::variable` catches an omission on the
/// first test that exercises the operator.
///
/// No operator currently pushes more than one frame for a single child, but
/// the return type is `u32` rather than `bool` so a future nested-frame
/// operator is expressible without changing every call site.
pub(crate) fn frames_pushed_for_child(opcode: OpCode, index: usize, len: usize) -> u32 {
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::Sort) {
        return u32::from(index == 2);
    }
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::GroupBy | OpCode::Distinct) {
        return u32::from(index == 1);
    }
    #[cfg(feature = "error-handling")]
    if matches!(opcode, OpCode::Try) {
        return u32::from(len >= 2 && index == len - 1);
    }
    let _ = len;
    u32::from(
        matches!(
            opcode,
            OpCode::Filter
                | OpCode::Map
                | OpCode::All
                | OpCode::Some
                | OpCode::None
                | OpCode::Reduce
        ) && index == 1,
    )
}

/// Annotate every variable-reading node in the tree with its compile-time
/// frame resolution.
///
/// Runs once over the finished tree from `Logic::compile_with`, after the CSE
/// pass (so wrappers are in place and walked transparently) and before
/// `Logic::new`'s populate pass. Unlike folding and CSE it is **not** gated on
/// `skip_fold`: an engine built with `with_constant_folding(false)`, and every
/// traced compile, should get the same resolution as an optimized one.
///
/// This pass only *annotates*. It creates no nodes, allocates no ids, and
/// never duplicates or wraps a subtree — so it cannot perturb the id
/// uniqueness [`crate::Logic::resolve_node_ids`] depends on, and it leaves
/// intact the raw `Var` / `Value` shapes that `populate_lits`,
/// `IterArgKind::classify`, `FastPredicate::try_detect_owned` and
/// `FusedMapBody::detect` all pattern-match. Wrapping a `Var` here would
/// silently disable those fast paths without a compile error.
pub(crate) fn resolve(root: &mut CompiledNode) {
    resolve_at(root, 0);
}

fn resolve_at(node: &mut CompiledNode, depth: u32) {
    match node {
        CompiledNode::Var {
            scope_level,
            binding,
            ..
        } => *binding = ScopeBinding::resolve(depth, *scope_level),
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => data.binding = ScopeBinding::resolve(depth, data.scope_level),
        _ => {}
    }

    // Only a `BuiltinOperator` can raise the depth for its children; every
    // other variant evaluates its children at its own depth, so the shared
    // child walker handles them. `Cse` is depth-transparent and reaches the
    // generic arm through `visit_children_mut`, matching every other
    // consumer's treatment of the wrapper.
    if let CompiledNode::BuiltinOperator { opcode, args, .. } = node {
        let opcode = *opcode;
        let len = args.len();
        for (index, child) in args.iter_mut().enumerate() {
            resolve_at(child, depth + frames_pushed_for_child(opcode, index, len));
        }
        return;
    }

    node.visit_children_mut(&mut |child| resolve_at(child, depth));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::ScopeBinding;

    #[test]
    fn iterator_bodies_push_one_frame() {
        for op in [
            OpCode::Filter,
            OpCode::Map,
            OpCode::All,
            OpCode::Some,
            OpCode::None,
            OpCode::Reduce,
        ] {
            assert_eq!(frames_pushed_for_child(op, 0, 3), 0, "{op:?} source");
            assert_eq!(frames_pushed_for_child(op, 1, 3), 1, "{op:?} body");
        }
    }

    /// `reduce`'s initial accumulator evaluates outside the iteration frames.
    #[test]
    fn reduce_initial_accumulator_is_frameless() {
        assert_eq!(frames_pushed_for_child(OpCode::Reduce, 2, 3), 0);
    }

    /// Operators that iterate `args[0]` but have no body push nothing.
    #[test]
    fn bodyless_iterators_push_nothing() {
        for op in [OpCode::Min, OpCode::Max, OpCode::Merge] {
            for index in 0..3 {
                assert_eq!(frames_pushed_for_child(op, index, 3), 0, "{op:?}[{index}]");
            }
        }
    }

    #[cfg(feature = "ext-array")]
    #[test]
    fn sort_and_key_extractors() {
        // sort: args[1] is the direction flag, args[2] the extractor.
        assert_eq!(frames_pushed_for_child(OpCode::Sort, 1, 3), 0);
        assert_eq!(frames_pushed_for_child(OpCode::Sort, 2, 3), 1);
        for op in [OpCode::GroupBy, OpCode::Distinct] {
            assert_eq!(frames_pushed_for_child(op, 0, 2), 0, "{op:?} source");
            assert_eq!(frames_pushed_for_child(op, 1, 2), 1, "{op:?} key");
        }
    }

    /// A single-arg `try` never reaches the catch path, so its lone argument
    /// evaluates at the operator's own depth.
    #[cfg(feature = "error-handling")]
    #[test]
    fn try_catch_arm_only_for_multi_arg() {
        assert_eq!(frames_pushed_for_child(OpCode::Try, 0, 1), 0);
        assert_eq!(frames_pushed_for_child(OpCode::Try, 0, 2), 0);
        assert_eq!(frames_pushed_for_child(OpCode::Try, 1, 2), 1);
        // Three-arm form: only the final arm is the catch arm.
        assert_eq!(frames_pushed_for_child(OpCode::Try, 1, 3), 0);
        assert_eq!(frames_pushed_for_child(OpCode::Try, 2, 3), 1);
    }

    /// The full `(static_depth, scope_level)` resolution table, pinned against
    /// the arithmetic of `ContextStack::get_at_level` — clamp and off-by-one
    /// included. Verified against a direct probe of the running engine: with
    /// three nested `map`s over frames `[A, B, C]`, `[[0]]` and `[[1]]` both
    /// yield `C`, `[[2]]` yields `B`, and `[[3]]` and beyond yield the root.
    ///
    /// Note `Ancestor` only becomes reachable at depth 3. That is why the
    /// conformance corpus — whose deepest iterator nesting is 2 — cannot
    /// distinguish a correct interior-frame resolver from a broken one.
    #[test]
    fn resolution_table() {
        use ScopeBinding::{Ancestor, Current, Root};
        let expected = [
            // (depth, level, binding)
            (0, 0, Root),    // no frames at all
            (0, 1, Root),    // clamp
            (0, 5, Root),    // clamp
            (1, 0, Current), // the sole frame
            (1, 1, Root),    // clamp: level >= depth
            (1, 2, Root),
            (2, 0, Current),
            (2, 1, Current), // the off-by-one: [[1]] aliases [[0]]
            (2, 2, Root),    // clamp
            (2, 3, Root),
            (3, 0, Current),
            (3, 1, Current),  // off-by-one again
            (3, 2, Ancestor), // first genuinely reachable interior frame
            (3, 3, Root),     // clamp
            (4, 2, Ancestor),
            (4, 3, Ancestor),
            (4, 4, Root),
        ];
        for (depth, level, want) in expected {
            assert_eq!(
                ScopeBinding::resolve(depth, level),
                want,
                "resolve(depth={depth}, level={level})"
            );
        }
    }

    /// The outermost frame is unreachable at every depth: no level resolves to
    /// frame index 0. Pinned so a future change to the walk has to confront it
    /// deliberately rather than "fixing" it by accident.
    #[test]
    fn outermost_frame_is_never_addressable() {
        for depth in 1..8u32 {
            for level in 0..depth + 3 {
                let b = ScopeBinding::resolve(depth, level);
                // `Ancestor` covers 2..=depth-1, which never includes the
                // index-0 frame; everything else is Current or Root.
                if b == ScopeBinding::Ancestor {
                    assert!(
                        (2..depth).contains(&level),
                        "Ancestor at depth={depth} level={level} is out of the \
                         reachable interior range 2..{depth}"
                    );
                }
            }
        }
    }

    /// Ordinary operators evaluate every argument at their own depth.
    #[test]
    fn plain_operators_push_nothing() {
        for index in 0..4 {
            assert_eq!(frames_pushed_for_child(OpCode::Add, index, 4), 0);
            assert_eq!(frames_pushed_for_child(OpCode::If, index, 4), 0);
        }
    }
}
