use serde_json::Value;
use std::borrow::Cow;

use crate::value_helpers::{coerce_to_number, loose_equals, strict_equals};
use crate::{ContextStack, Evaluator, Operator, Result};

/// Equals operator (== for loose, === for strict)
pub struct EqualsOperator {
    pub strict: bool,
}

impl Operator for EqualsOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(true)));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = if self.strict {
            strict_equals(left.as_ref(), right.as_ref())
        } else {
            loose_equals(left.as_ref(), right.as_ref())
        };

        Ok(Cow::Owned(Value::Bool(result)))
    }
}

/// Not equals operator (!= for loose, !== for strict)
pub struct NotEqualsOperator {
    pub strict: bool,
}

impl Operator for NotEqualsOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = if self.strict {
            !strict_equals(left.as_ref(), right.as_ref())
        } else {
            !loose_equals(left.as_ref(), right.as_ref())
        };

        Ok(Cow::Owned(Value::Bool(result)))
    }
}

/// Greater than operator (>)
pub struct GreaterThanOperator;

impl Operator for GreaterThanOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = match (
            coerce_to_number(left.as_ref()),
            coerce_to_number(right.as_ref()),
        ) {
            (Some(l), Some(r)) => l > r,
            _ => false,
        };

        Ok(Cow::Owned(Value::Bool(result)))
    }
}

/// Greater than or equal operator (>=)
pub struct GreaterThanEqualOperator;

impl Operator for GreaterThanEqualOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        let result = match (
            coerce_to_number(left.as_ref()),
            coerce_to_number(right.as_ref()),
        ) {
            (Some(l), Some(r)) => l >= r,
            _ => false,
        };

        Ok(Cow::Owned(Value::Bool(result)))
    }
}

/// Less than operator (<) - supports variadic arguments
pub struct LessThanOperator;

impl Operator for LessThanOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            let result = match (
                coerce_to_number(prev.as_ref()),
                coerce_to_number(current.as_ref()),
            ) {
                (Some(l), Some(r)) => l < r,
                _ => return Ok(Cow::Owned(Value::Bool(false))),
            };

            if !result {
                return Ok(Cow::Owned(Value::Bool(false)));
            }

            prev = current;
        }

        Ok(Cow::Owned(Value::Bool(true)))
    }
}

/// Less than or equal operator (<=) - supports variadic arguments
pub struct LessThanEqualOperator;

impl Operator for LessThanEqualOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            let result = match (
                coerce_to_number(prev.as_ref()),
                coerce_to_number(current.as_ref()),
            ) {
                (Some(l), Some(r)) => l <= r,
                _ => return Ok(Cow::Owned(Value::Bool(false))),
            };

            if !result {
                return Ok(Cow::Owned(Value::Bool(false)));
            }

            prev = current;
        }

        Ok(Cow::Owned(Value::Bool(true)))
    }
}
