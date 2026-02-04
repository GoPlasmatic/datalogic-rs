//! Floor operator implementation.
//!
//! Provides the `floor` operator for rounding numbers down to the nearest integer.
//! Supports both single values and variadic calls returning arrays.

use serde_json::Value;

use super::helpers::get_number_strict;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Floor operator function (floor)
#[inline]
pub fn evaluate_floor(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Check if we have multiple arguments - if so, return array of floor values
    if args.len() > 1 {
        let mut results = Vec::new();
        for arg in args {
            let value = engine.evaluate_node_cow(arg, context)?;
            if let Some(num) = get_number_strict(&value) {
                let floor_val = num.floor();
                results.push(Value::Number((floor_val as i64).into()));
            } else {
                return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
            }
        }
        return Ok(Value::Array(results));
    }

    // Single argument - evaluate and return floor
    let value = engine.evaluate_node_cow(&args[0], context)?;

    if let Some(num) = get_number_strict(&value) {
        let floor_val = num.floor();
        Ok(Value::Number((floor_val as i64).into()))
    } else {
        Err(Error::InvalidArguments(INVALID_ARGS.to_string()))
    }
}
