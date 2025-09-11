use serde_json::Value;

use crate::value_helpers::{coerce_to_number, loose_equals, strict_equals};
use crate::{ContextStack, Evaluator, Operator, Result};

/// Equals operator (== for loose, === for strict)
pub struct EqualsOperator {
    pub strict: bool,
}

impl Operator for EqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(true));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = if self.strict {
            strict_equals(&left, &right)
        } else {
            loose_equals(&left, &right)
        };

        Ok(Value::Bool(result))
    }
}

/// Not equals operator (!= for loose, !== for strict)
pub struct NotEqualsOperator {
    pub strict: bool,
}

impl Operator for NotEqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = if self.strict {
            !strict_equals(&left, &right)
        } else {
            !loose_equals(&left, &right)
        };

        Ok(Value::Bool(result))
    }
}

/// Greater than operator (>)
pub struct GreaterThanOperator;

impl Operator for GreaterThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = match (coerce_to_number(&left), coerce_to_number(&right)) {
            (Some(l), Some(r)) => l > r,
            _ => false,
        };

        Ok(Value::Bool(result))
    }
}

/// Greater than or equal operator (>=)
pub struct GreaterThanEqualOperator;

impl Operator for GreaterThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = match (coerce_to_number(&left), coerce_to_number(&right)) {
            (Some(l), Some(r)) => l >= r,
            _ => false,
        };

        Ok(Value::Bool(result))
    }
}

/// Less than operator (<) - supports variadic arguments
pub struct LessThanOperator;

impl Operator for LessThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            let result = match (coerce_to_number(&prev), coerce_to_number(&current)) {
                (Some(l), Some(r)) => l < r,
                _ => return Ok(Value::Bool(false)),
            };

            if !result {
                return Ok(Value::Bool(false));
            }

            prev = current;
        }

        Ok(Value::Bool(true))
    }
}

/// Less than or equal operator (<=) - supports variadic arguments
pub struct LessThanEqualOperator;

impl Operator for LessThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            let result = match (coerce_to_number(&prev), coerce_to_number(&current)) {
                (Some(l), Some(r)) => l <= r,
                _ => return Ok(Value::Bool(false)),
            };

            if !result {
                return Ok(Value::Bool(false));
            }

            prev = current;
        }

        Ok(Value::Bool(true))
    }
}
