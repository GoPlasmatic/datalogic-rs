//! Strength reduction pass.
//!
//! Replaces expensive patterns with cheaper equivalents: nested
//! truthiness operators collapse into one (`{"!": [{"!": [X]}]}` →
//! `{"!!": [X]}`, `{"!": [{"!!": [X]}]}` → `{"!": [X]}`).

use crate::OpCode;
use crate::node::CompiledNode;
use crate::operators::meta::Algebra;

/// Apply strength reduction to a compiled node.
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Used by the optimiser pipeline to drive fixpoint iteration.
///
/// Nested one-argument truthiness operators (rows with `Algebra::Truth`)
/// collapse into the single operator their composition computes, applied
/// to the inner argument: `!(!x)` → `!!x`, `!!(!!x)` → `!!x`,
/// `!(!!x)` → `!x`, `!!(!x)` → `!x`.
pub(crate) fn reduce(node: CompiledNode) -> (CompiledNode, bool) {
    if let CompiledNode::BuiltinOperator {
        id, opcode, args, ..
    } = &node
        && let Some(Algebra::Truth(outer)) = opcode.algebra()
        && let [
            CompiledNode::BuiltinOperator {
                opcode: inner_opcode,
                args: inner_args,
                ..
            },
        ] = &args[..]
        && let Some(Algebra::Truth(inner)) = inner_opcode.algebra()
        && inner_args.len() == 1
        && let Some(opcode) = OpCode::with_algebra(Algebra::Truth(outer.compose(inner)))
    {
        return (
            CompiledNode::BuiltinOperator {
                id: *id,
                opcode,
                args: inner_args.clone(),
                predicate_hint: None,
                iter_arg_kind: crate::operators::array::IterArgKind::General,
            },
            true,
        );
    }
    (node, false)
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{builtin, var_node};
    use super::*;

    #[test]
    fn test_double_negation() {
        let inner = builtin(OpCode::Not, vec![var_node("x")]);
        let outer = builtin(OpCode::Not, vec![inner]);
        let (result, changed) = reduce(outer);
        assert!(changed);
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = &result {
            assert_eq!(*opcode, OpCode::BoolCast);
            assert_eq!(args.len(), 1);
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    #[test]
    fn test_idempotent_double_not() {
        let inner = builtin(OpCode::BoolCast, vec![var_node("x")]);
        let outer = builtin(OpCode::BoolCast, vec![inner]);
        let (result, changed) = reduce(outer);
        assert!(changed);
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = &result {
            assert_eq!(*opcode, OpCode::BoolCast);
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], CompiledNode::Var { .. }));
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    /// Nested truthiness operators compose: the result negates when exactly
    /// one of the two does, and keeps the inner argument.
    #[test]
    fn truth_compositions_collapse() {
        use OpCode::{BoolCast, Not};
        for (outer, inner, want) in [
            (Not, Not, BoolCast),
            (BoolCast, BoolCast, BoolCast),
            (Not, BoolCast, Not),
            (BoolCast, Not, Not),
        ] {
            let node = builtin(outer, vec![builtin(inner, vec![var_node("x")])]);
            let (result, changed) = reduce(node);
            assert!(changed, "{outer:?}({inner:?}(x))");
            let CompiledNode::BuiltinOperator { opcode, args, .. } = &result else {
                panic!("expected BuiltinOperator");
            };
            assert_eq!(*opcode, want, "{outer:?}({inner:?}(x))");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], CompiledNode::Var { .. }));
        }
    }

    /// Only the one-argument forms compose: `{"!": [a, b]}` reads `a` only
    /// but is left for the runtime arity policy.
    #[test]
    fn truth_with_other_arities_is_unchanged() {
        let inner = builtin(OpCode::Not, vec![var_node("x")]);
        let outer = builtin(OpCode::Not, vec![inner, var_node("y")]);
        assert!(!reduce(outer).1);
        let inner = builtin(OpCode::Not, vec![var_node("x"), var_node("y")]);
        let outer = builtin(OpCode::Not, vec![inner]);
        assert!(!reduce(outer).1);
    }

    #[test]
    fn test_unchanged_when_no_pattern() {
        let node = var_node("x");
        let (_result, changed) = reduce(node);
        assert!(!changed);
    }
}
