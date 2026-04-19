//! Strength reduction pass.
//!
//! Replaces expensive patterns with cheaper equivalents:
//! - `{"!": [{"!": [X]}]}` → `{"!!": [X]}` (double negation → bool coerce)
//! - `{"!!": [{"!!": [X]}]}` → `{"!!": [X]}` (idempotent bool coerce)

use crate::node::CompiledNode;
use crate::opcode::OpCode;

/// Apply strength reduction to a compiled node.
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Used by the optimiser pipeline to drive fixpoint iteration.
pub fn reduce(node: CompiledNode) -> (CompiledNode, bool) {
    match &node {
        CompiledNode::BuiltinOperator { id, opcode, args } => match opcode {
            OpCode::Not if args.len() == 1 => {
                // Check if inner is also Not → collapse to DoubleNot
                if let CompiledNode::BuiltinOperator {
                    opcode: OpCode::Not,
                    args: inner_args,
                    ..
                } = &args[0]
                    && inner_args.len() == 1
                {
                    return (
                        CompiledNode::BuiltinOperator {
                            id: *id,
                            opcode: OpCode::DoubleNot,
                            args: inner_args.clone(),
                        },
                        true,
                    );
                }
                (node, false)
            }
            OpCode::DoubleNot if args.len() == 1 => {
                // Check if inner is also DoubleNot → collapse (idempotent)
                if let CompiledNode::BuiltinOperator {
                    opcode: OpCode::DoubleNot,
                    args: inner_args,
                    ..
                } = &args[0]
                    && inner_args.len() == 1
                {
                    return (
                        CompiledNode::BuiltinOperator {
                            id: *id,
                            opcode: OpCode::DoubleNot,
                            args: inner_args.clone(),
                        },
                        true,
                    );
                }
                (node, false)
            }
            _ => (node, false),
        },
        _ => (node, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::SYNTHETIC_ID;

    fn var_node(name: &str) -> CompiledNode {
        CompiledNode::CompiledVar {
            id: SYNTHETIC_ID,
            scope_level: 0,
            segments: vec![crate::node::PathSegment::Field(name.into())].into_boxed_slice(),
            reduce_hint: crate::node::ReduceHint::None,
            metadata_hint: crate::node::MetadataHint::None,
            default_value: None,
        }
    }

    fn builtin(opcode: OpCode, args: Vec<CompiledNode>) -> CompiledNode {
        CompiledNode::BuiltinOperator {
            id: SYNTHETIC_ID,
            opcode,
            args: args.into_boxed_slice(),
        }
    }

    #[test]
    fn test_double_negation() {
        let inner = builtin(OpCode::Not, vec![var_node("x")]);
        let outer = builtin(OpCode::Not, vec![inner]);
        let (result, changed) = reduce(outer);
        assert!(changed);
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = &result {
            assert_eq!(*opcode, OpCode::DoubleNot);
            assert_eq!(args.len(), 1);
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    #[test]
    fn test_idempotent_double_not() {
        let inner = builtin(OpCode::DoubleNot, vec![var_node("x")]);
        let outer = builtin(OpCode::DoubleNot, vec![inner]);
        let (result, changed) = reduce(outer);
        assert!(changed);
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = &result {
            assert_eq!(*opcode, OpCode::DoubleNot);
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], CompiledNode::CompiledVar { .. }));
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    #[test]
    fn test_unchanged_when_no_pattern() {
        let node = var_node("x");
        let (_result, changed) = reduce(node);
        assert!(!changed);
    }
}
