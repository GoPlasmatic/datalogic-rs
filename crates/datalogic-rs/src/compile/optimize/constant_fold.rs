//! Partial constant folding pass.
//!
//! Folds literal arguments where the result cannot change:
//! - `{"+": [1, 2, {"var": "x"}, 3]}` → `{"+": [3, {"var": "x"}, 3]}`
//!   (the leading integers of a variadic `+` / `*`; see [`try_partial_fold`])
//! - `{"cat": ["hello ", "world", {"var": "name"}]}` → `{"cat": ["hello world", {"var": "name"}]}`
//!
//! Numeric string literals are deliberately NOT pre-coerced — see the note in [`fold`].

use crate::Engine;
use crate::OpCode;
use crate::node::CompiledNode;
use crate::operators::meta::{Algebra, ArithOp};
use datavalue::{NumberValue, OwnedDataValue};

/// Apply partial constant folding to a compiled node.
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Used by the optimiser pipeline to drive fixpoint iteration.
pub(crate) fn fold(node: CompiledNode) -> (CompiledNode, bool) {
    // NOTE: a `precoerce_numeric_strings` pass used to run here, rewriting
    // numeric string literals in arithmetic contexts into number literals
    // (`{"+": ["5", x]}` → `{"+": [5, x]}`). It was removed as unsound: at
    // runtime a *string* operand keeps the arithmetic in f64 space (its
    // coercion rounds beyond 2^53), while a *number* literal — even an
    // integral float — takes the exact-integer paths, so the rewrite
    // changed observable results (e.g. `3 + "9007199254740990"`), caught
    // by the differential property oracle. `try_partial_fold` below stays
    // sound by folding only what leaves the evaluator's state unchanged.
    match &node {
        CompiledNode::BuiltinOperator {
            id, opcode, args, ..
        } => {
            // Partial fold of the leading literals of `+` / `*`
            if associative_op(*opcode).is_some() {
                match try_partial_fold(*id, *opcode, args) {
                    Some(new) => (new, true),
                    None => (node, false),
                }
            } else if opcode.algebra() == Some(Algebra::Concat) && args.len() >= 2 {
                match try_fold_concat(*id, *opcode, args) {
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

/// The arithmetic of an operator whose leading literal arguments may be
/// folded together, read from its table row's `algebra`.
fn associative_op(opcode: OpCode) -> Option<ArithOp> {
    match opcode.meta().algebra {
        Some(Algebra::Arith(op)) if op.is_associative() => Some(op),
        _ => None,
    }
}

/// Fold the leading integer literals of a variadic `+` or `*`.
/// E.g., `{"+": [1, 2, {"var":"x"}, 3]}` → `{"+": [3, {"var":"x"}, 3]}`.
///
/// The fold must leave the evaluator in the state it would have reached on
/// its own, so it is narrower than associativity alone allows:
///
/// - Only a *leading* run: the evaluator accumulates left to right, and an
///   integer that overflows into `f64`, `f64` rounding and numeric-string
///   coercion are not associative, so a literal after a dynamic argument
///   cannot be moved ahead of it.
/// - Only integer literals, folded exactly: the accumulator after an
///   exact-integer run is that integer, as the folded literal starts it. A
///   run that overflows is left alone.
/// - Only when three or more arguments remain: the two-argument forms
///   coerce and dispatch differently from the variadic one (datetime
///   arithmetic, string-to-integer coercion), so `[1, 2, x]` stays as is.
fn try_partial_fold(
    outer_id: crate::node::NodeId,
    opcode: OpCode,
    args: &[CompiledNode],
) -> Option<CompiledNode> {
    let op = associative_op(opcode)?;
    let lead = args
        .iter()
        .take_while(|a| {
            matches!(
                a,
                CompiledNode::Value {
                    value: OwnedDataValue::Number(NumberValue::Integer(_)),
                    ..
                }
            )
        })
        .count();
    // At least two to fold, and three arguments left after folding.
    if lead < 2 || args.len() - lead + 1 < 3 {
        return None;
    }
    let combine = match op {
        ArithOp::Mul => i64::checked_mul,
        _ => i64::checked_add,
    };
    let folded = args[..lead]
        .iter()
        .try_fold(op.right_identity(), |acc, a| match a {
            CompiledNode::Value {
                value: OwnedDataValue::Number(NumberValue::Integer(i)),
                ..
            } => combine(acc, *i),
            _ => None,
        })?;

    // Reconstruct: [folded_constant, ...rest]. The folded literal gets
    // SYNTHETIC_ID (literals never emit trace steps). The outer op keeps
    // its original id so tracing / error reporting still point at the source.
    let mut new_args = Vec::with_capacity(1 + args.len() - lead);
    new_args.push(CompiledNode::synthetic_value(OwnedDataValue::Number(
        NumberValue::Integer(folded),
    )));
    new_args.extend(args[lead..].iter().cloned());

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
fn try_fold_concat(
    outer_id: crate::node::NodeId,
    opcode: OpCode,
    args: &[CompiledNode],
) -> Option<CompiledNode> {
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
        opcode,
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
    // Only `node_is_static` subtrees reach here, and that predicate rejects
    // every `Var` / `Exists` / `Missing`, so no frame is ever pushed.
    let mut ctx = crate::arena::ContextStack::new(null_root, false);
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

    /// The folded literal of `args`' partial fold, and how many arguments
    /// the call keeps; `None` when nothing folds.
    fn partial(opcode: OpCode, args: Vec<CompiledNode>) -> Option<(OwnedDataValue, usize)> {
        let (result, changed) = fold(builtin(opcode, args));
        if !changed {
            return None;
        }
        let CompiledNode::BuiltinOperator { args, .. } = &result else {
            panic!("expected BuiltinOperator");
        };
        let CompiledNode::Value { value, .. } = &args[0] else {
            panic!("expected folded value");
        };
        Some((value.clone(), args.len()))
    }

    #[test]
    fn test_partial_fold_add() {
        let got = partial(
            OpCode::Add,
            vec![val(ov("1")), val(ov("2")), var_node("x"), val(ov("3"))],
        );
        assert_eq!(got, Some((ov("3"), 3)));
        let got = partial(
            OpCode::Multiply,
            vec![
                val(ov("2")),
                val(ov("3")),
                val(ov("4")),
                var_node("x"),
                var_node("y"),
            ],
        );
        assert_eq!(got, Some((ov("24"), 3)));
    }

    /// Folding must leave the evaluator where it would have been: only a
    /// leading run of integers that folds exactly, and only while the call
    /// stays variadic.
    #[test]
    fn partial_folds_that_could_change_the_result_are_left_alone() {
        let max = || val(ov(&i64::MAX.to_string()));
        for (opcode, args) in [
            // Not leading: `x` is accumulated before the literals.
            (
                OpCode::Add,
                vec![var_node("x"), val(ov("1")), val(ov("2")), var_node("y")],
            ),
            // Overflows in the fold.
            (
                OpCode::Add,
                vec![max(), val(ov("1")), var_node("x"), var_node("y")],
            ),
            // Floats round differently regrouped.
            (
                OpCode::Multiply,
                vec![val(ov("0.1")), val(ov("10")), var_node("x"), var_node("y")],
            ),
            // A numeric string coerces, so it is not an integer literal.
            (
                OpCode::Add,
                vec![val(ov("1")), val(ov("\"5\"")), var_node("x"), var_node("y")],
            ),
            // Two arguments would be left: the two-argument form differs.
            (OpCode::Add, vec![val(ov("1")), val(ov("2")), var_node("x")]),
        ] {
            assert_eq!(partial(opcode, args), None);
        }
    }

    #[test]
    fn test_fold_cat_adjacent() {
        let node = builtin(
            OpCode::Concat,
            vec![val(ov("\"hello \"")), val(ov("\"world\"")), var_node("x")],
        );
        let (result, _changed) = fold(node);
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
        let node = builtin(OpCode::Add, vec![val(ov("\"5\"")), var_node("x")]);
        let (result, changed) = fold(node);
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
