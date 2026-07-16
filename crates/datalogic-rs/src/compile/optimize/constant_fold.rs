//! Partial constant folding pass.
//!
//! Folds static arguments in commutative/associative operators:
//! - `{"+": [1, 2, {"var": "x"}, 3]}` → `{"+": [6, {"var": "x"}]}`
//! - `{"cat": ["hello ", "world", {"var": "name"}]}` → `{"cat": ["hello world", {"var": "name"}]}`
//! - `{"*": [2, {"var": "x"}, 5]}` → `{"*": [10, {"var": "x"}]}`
//!
//! Numeric string literals are deliberately NOT pre-coerced — see the note in [`fold`].

use crate::Engine;
use crate::node::{CompiledNode, SYNTHETIC_ID, node_is_static};
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

/// Apply partial constant folding to a compiled node.
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Used by the optimiser pipeline to drive fixpoint iteration.
pub(crate) fn fold(node: CompiledNode, engine: &Engine) -> (CompiledNode, bool) {
    // NOTE: a `precoerce_numeric_strings` pass used to run here, rewriting
    // numeric string literals in arithmetic contexts into number literals
    // (`{"+": ["5", x]}` → `{"+": [5, x]}`). It was removed as unsound: at
    // runtime a *string* operand keeps the arithmetic in f64 space (its
    // coercion rounds beyond 2^53), while a *number* literal — even an
    // integral float — takes the exact-integer paths, so the rewrite
    // changed observable results (e.g. `3 + "9007199254740990"`), caught
    // by the differential property oracle. `try_partial_fold` below stays
    // sound because it evaluates static args through the real engine.
    match &node {
        CompiledNode::BuiltinOperator {
            id, opcode, args, ..
        } => {
            // Partial fold for commutative operators with mixed static/dynamic args
            if is_commutative(opcode) && args.len() >= 2 {
                match try_partial_fold(*id, *opcode, args, engine) {
                    Some(new) => (new, true),
                    None => (node, false),
                }
            } else if *opcode == OpCode::Concat && args.len() >= 2 {
                match try_fold_concat(*id, args) {
                    Some(new) => (new, true),
                    None => (node, false),
                }
            } else {
                (node, false)
            }
        }
        _ => (node, false),
    }
}

/// Check if an operator is commutative and associative (safe to reorder static args).
fn is_commutative(opcode: &OpCode) -> bool {
    matches!(opcode, OpCode::Add | OpCode::Multiply)
}

/// Try to fold static args in a commutative operator.
/// E.g., `{"+": [1, {"var":"x"}, 2, 3]}` → `{"+": [6, {"var":"x"}]}`
fn try_partial_fold(
    outer_id: crate::node::NodeId,
    opcode: OpCode,
    args: &[CompiledNode],
    engine: &Engine,
) -> Option<CompiledNode> {
    // Count first (no cloning): need at least 2 static args to fold, and at
    // least 1 dynamic to be "partial". Bail before cloning any subtree.
    let static_count = args.iter().filter(|a| node_is_static(a)).count();
    if static_count < 2 || static_count == args.len() {
        return None;
    }

    let mut static_args: Vec<CompiledNode> = Vec::with_capacity(static_count);
    let mut dynamic_args: Vec<CompiledNode> = Vec::with_capacity(args.len() - static_count);

    for arg in args {
        if node_is_static(arg) {
            static_args.push(arg.clone());
        } else {
            dynamic_args.push(arg.clone());
        }
    }

    // Evaluate the static portion. The transient node is purely local — it
    // doesn't appear in the compiled tree, so synthetic ids are fine.
    let static_node = CompiledNode::BuiltinOperator {
        id: SYNTHETIC_ID,
        opcode,
        args: static_args.into_boxed_slice(),
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    };
    let folded_value = fold_static_node(&static_node, engine)?;

    // Reconstruct: [folded_constant, ...dynamic_args]. The folded literal
    // gets SYNTHETIC_ID (literals never emit trace steps). The outer op keeps
    // its original id so tracing / error reporting still point at the source.
    let mut new_args = Vec::with_capacity(1 + dynamic_args.len());
    new_args.push(CompiledNode::synthetic_value(folded_value));
    new_args.extend(dynamic_args);

    Some(CompiledNode::BuiltinOperator {
        id: outer_id,
        opcode,
        args: new_args.into_boxed_slice(),
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    })
}

/// Try to fold adjacent static strings in cat operator.
/// `{"cat": ["hello ", "world", {"var": "x"}]}` → `{"cat": ["hello world", {"var": "x"}]}`
fn try_fold_concat(outer_id: crate::node::NodeId, args: &[CompiledNode]) -> Option<CompiledNode> {
    // Bail before cloning unless two adjacent string literals exist — that
    // adjacency is the only thing that sets `folded_any` below.
    let has_adjacent_strings = args.windows(2).any(|w| {
        matches!(
            (&w[0], &w[1]),
            (
                CompiledNode::Value {
                    value: OwnedDataValue::String(_),
                    ..
                },
                CompiledNode::Value {
                    value: OwnedDataValue::String(_),
                    ..
                },
            )
        )
    });
    if !has_adjacent_strings {
        return None;
    }

    let mut new_args: Vec<CompiledNode> = Vec::new();
    let mut current_static_str: Option<String> = None;
    let mut folded_any = false;

    for arg in args {
        if let CompiledNode::Value {
            value: OwnedDataValue::String(s),
            ..
        } = arg
        {
            match &mut current_static_str {
                Some(accumulated) => {
                    accumulated.push_str(s);
                    folded_any = true;
                }
                None => {
                    current_static_str = Some(s.clone());
                }
            }
        } else {
            // Flush any accumulated static string
            if let Some(s) = current_static_str.take() {
                new_args.push(CompiledNode::synthetic_value(OwnedDataValue::String(s)));
            }
            new_args.push(arg.clone());
        }
    }

    // Flush final accumulated string
    if let Some(s) = current_static_str.take() {
        new_args.push(CompiledNode::synthetic_value(OwnedDataValue::String(s)));
    }

    if !folded_any {
        return None;
    }

    if new_args.len() == 1 {
        // Entire cat was static strings
        return Some(new_args.into_iter().next().unwrap());
    }

    Some(CompiledNode::BuiltinOperator {
        id: outer_id,
        opcode: OpCode::Concat,
        args: new_args.into_boxed_slice(),
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    })
}

/// One-shot arena evaluation for compile-time constant folding.
///
/// The arena lives only for this fold call — uses a fresh `Bump`, not the
/// thread-local pool, since folding runs during `compile`, not the eval hot
/// path. Returns `None` on any error (the caller falls back to leaving the
/// node un-folded).
pub(crate) fn fold_static_node(node: &CompiledNode, engine: &Engine) -> Option<OwnedDataValue> {
    let arena = bumpalo::Bump::new();
    let null_root: &crate::arena::DataValue<'_> = arena.alloc(crate::arena::DataValue::Null);
    let mut ctx = crate::arena::ContextStack::new(null_root);
    let av = engine.dispatch_node(node, &mut ctx, &arena).ok()?;
    Some(av.to_owned())
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{builtin, val, var_node};
    use super::*;
    use datavalue::OwnedDataValue;

    fn ov(s: &str) -> OwnedDataValue {
        OwnedDataValue::from_json(s).unwrap()
    }

    #[test]
    fn test_partial_fold_add() {
        let engine = Engine::new();
        let node = builtin(
            OpCode::Add,
            vec![val(ov("1")), val(ov("2")), var_node("x"), val(ov("3"))],
        );
        let (result, _changed) = fold(node, &engine);
        if let CompiledNode::BuiltinOperator { args, .. } = &result {
            assert_eq!(args.len(), 2);
            if let CompiledNode::Value { value, .. } = &args[0] {
                assert_eq!(value.as_i64(), Some(6));
            } else {
                panic!("expected folded value");
            }
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    #[test]
    fn test_fold_cat_adjacent() {
        let engine = Engine::new();
        let node = builtin(
            OpCode::Concat,
            vec![val(ov("\"hello \"")), val(ov("\"world\"")), var_node("x")],
        );
        let (result, _changed) = fold(node, &engine);
        if let CompiledNode::BuiltinOperator { args, .. } = &result {
            assert_eq!(args.len(), 2);
            if let CompiledNode::Value { value, .. } = &args[0] {
                assert_eq!(value.as_str(), Some("hello world"));
            }
        }
    }

    #[test]
    fn numeric_strings_are_not_precoerced() {
        // A numeric string literal must stay a string: at runtime a string
        // operand keeps the arithmetic in f64 space, so rewriting it into a
        // number literal changes observable results beyond 2^53.
        let engine = Engine::new();
        let node = builtin(OpCode::Add, vec![val(ov("\"5\"")), var_node("x")]);
        let (result, changed) = fold(node, &engine);
        assert!(!changed);
        if let CompiledNode::BuiltinOperator { args, .. } = &result {
            assert!(matches!(
                &args[0],
                CompiledNode::Value {
                    value: OwnedDataValue::String(_),
                    ..
                }
            ));
        } else {
            panic!("expected BuiltinOperator");
        }
    }
}
