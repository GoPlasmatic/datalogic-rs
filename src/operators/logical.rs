use serde_json::Value;

use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Result};

/// Logical NOT operator function (!)
#[inline]
pub fn evaluate_not(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    let value = if args.is_empty() {
        Value::Null
    } else {
        evaluator.evaluate(&args[0], context)?
    };

    Ok(Value::Bool(!is_truthy(&value)))
}

/// Double NOT operator function (!!) - converts to boolean
#[inline]
pub fn evaluate_double_not(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    let value = if args.is_empty() {
        Value::Null
    } else {
        evaluator.evaluate(&args[0], context)?
    };

    Ok(Value::Bool(is_truthy(&value)))
}

/// Logical AND operator function - returns first falsy or last value
#[inline]
pub fn evaluate_and(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    // Check for invalid arguments marker
    if args.len() == 1
        && let Value::Object(obj) = &args[0]
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::error::Error::InvalidArguments(
            "Invalid Arguments".to_string(),
        ));
    }

    if args.is_empty() {
        return Ok(Value::Null);
    }

    let mut last_value = Value::Bool(true);

    for arg in args {
        let value = evaluator.evaluate(arg, context)?;
        if !is_truthy(&value) {
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}

/// Logical OR operator function - returns first truthy or last value
#[inline]
pub fn evaluate_or(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    // Check for invalid arguments marker
    if args.len() == 1
        && let Value::Object(obj) = &args[0]
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::error::Error::InvalidArguments(
            "Invalid Arguments".to_string(),
        ));
    }

    if args.is_empty() {
        return Ok(Value::Null);
    }

    let mut last_value = Value::Bool(false);

    for arg in args {
        let value = evaluator.evaluate(arg, context)?;
        if is_truthy(&value) {
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}
