//! Strength reduction pass.
//!
//! Replaces expensive patterns with cheaper equivalents:
//! - `{"!": [{"!": [X]}]}` → `{"!!": [X]}` (double negation → bool coerce)
//! - `{"!!": [{"!!": [X]}]}` → `{"!!": [X]}` (idempotent bool coerce)

use crate::node::CompiledNode;
use crate::opcode::OpCode;

/// Apply strength reduction to a compiled node.
pub fn reduce(node: CompiledNode) -> CompiledNode {
    match &node {
        CompiledNode::BuiltinOperator { opcode, args } => {
            match opcode {
                OpCode::Not if args.len() == 1 => {
                    // Check if inner is also Not → collapse to DoubleNot
                    if let CompiledNode::BuiltinOperator {
                        opcode: OpCode::Not,
                        args: inner_args,
                    } = &args[0]
                        && inner_args.len() == 1
                    {
                        return CompiledNode::BuiltinOperator {
                            opcode: OpCode::DoubleNot,
                            args: inner_args.clone(),
                        };
                    }
                    node
                }
                OpCode::DoubleNot if args.len() == 1 => {
                    // Check if inner is also DoubleNot → collapse (idempotent)
                    if let CompiledNode::BuiltinOperator {
                        opcode: OpCode::DoubleNot,
                        args: inner_args,
                    } = &args[0]
                        && inner_args.len() == 1
                    {
                        return CompiledNode::BuiltinOperator {
                            opcode: OpCode::DoubleNot,
                            args: inner_args.clone(),
                        };
                    }
                    node
                }
                _ => node,
            }
        }
        _ => node,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var_node(name: &str) -> CompiledNode {
        CompiledNode::CompiledVar {
            scope_level: 0,
            segments: vec![crate::node::PathSegment::Field(name.into())].into_boxed_slice(),
            reduce_hint: crate::node::ReduceHint::None,
            metadata_hint: crate::node::MetadataHint::None,
            default_value: None,
        }
    }

    fn builtin(opcode: OpCode, args: Vec<CompiledNode>) -> CompiledNode {
        CompiledNode::BuiltinOperator {
            opcode,
            args: args.into_boxed_slice(),
        }
    }

    #[test]
    fn test_double_negation() {
        let inner = builtin(OpCode::Not, vec![var_node("x")]);
        let outer = builtin(OpCode::Not, vec![inner]);
        let result = reduce(outer);
        if let CompiledNode::BuiltinOperator { opcode, args } = &result {
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
        let result = reduce(outer);
        if let CompiledNode::BuiltinOperator { opcode, args } = &result {
            assert_eq!(*opcode, OpCode::DoubleNot);
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], CompiledNode::CompiledVar { .. }));
        } else {
            panic!("expected BuiltinOperator");
        }
    }
}
