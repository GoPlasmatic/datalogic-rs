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
//! - The **filter invariant fast path** is the one deliberate exception: it
//!   dispatches a hoisted predicate operand one frame shallower than its
//!   static depth. That is sound only because `is_filter_invariant` admits
//!   nothing but literals and `Root`-bound references, neither of which
//!   reads the stack — and a level that clamps to the root at depth `D`
//!   still clamps at `D - 1`, so the debug oracle agrees.
//!
//! # Single source of truth
//!
//! Two consumers depend on knowing which child positions run under a frame:
//! this module's scope resolution, and the CSE pass (which uses it to skip
//! wrapping subtrees that could never hit the depth-gated runtime memo).
//! Both call [`frames_pushed_for_child`] so the two can never drift.

use crate::node::{CompiledNode, MetadataHint, ScopeBinding, metadata_reads_ancestor};
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
/// Returns whether the tree can ever read an *ancestor* frame — i.e. a frame
/// below the innermost one. That is the only thing `ContextStack::parents`
/// exists to serve: a lookup reaches it exactly when its climb lands on a
/// strict ancestor, which for a data read is [`ScopeBinding::Ancestor`] and
/// for an `index` / `key` read is [`metadata_reads_ancestor`]. The two can
/// disagree, because a metadata climb is one frame shorter than the data
/// climb at the same odd level. When this is `false` the evaluator can skip
/// maintaining the ancestor list entirely.
///
/// Deliberately conservative. A dynamic `val` — `{"val": [<expr>, …]}`, which
/// stays a `BuiltinOperator` because its level is not a literal — resolves its
/// level from *data* at runtime and can therefore reach any depth, so its mere
/// presence forces the list on.
pub(crate) fn resolve(root: &mut CompiledNode) -> bool {
    let mut needs_ancestors = false;
    resolve_at(root, 0, &mut needs_ancestors);
    needs_ancestors
}

fn resolve_at(node: &mut CompiledNode, depth: u32, needs_ancestors: &mut bool) {
    match node {
        CompiledNode::Var {
            scope_level,
            binding,
            metadata_hint,
            ..
        } => {
            *binding = ScopeBinding::resolve(depth, *scope_level);
            // A node with a metadata hint reads `index`/`key` from the frame
            // `metadata_climb` names and never walks to `binding`'s frame, so
            // only one of the two questions applies to it.
            *needs_ancestors |= if *metadata_hint != MetadataHint::None {
                metadata_reads_ancestor(depth, *scope_level)
            } else {
                *binding == ScopeBinding::Ancestor
            };
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => {
            data.binding = ScopeBinding::resolve(depth, data.scope_level);
            *needs_ancestors |= data.binding == ScopeBinding::Ancestor;
        }
        // A `val` that survived as a generic operator has a non-literal level
        // argument, so the frame it reads is only known at runtime and could
        // be any ancestor.
        CompiledNode::BuiltinOperator {
            opcode: OpCode::Val,
            ..
        } => *needs_ancestors = true,
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
            resolve_at(
                child,
                depth + frames_pushed_for_child(opcode, index, len),
                needs_ancestors,
            );
        }
        return;
    }

    node.visit_children_mut(&mut |child| resolve_at(child, depth, needs_ancestors));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{ScopeBinding, metadata_reads_ancestor};

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

    /// The full `(static_depth, scope_level)` resolution table for a data
    /// read, pinned against the arithmetic of `ContextStack::get_at_level`.
    /// A level climbs `ceil(level / 2)` frames, so `[[2k-1]]` and `[[2k]]`
    /// name the same frame and the clamp fires only once the climb passes the
    /// outermost frame. Verified against a direct probe of the running
    /// engine: with three nested `map`s over frames `[A, B, C]`, `[[0]]`
    /// yields `C`, `[[1]]` and `[[2]]` yield `B`, `[[3]]` and `[[4]]` yield
    /// `A`, and `[[5]]` and beyond yield the root.
    #[test]
    fn resolution_table() {
        use ScopeBinding::{Ancestor, Current, Root};
        let expected = [
            // (depth, level, binding)
            (0, 0, Root),    // no frames at all
            (0, 1, Root),    // clamp
            (0, 5, Root),    // clamp
            (1, 0, Current), // the sole frame
            (1, 1, Root),    // clamp: climb 1 >= depth
            (1, 2, Root),
            (2, 0, Current),
            (2, 1, Ancestor), // climb 1: the enclosing frame
            (2, 2, Ancestor), // same frame as [[1]]
            (2, 3, Root),     // clamp: climb 2 >= depth
            (2, 4, Root),
            (3, 0, Current),
            (3, 1, Ancestor), // climb 1
            (3, 2, Ancestor),
            (3, 3, Ancestor), // climb 2: the outermost frame
            (3, 4, Ancestor),
            (3, 5, Root), // clamp
            (4, 4, Ancestor),
            (4, 6, Ancestor),
            (4, 7, Root),
        ];
        for (depth, level, want) in expected {
            assert_eq!(
                ScopeBinding::resolve(depth, level),
                want,
                "resolve(depth={depth}, level={level})"
            );
        }
    }

    /// Every frame is addressable, the outermost included — the property
    /// issue #74 was about. The outermost frame is the one a climb of
    /// `depth - 1` names, which is where levels `2*depth - 3` and
    /// `2*depth - 2` land.
    #[test]
    fn every_frame_is_addressable() {
        use crate::arena::{FrameTarget, frame_at_climb};
        for depth in 2..8u32 {
            assert_eq!(
                frame_at_climb(depth as usize, depth as usize - 1),
                FrameTarget::Ancestor(0),
                "the outermost frame sits at parents[0] at depth={depth}"
            );
            for level in [2 * depth - 3, 2 * depth - 2] {
                assert_eq!(
                    ScopeBinding::resolve(depth, level),
                    ScopeBinding::Ancestor,
                    "depth={depth} level={level} names the outermost frame"
                );
            }
            assert_eq!(
                ScopeBinding::resolve(depth, 2 * depth - 1),
                ScopeBinding::Root,
                "one level further clamps to the root"
            );
        }
    }

    /// What `needs_ancestor_frames` actually costs: it turns on only for a
    /// rule that both nests two frames and reads a level that reaches past
    /// the innermost one. A single iterator never pays for the ancestor list,
    /// whatever level it uses, because every level there clamps to the root.
    #[cfg(feature = "serde_json")]
    #[test]
    fn ancestor_tracking_turns_on_only_where_a_level_can_reach_one() {
        let engine = crate::Engine::new();
        for (rule, want, why) in [
            (
                r#"{"map": [{"val": "a"}, {"val": [[2], "x"]}]}"#,
                false,
                "one frame: clamps to root",
            ),
            (
                r#"{"map": [{"val": "a"}, {"val": [[1], "index"]}]}"#,
                false,
                "metadata of the current frame",
            ),
            (
                r#"{"map": [{"val": "a"}, {"map": [{"val": "b"}, {"val": "x"}]}]}"#,
                false,
                "nested, but no level",
            ),
            (
                r#"{"map": [{"val": "a"}, {"map": [{"val": "b"}, {"val": [[1], "x"]}]}]}"#,
                true,
                "nested data level",
            ),
            (
                r#"{"map": [{"val": "a"}, {"map": [{"val": "b"}, {"val": [[3], "index"]}]}]}"#,
                true,
                "nested metadata level: the data climb clamps to root, the metadata climb does not",
            ),
            (
                r#"{"map": [{"val": "a"}, {"map": [{"val": "b"}, {"val": [[1], "index"]}]}]}"#,
                false,
                "nested metadata level naming the innermost frame: the data binding is Ancestor, but the node never walks it",
            ),
            (
                r#"{"map": [{"val": "a"}, {"map": [{"val": "b"}, {"val": [[4], "x"]}]}]}"#,
                false,
                "nested, but the level clamps past both frames",
            ),
        ] {
            let logic = engine.compile(rule).expect("compiles");
            assert_eq!(logic.needs_ancestor_frames, want, "{why}: {rule}");
        }
    }

    /// A metadata climb is one frame shorter than the data climb at the same
    /// odd level, so the ancestor requirement has to be tested separately.
    #[test]
    fn metadata_climb_can_need_ancestors_when_data_does_not() {
        // depth 2, `[[3]]`: data clamps to the root, metadata reads the
        // enclosing frame's index.
        assert_eq!(ScopeBinding::resolve(2, 3), ScopeBinding::Root);
        assert!(metadata_reads_ancestor(2, 3));
        // `[[1]]` reads the innermost frame's metadata at any depth.
        assert!(!metadata_reads_ancestor(4, 1));
        // An even level names no metadata frame at all.
        assert!(!metadata_reads_ancestor(4, 2));
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
