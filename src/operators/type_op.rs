use serde_json::Value;

use crate::{ContextStack, Evaluator, Operator, Result};

/// Type operator - returns the type of a value as a string
pub struct TypeOperator;

impl Operator for TypeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Special handling for the type operator:
        // - {"type": null} -> args = [null] -> type of null
        // - {"type": []} -> args = [] -> type of empty array
        // - {"type": [1,2,3]} -> args = [1,2,3] -> type of array [1,2,3]
        // - {"type": {"var": "x"}} -> args = [{"var": "x"}] -> type of evaluated var

        // If we have exactly one argument and it's not a simple value, evaluate it
        // Otherwise, if we have 0 or multiple arguments, it was an array literal
        let value = if args.len() == 1 {
            // Single argument - check if it needs evaluation
            evaluator.evaluate(&args[0], context)?
        } else {
            // Multiple arguments or no arguments - reconstruct the array
            let mut arr = Vec::new();
            for arg in args {
                arr.push(evaluator.evaluate(arg, context)?);
            }
            Value::Array(arr)
        };

        let type_str = match &value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(s) => {
                // Check for special datetime/duration formats
                // Simple heuristic: if it looks like an ISO datetime or duration
                if s.contains('T') && s.contains(':') && (s.contains('Z') || s.contains('+')) {
                    "datetime"
                } else if s.chars().any(|c| matches!(c, 'd' | 'h' | 'm' | 's'))
                    && s.chars().any(|c| c.is_ascii_digit())
                {
                    // Simple duration check (e.g., "1d", "2h30m")
                    "duration"
                } else {
                    "string"
                }
            }
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };

        Ok(Value::String(type_str.to_string()))
    }
}
