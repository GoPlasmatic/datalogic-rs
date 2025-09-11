use serde_json::Value;
use std::borrow::Cow;

use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Logical NOT operator (!)
pub struct NotOperator;

impl Operator for NotOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        let value = if args.is_empty() {
            Cow::Owned(Value::Null)
        } else {
            evaluator.evaluate(&args[0], context)?
        };

        Ok(Cow::Owned(Value::Bool(!is_truthy(value.as_ref()))))
    }
}

/// Double NOT operator (!!) - converts to boolean
pub struct DoubleNotOperator;

impl Operator for DoubleNotOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        let value = if args.is_empty() {
            Cow::Owned(Value::Null)
        } else {
            evaluator.evaluate(&args[0], context)?
        };

        Ok(Cow::Owned(Value::Bool(is_truthy(value.as_ref()))))
    }
}

/// Logical AND operator - returns first falsy or last value
pub struct AndOperator;

impl Operator for AndOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Bool(true)));
        }

        let mut last_value = Cow::Owned(Value::Bool(true));

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            if !is_truthy(value.as_ref()) {
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
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let mut last_value = Cow::Owned(Value::Bool(false));

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            if is_truthy(value.as_ref()) {
                return Ok(value);
            }
            last_value = value;
        }

        Ok(last_value)
    }
}
