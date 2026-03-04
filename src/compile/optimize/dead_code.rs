//! Dead code elimination pass.
//!
//! Eliminates unreachable branches when conditions are compile-time constants:
//! - `{"if": [true, A, B]}` → `A`
//! - `{"?:": [false, A, B]}` → `B`
//! - `{"and": [true, X]}` → `X` (strip identity elements)
//! - `{"and": [false, X]}` → `false` (absorbing element)
//! - `{"or": [true, X]}` → `true` (absorbing element)
//! - `{"or": [false, X]}` → `X` (strip identity elements)

use crate::DataLogic;
use crate::node::CompiledNode;
use crate::opcode::OpCode;
use serde_json::Value;

use super::helpers::is_truthy_literal;

/// Eliminate dead branches from a compiled node tree.
/// Only transforms the top-level node — recursive application is handled by the pipeline.
pub fn eliminate(node: CompiledNode, engine: &DataLogic) -> CompiledNode {
    match &node {
        CompiledNode::BuiltinOperator { opcode, args } => match opcode {
            OpCode::If => eliminate_if(args, engine),
            OpCode::Ternary => eliminate_ternary(args, engine),
            OpCode::And => eliminate_and(args, engine),
            OpCode::Or => eliminate_or(args, engine),
            _ => node,
        },
        _ => node,
    }
}

/// Eliminate dead branches in if/elseif/else chains.
/// `{"if": [true, A, B]}` → `A`
/// `{"if": [false, A, true, B, C]}` → `B`
fn eliminate_if(args: &[CompiledNode], engine: &DataLogic) -> CompiledNode {
    if args.is_empty() {
        return CompiledNode::Value { value: Value::Null };
    }

    let mut i = 0;
    let mut new_args: Vec<CompiledNode> = Vec::new();

    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause — keep it
            if new_args.is_empty() {
                // All previous conditions were false → this is the result
                return args[i].clone();
            }
            new_args.push(args[i].clone());
            break;
        }

        // Check if condition is a static value
        match is_truthy_literal(&args[i], engine) {
            Some(true) => {
                // Condition is statically true → return the then-branch
                if i + 1 < args.len() {
                    if new_args.is_empty() {
                        return args[i + 1].clone();
                    }
                    // We had prior non-static conditions; this becomes the else
                    new_args.push(args[i + 1].clone());
                    break;
                }
                return args[i].clone();
            }
            Some(false) => {
                // Condition is statically false → skip this condition+then pair
                i += 2;
                continue;
            }
            None => {
                // Non-static condition — keep it
                new_args.push(args[i].clone());
                if i + 1 < args.len() {
                    new_args.push(args[i + 1].clone());
                }
                i += 2;
            }
        }
    }

    if new_args.is_empty() {
        // All conditions were statically false, no else clause
        return CompiledNode::Value { value: Value::Null };
    }

    if new_args.len() == 1 {
        // Single remaining element is the else clause
        return new_args.into_iter().next().unwrap();
    }

    CompiledNode::BuiltinOperator {
        opcode: OpCode::If,
        args: new_args.into_boxed_slice(),
    }
}

/// Eliminate dead branches in ternary (`?:`) operator.
/// `{"?:": [true, A, B]}` → `A`
/// `{"?:": [false, A, B]}` → `B`
fn eliminate_ternary(args: &[CompiledNode], engine: &DataLogic) -> CompiledNode {
    if args.len() < 3 {
        return CompiledNode::BuiltinOperator {
            opcode: OpCode::Ternary,
            args: args.to_vec().into_boxed_slice(),
        };
    }

    match is_truthy_literal(&args[0], engine) {
        Some(true) => args[1].clone(),
        Some(false) => args[2].clone(),
        None => CompiledNode::BuiltinOperator {
            opcode: OpCode::Ternary,
            args: args.to_vec().into_boxed_slice(),
        },
    }
}

/// Eliminate identity/absorbing elements in `and`.
/// `{"and": [true, X]}` → `X`
/// `{"and": [false, X]}` → `false`
fn eliminate_and(args: &[CompiledNode], engine: &DataLogic) -> CompiledNode {
    if args.is_empty() {
        return CompiledNode::BuiltinOperator {
            opcode: OpCode::And,
            args: args.to_vec().into_boxed_slice(),
        };
    }

    let mut remaining: Vec<CompiledNode> = Vec::new();

    for arg in args {
        match is_truthy_literal(arg, engine) {
            Some(false) => {
                // Absorbing element — and short-circuits, returns the falsy value
                return arg.clone();
            }
            Some(true) => {
                // Identity element — skip (and returns the value, not bool)
                // But if this is the last one, we need it as the result
                continue;
            }
            None => {
                remaining.push(arg.clone());
            }
        }
    }

    if remaining.is_empty() {
        // All elements were truthy literals — return the last one
        return args.last().unwrap().clone();
    }

    if remaining.len() == 1 {
        return remaining.into_iter().next().unwrap();
    }

    // Check if we actually stripped anything
    if remaining.len() == args.len() {
        return CompiledNode::BuiltinOperator {
            opcode: OpCode::And,
            args: args.to_vec().into_boxed_slice(),
        };
    }

    CompiledNode::BuiltinOperator {
        opcode: OpCode::And,
        args: remaining.into_boxed_slice(),
    }
}

/// Eliminate identity/absorbing elements in `or`.
/// `{"or": [true, X]}` → `true`
/// `{"or": [false, X]}` → `X`
fn eliminate_or(args: &[CompiledNode], engine: &DataLogic) -> CompiledNode {
    if args.is_empty() {
        return CompiledNode::BuiltinOperator {
            opcode: OpCode::Or,
            args: args.to_vec().into_boxed_slice(),
        };
    }

    let mut remaining: Vec<CompiledNode> = Vec::new();

    for arg in args {
        match is_truthy_literal(arg, engine) {
            Some(true) => {
                // Absorbing element — or short-circuits, returns the truthy value
                return arg.clone();
            }
            Some(false) => {
                // Identity element — skip
                continue;
            }
            None => {
                remaining.push(arg.clone());
            }
        }
    }

    if remaining.is_empty() {
        // All elements were falsy literals — return the last one
        return args.last().unwrap().clone();
    }

    if remaining.len() == 1 {
        return remaining.into_iter().next().unwrap();
    }

    if remaining.len() == args.len() {
        return CompiledNode::BuiltinOperator {
            opcode: OpCode::Or,
            args: args.to_vec().into_boxed_slice(),
        };
    }

    CompiledNode::BuiltinOperator {
        opcode: OpCode::Or,
        args: remaining.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn val(v: Value) -> CompiledNode {
        CompiledNode::Value { value: v }
    }

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
    fn test_if_true_condition() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::If,
            vec![val(json!(true)), var_node("x"), var_node("y")],
        );
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_if_false_condition() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::If,
            vec![val(json!(false)), var_node("x"), var_node("y")],
        );
        let result = eliminate(node, &engine);
        // Should return "y" (the else branch)
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_and_with_true_prefix() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::And, vec![val(json!(true)), var_node("x")]);
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_and_with_false() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::And, vec![val(json!(false)), var_node("x")]);
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::Value { .. }));
    }

    #[test]
    fn test_or_with_true() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::Or, vec![val(json!(true)), var_node("x")]);
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::Value { .. }));
    }

    #[test]
    fn test_or_with_false_prefix() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::Or, vec![val(json!(false)), var_node("x")]);
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_ternary_true() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::Ternary,
            vec![val(json!(true)), var_node("x"), var_node("y")],
        );
        let result = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }
}
