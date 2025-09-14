use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Throw operator function - throws an error with a type
#[inline]
pub fn evaluate_throw(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let error_value = if args.is_empty() {
        Value::Null
    } else {
        engine.evaluate_node(&args[0], context)?
    };

    // If the error value is an object with a "type" field, use that as the error
    // Otherwise, convert the value to a string and use it as the error type
    let error_obj = if let Value::Object(map) = &error_value {
        // Check if it's already an error object with a "type" field
        if map.contains_key("type") {
            error_value
        } else {
            // It's a regular object, use it as is
            error_value
        }
    } else if let Value::String(s) = &error_value {
        // Create an error object with the string as the type
        serde_json::json!({"type": s})
    } else {
        // For other types, convert to string and use as type
        serde_json::json!({"type": error_value.to_string()})
    };

    Err(Error::Thrown(error_obj))
}
