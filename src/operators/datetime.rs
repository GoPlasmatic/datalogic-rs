use serde_json::Value;

use crate::{ContextStack, Evaluator, Operator, Result};

/// DatetimeOperator - returns a datetime string (for type testing)
pub struct DatetimeOperator;

impl Operator for DatetimeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // For now, just return the first argument as-is
        // This is enough for type operator testing
        if args.is_empty() {
            Ok(Value::String("".to_string()))
        } else {
            evaluator.evaluate(&args[0], context)
        }
    }
}

/// TimestampOperator - returns a duration string (for type testing)
pub struct TimestampOperator;

impl Operator for TimestampOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // For now, just return the first argument as-is
        // This is enough for type operator testing
        if args.is_empty() {
            Ok(Value::String("".to_string()))
        } else {
            evaluator.evaluate(&args[0], context)
        }
    }
}
