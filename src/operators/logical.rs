use serde_json::Value;

use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Logical NOT operator (!)
pub struct NotOperator;

impl Operator for NotOperator {
    fn evaluate(
        &self,
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
}

/// Double NOT operator (!!) - converts to boolean
pub struct DoubleNotOperator;

impl Operator for DoubleNotOperator {
    fn evaluate(
        &self,
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
}

/// Logical AND operator - returns first falsy or last value
pub struct AndOperator;

impl Operator for AndOperator {
    fn evaluate(
        &self,
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
}

/// Logical OR operator - returns first truthy or last value
pub struct OrOperator;

impl Operator for OrOperator {
    fn evaluate(
        &self,
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
}
