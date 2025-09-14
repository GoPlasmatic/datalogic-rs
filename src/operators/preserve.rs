use serde_json::Value;

use crate::{ContextStack, Evaluator, Result};

/// Preserve operator function - returns its argument unchanged
#[inline]
pub fn evaluate_preserve(
    args: &[Value],
    _context: &mut ContextStack,
    _evaluator: &dyn Evaluator,
) -> Result<Value> {
    // Preserve returns its arguments unchanged
    // - With no arguments: return empty array
    // - With one argument: return that argument
    // - With multiple arguments: return array of arguments
    match args.len() {
        0 => Ok(Value::Array(vec![])),
        1 => Ok(args[0].clone()),
        _ => Ok(Value::Array(args.to_vec())),
    }
}
