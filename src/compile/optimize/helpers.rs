//! Shared helper functions for optimization passes.

use crate::DataLogic;
use crate::node::CompiledNode;
use crate::operators::helpers::is_truthy;

/// Check if a compiled node is a literal value and determine its truthiness.
/// Returns `Some(true)` / `Some(false)` for static values, `None` for dynamic nodes.
///
/// Uses the engine's configured truthiness evaluator (JavaScript, Python, StrictBoolean, Custom).
pub fn is_truthy_literal(node: &CompiledNode, engine: &DataLogic) -> Option<bool> {
    match node {
        CompiledNode::Value { value } => Some(is_truthy(value, engine)),
        _ => None,
    }
}
