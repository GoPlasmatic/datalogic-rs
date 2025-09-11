use serde_json::Value;

use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Operator, Result};

/// If operator - supports if/then/else and if/elseif/else chains
pub struct IfOperator;

impl Operator for IfOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        // Support variadic if/elseif/else chains
        let mut i = 0;
        while i < args.len() {
            if i == args.len() - 1 {
                // Final else clause
                return evaluator.evaluate(&args[i], context);
            }

            // Evaluate condition
            let condition = evaluator.evaluate(&args[i], context)?;
            if is_truthy(&condition) {
                // Evaluate then branch
                if i + 1 < args.len() {
                    return evaluator.evaluate(&args[i + 1], context);
                } else {
                    return Ok(condition);
                }
            }

            // Move to next if/elseif pair
            i += 2;
        }

        Ok(Value::Null)
    }
}

/// Ternary operator (?:)
pub struct TernaryOperator;

impl Operator for TernaryOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 3 {
            return Ok(Value::Null);
        }

        let condition = evaluator.evaluate(&args[0], context)?;

        if is_truthy(&condition) {
            evaluator.evaluate(&args[1], context)
        } else {
            evaluator.evaluate(&args[2], context)
        }
    }
}
