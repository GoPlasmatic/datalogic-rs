//! Absolute value operator implementation.
//!
//! Provides the `abs` operator for computing absolute values of numbers.
//! Supports both single values and variadic calls returning arrays.

use serde_json::Value;

use super::helpers::get_number_strict;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Absolute value operator function (abs)
#[inline]
pub fn evaluate_abs(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Check if we have multiple arguments - if so, return array of abs values
    if args.len() > 1 {
        let mut results = Vec::new();
        for arg in args {
            let value = engine.evaluate_node(arg, context)?;
            if let Some(num) = get_number_strict(&value) {
                let abs_val = num.abs();
                // Try to keep as integer if possible
                let int_val = abs_val as i64;
                if int_val as f64 == abs_val {
                    results.push(Value::Number(int_val.into()));
                } else {
                    results.push(
                        serde_json::Number::from_f64(abs_val)
                            .map(Value::Number)
                            .unwrap_or(Value::Null),
                    );
                }
            } else {
                return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
            }
        }
        return Ok(Value::Array(results));
    }

    // Single argument - evaluate and return abs
    let value = engine.evaluate_node(&args[0], context)?;

    if let Some(num) = get_number_strict(&value) {
        let abs_val = num.abs();
        // Try to keep as integer if possible
        let int_val = abs_val as i64;
        if int_val as f64 == abs_val {
            return Ok(Value::Number(int_val.into()));
        }
        Ok(serde_json::Number::from_f64(abs_val)
            .map(Value::Number)
            .unwrap_or(Value::Null))
    } else {
        Err(Error::InvalidArguments(INVALID_ARGS.to_string()))
    }
}
