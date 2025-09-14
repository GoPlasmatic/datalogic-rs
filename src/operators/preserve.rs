use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Preserve operator function - returns its argument unchanged
#[inline]
pub fn evaluate_preserve(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Preserve evaluates and returns its arguments
    // - With no arguments: return empty array
    // - With one argument: return that argument evaluated
    // - With multiple arguments: return array of evaluated arguments
    match args.len() {
        0 => Ok(Value::Array(vec![])),
        1 => engine.evaluate_node(&args[0], context),
        _ => {
            let mut results = Vec::with_capacity(args.len());
            for arg in args {
                results.push(engine.evaluate_node(arg, context)?);
            }
            Ok(Value::Array(results))
        }
    }
}
