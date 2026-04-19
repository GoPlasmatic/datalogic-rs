//! Partial constant folding pass.
//!
//! Folds static arguments in commutative/associative operators:
//! - `{"+": [1, 2, {"var": "x"}, 3]}` → `{"+": [6, {"var": "x"}]}`
//! - `{"cat": ["hello ", "world", {"var": "name"}]}` → `{"cat": ["hello world", {"var": "name"}]}`
//! - `{"*": [2, {"var": "x"}, 5]}` → `{"*": [10, {"var": "x"}]}`
//!
//! Also pre-coerces numeric string literals in arithmetic contexts.

use crate::node::{CompiledNode, node_is_static};
use crate::opcode::OpCode;
use crate::{ContextStack, DataLogic};
use serde_json::Value;
use std::sync::Arc;

/// Apply partial constant folding to a compiled node.
///
/// Returns `(node, changed)` where `changed` is `true` if the pass rewrote
/// the input. Used by the optimiser pipeline to drive fixpoint iteration.
pub fn fold(node: CompiledNode, engine: &DataLogic) -> (CompiledNode, bool) {
    match &node {
        CompiledNode::BuiltinOperator { .. } => {
            // First: pre-coerce numeric strings in arithmetic operators
            let (node, coerced) = match precoerce_numeric_strings(&node) {
                Some(new) => (new, true),
                None => (node, false),
            };

            match &node {
                CompiledNode::BuiltinOperator { opcode, args } => {
                    // Partial fold for commutative operators with mixed static/dynamic args
                    if is_commutative(opcode) && args.len() >= 2 {
                        match try_partial_fold(*opcode, args, engine) {
                            Some(new) => (new, true),
                            None => (node, coerced),
                        }
                    } else if *opcode == OpCode::Cat && args.len() >= 2 {
                        match try_fold_cat(args) {
                            Some(new) => (new, true),
                            None => (node, coerced),
                        }
                    } else {
                        (node, coerced)
                    }
                }
                _ => (node, coerced),
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
    opcode: OpCode,
    args: &[CompiledNode],
    engine: &DataLogic,
) -> Option<CompiledNode> {
    let mut static_args: Vec<CompiledNode> = Vec::new();
    let mut dynamic_args: Vec<CompiledNode> = Vec::new();

    for arg in args {
        if node_is_static(arg) {
            static_args.push(arg.clone());
        } else {
            dynamic_args.push(arg.clone());
        }
    }

    // Need at least 2 static args to fold, and at least 1 dynamic to be "partial"
    if static_args.len() < 2 || dynamic_args.is_empty() {
        return None;
    }

    // Evaluate the static portion
    let static_node = CompiledNode::BuiltinOperator {
        opcode,
        args: static_args.into_boxed_slice(),
    };
    let mut context = ContextStack::new(Arc::new(Value::Null));
    let folded_value = engine.evaluate_node(&static_node, &mut context).ok()?;

    // Reconstruct: [folded_constant, ...dynamic_args]
    let mut new_args = Vec::with_capacity(1 + dynamic_args.len());
    new_args.push(CompiledNode::Value {
        value: folded_value,
    });
    new_args.extend(dynamic_args);

    Some(CompiledNode::BuiltinOperator {
        opcode,
        args: new_args.into_boxed_slice(),
    })
}

/// Try to fold adjacent static strings in cat operator.
/// `{"cat": ["hello ", "world", {"var": "x"}]}` → `{"cat": ["hello world", {"var": "x"}]}`
fn try_fold_cat(args: &[CompiledNode]) -> Option<CompiledNode> {
    let mut new_args: Vec<CompiledNode> = Vec::new();
    let mut current_static_str: Option<String> = None;
    let mut folded_any = false;

    for arg in args {
        if let CompiledNode::Value {
            value: Value::String(s),
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
                new_args.push(CompiledNode::Value {
                    value: Value::String(s),
                });
            }
            new_args.push(arg.clone());
        }
    }

    // Flush final accumulated string
    if let Some(s) = current_static_str.take() {
        new_args.push(CompiledNode::Value {
            value: Value::String(s),
        });
    }

    if !folded_any {
        return None;
    }

    if new_args.len() == 1 {
        // Entire cat was static strings
        return Some(new_args.into_iter().next().unwrap());
    }

    Some(CompiledNode::BuiltinOperator {
        opcode: OpCode::Cat,
        args: new_args.into_boxed_slice(),
    })
}

/// Pre-coerce numeric string literals in arithmetic contexts.
/// `{"+": ["5", {"var": "x"}]}` → `{"+": [5, {"var": "x"}]}`.
/// Returns `Some(new_node)` if any string was coerced, `None` otherwise.
fn precoerce_numeric_strings(node: &CompiledNode) -> Option<CompiledNode> {
    if let CompiledNode::BuiltinOperator { opcode, args } = node {
        if !is_arithmetic(opcode) {
            return None;
        }

        let mut changed = false;
        let new_args: Vec<CompiledNode> = args
            .iter()
            .map(|arg| {
                if let CompiledNode::Value {
                    value: Value::String(s),
                } = arg
                {
                    // Try parsing as integer first, then float
                    if let Ok(i) = s.parse::<i64>() {
                        changed = true;
                        CompiledNode::Value {
                            value: Value::Number(i.into()),
                        }
                    } else if let Ok(f) = s.parse::<f64>() {
                        if f.is_finite() {
                            if let Some(n) = serde_json::Number::from_f64(f) {
                                changed = true;
                                CompiledNode::Value {
                                    value: Value::Number(n),
                                }
                            } else {
                                arg.clone()
                            }
                        } else {
                            arg.clone()
                        }
                    } else {
                        arg.clone()
                    }
                } else {
                    arg.clone()
                }
            })
            .collect();

        if changed {
            return Some(CompiledNode::BuiltinOperator {
                opcode: *opcode,
                args: new_args.into_boxed_slice(),
            });
        }
    }

    None
}

fn is_arithmetic(opcode: &OpCode) -> bool {
    matches!(
        opcode,
        OpCode::Add | OpCode::Subtract | OpCode::Multiply | OpCode::Divide | OpCode::Modulo
    )
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
    fn test_partial_fold_add() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::Add,
            vec![val(json!(1)), val(json!(2)), var_node("x"), val(json!(3))],
        );
        let (result, _changed) = fold(node, &engine);
        if let CompiledNode::BuiltinOperator { args, .. } = &result {
            assert_eq!(args.len(), 2);
            if let CompiledNode::Value { value } = &args[0] {
                assert_eq!(*value, json!(6));
            } else {
                panic!("expected folded value");
            }
        } else {
            panic!("expected BuiltinOperator");
        }
    }

    #[test]
    fn test_fold_cat_adjacent() {
        let engine = DataLogic::new();
        let node = builtin(
            OpCode::Cat,
            vec![val(json!("hello ")), val(json!("world")), var_node("x")],
        );
        let (result, _changed) = fold(node, &engine);
        if let CompiledNode::BuiltinOperator { args, .. } = &result {
            assert_eq!(args.len(), 2);
            if let CompiledNode::Value { value } = &args[0] {
                assert_eq!(*value, json!("hello world"));
            }
        }
    }

    #[test]
    fn test_precoerce_numeric_string() {
        let engine = DataLogic::new();
        let node = builtin(OpCode::Add, vec![val(json!("5")), var_node("x")]);
        let (result, _changed) = fold(node, &engine);
        if let CompiledNode::BuiltinOperator { args, .. } = &result
            && let CompiledNode::Value { value } = &args[0]
        {
            assert_eq!(*value, json!(5));
        }
    }
}
