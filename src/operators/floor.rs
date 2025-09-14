use serde_json::Value;

use crate::{ContextStack, Error, Evaluator, Operator, Result};

// Strict number extraction - only accepts actual numbers or numeric strings
#[inline]
fn get_number_strict(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Floor operator function (floor)
#[inline]
pub fn evaluate_floor(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    FloorOperator.evaluate(args, context, evaluator)
}

/// Floor operator (floor)
pub struct FloorOperator;

impl Operator for FloorOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Check if we have multiple arguments - if so, return array of floor values
        if args.len() > 1 {
            let mut results = Vec::new();
            for arg in args {
                let value = evaluator.evaluate(arg, context)?;
                if let Some(num) = get_number_strict(&value) {
                    let floor_val = num.floor();
                    results.push(Value::Number((floor_val as i64).into()));
                } else {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }
            }
            return Ok(Value::Array(results));
        }

        // Single argument - evaluate and return floor
        let value = evaluator.evaluate(&args[0], context)?;

        if let Some(num) = get_number_strict(&value) {
            let floor_val = num.floor();
            Ok(Value::Number((floor_val as i64).into()))
        } else {
            Err(Error::InvalidArguments("Invalid Arguments".to_string()))
        }
    }
}
