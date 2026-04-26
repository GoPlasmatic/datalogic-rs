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
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Only transforms the top-level node — recursive application
/// is driven by the optimiser pipeline's fixpoint loop.
pub fn eliminate(node: CompiledNode, engine: &DataLogic) -> (CompiledNode, bool) {
    match &node {
        CompiledNode::BuiltinOperator { id, opcode, args } => {
            let rewritten = match opcode {
                OpCode::If => eliminate_if(*id, args, engine),
                OpCode::Ternary => eliminate_ternary(args, engine),
                OpCode::And => eliminate_and(*id, args, engine),
                OpCode::Or => eliminate_or(*id, args, engine),
                _ => None,
            };
            match rewritten {
                Some(new_node) => (new_node, true),
                None => (node, false),
            }
        }
        _ => (node, false),
    }
}

/// Eliminate dead branches in if/elseif/else chains.
/// Returns `Some(new_node)` if the input was rewritten, `None` otherwise.
fn eliminate_if(outer_id: u32, args: &[CompiledNode], engine: &DataLogic) -> Option<CompiledNode> {
    if args.is_empty() {
        return Some(CompiledNode::synthetic_value(Value::Null));
    }

    let mut i = 0;
    let mut new_args: Vec<CompiledNode> = Vec::new();
    let mut skipped_any = false;

    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause — keep it
            if new_args.is_empty() {
                // All previous conditions were false → this is the result
                return Some(args[i].clone());
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
                        return Some(args[i + 1].clone());
                    }
                    // We had prior non-static conditions; this becomes the else
                    new_args.push(args[i + 1].clone());
                    skipped_any = true;
                    break;
                }
                return Some(args[i].clone());
            }
            Some(false) => {
                // Condition is statically false → skip this condition+then pair
                skipped_any = true;
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
        return Some(CompiledNode::synthetic_value(Value::Null));
    }

    if new_args.len() == 1 {
        // Single remaining element is the else clause
        return Some(new_args.into_iter().next().unwrap());
    }

    if !skipped_any && new_args.len() == args.len() {
        // No-op rebuild — leave input untouched
        return None;
    }

    Some(CompiledNode::BuiltinOperator {
        id: outer_id,
        opcode: OpCode::If,
        args: new_args.into_boxed_slice(),
    })
}

/// Eliminate dead branches in ternary (`?:`) operator.
fn eliminate_ternary(args: &[CompiledNode], engine: &DataLogic) -> Option<CompiledNode> {
    if args.len() < 3 {
        return None;
    }

    match is_truthy_literal(&args[0], engine) {
        Some(true) => Some(args[1].clone()),
        Some(false) => Some(args[2].clone()),
        None => None,
    }
}

/// Eliminate identity/absorbing elements in `and`.
fn eliminate_and(outer_id: u32, args: &[CompiledNode], engine: &DataLogic) -> Option<CompiledNode> {
    if args.is_empty() {
        return None;
    }

    let mut remaining: Vec<CompiledNode> = Vec::new();

    for arg in args {
        match is_truthy_literal(arg, engine) {
            Some(false) => {
                // Absorbing element — and short-circuits, returns the falsy value
                return Some(arg.clone());
            }
            Some(true) => {
                // Identity element — skip (and returns the value, not bool)
                continue;
            }
            None => {
                remaining.push(arg.clone());
            }
        }
    }

    if remaining.is_empty() {
        // All elements were truthy literals — return the last one
        return Some(args.last().unwrap().clone());
    }

    if remaining.len() == 1 {
        return Some(remaining.into_iter().next().unwrap());
    }

    if remaining.len() == args.len() {
        // Nothing stripped — no change
        return None;
    }

    Some(CompiledNode::BuiltinOperator {
        id: outer_id,
        opcode: OpCode::And,
        args: remaining.into_boxed_slice(),
    })
}

/// Eliminate identity/absorbing elements in `or`.
fn eliminate_or(outer_id: u32, args: &[CompiledNode], engine: &DataLogic) -> Option<CompiledNode> {
    if args.is_empty() {
        return None;
    }

    let mut remaining: Vec<CompiledNode> = Vec::new();

    for arg in args {
        match is_truthy_literal(arg, engine) {
            Some(true) => {
                // Absorbing element — or short-circuits, returns the truthy value
                return Some(arg.clone());
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
        return Some(args.last().unwrap().clone());
    }

    if remaining.len() == 1 {
        return Some(remaining.into_iter().next().unwrap());
    }

    if remaining.len() == args.len() {
        return None;
    }

    Some(CompiledNode::BuiltinOperator {
        id: outer_id,
        opcode: OpCode::Or,
        args: remaining.into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{builtin, val, var_node};
    use super::*;
    use serde_json::json;

    #[test]
    fn test_if_true_condition() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::If,
            vec![val(json!(true)), var_node("x"), var_node("y")],
        );
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_if_false_condition() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::If,
            vec![val(json!(false)), var_node("x"), var_node("y")],
        );
        let (result, _changed) = eliminate(node, &engine);
        // Should return "y" (the else branch)
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_and_with_true_prefix() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::And, vec![val(json!(true)), var_node("x")]);
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_and_with_false() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::And, vec![val(json!(false)), var_node("x")]);
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::Value { .. }));
    }

    #[test]
    fn test_or_with_true() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::Or, vec![val(json!(true)), var_node("x")]);
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::Value { .. }));
    }

    #[test]
    fn test_or_with_false_prefix() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::Or, vec![val(json!(false)), var_node("x")]);
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }

    #[test]
    fn test_ternary_true() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::Ternary,
            vec![val(json!(true)), var_node("x"), var_node("y")],
        );
        let (result, _changed) = eliminate(node, &engine);
        assert!(matches!(result, CompiledNode::CompiledVar { .. }));
    }
}
