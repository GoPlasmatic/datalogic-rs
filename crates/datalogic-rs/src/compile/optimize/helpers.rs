//! Shared helper functions for optimization passes.

use crate::Engine;
use crate::node::CompiledNode;
use crate::operators::truthy::truthy_owned;

/// Check if a compiled node is a literal value and determine its truthiness.
/// Returns `Some(true)` / `Some(false)` for static values, `None` for dynamic nodes.
///
/// Uses the engine's configured truthiness evaluator (JavaScript, Python, StrictBoolean, Custom).
pub(super) fn is_truthy_literal(node: &CompiledNode, engine: &Engine) -> Option<bool> {
    match node {
        CompiledNode::Value { value, .. } => Some(truthy_owned(value, engine)),
        _ => None,
    }
}
