//! Ceiling operator implementation.
//!
//! Provides the `ceil` operator for rounding numbers up to the nearest integer.
//! Supports both single values and variadic calls returning arrays.

use serde_json::Value;

use super::helpers::get_number_strict;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Ceiling operator function (ceil)
#[inline]
pub fn evaluate_ceil(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Check if we have multiple arguments - if so, return array of ceil values
    if args.len() > 1 {
        let mut results = Vec::new();
        for arg in args {
            let value = engine.evaluate_node_cow(arg, context)?;
            if let Some(num) = get_number_strict(&value) {
                let ceil_val = num.ceil();
                results.push(Value::Number((ceil_val as i64).into()));
            } else {
                return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
            }
        }
        return Ok(Value::Array(results));
    }

    // Single argument - evaluate and return ceil
    let value = engine.evaluate_node_cow(&args[0], context)?;

    if let Some(num) = get_number_strict(&value) {
        let ceil_val = num.ceil();
        Ok(Value::Number((ceil_val as i64).into()))
    } else {
        Err(Error::InvalidArguments(INVALID_ARGS.to_string()))
    }
}
