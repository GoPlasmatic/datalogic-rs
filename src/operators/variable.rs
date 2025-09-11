use serde_json::Value;

use crate::value_helpers::access_path;
use crate::{ContextStack, Error, Evaluator, Operator, Result};

/// Variable access operator (var)
pub struct VarOperator;

impl Operator for VarOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Evaluate the first argument to get the path
        let path_arg = if args.is_empty() {
            Value::String(String::new())
        } else {
            evaluator.evaluate(&args[0], context)?
        };

        // Get the path string
        let path_str;
        let path = match &path_arg {
            Value::String(s) => s.as_str(),
            Value::Number(n) => {
                // Support numeric indices for array access
                path_str = n.to_string();
                path_str.as_str()
            }
            _ => "",
        };

        // Access the variable in current context
        match access_path(&context.current().data, path) {
            Some(result) => Ok(result),
            None => {
                // If not found and there's a default value, use it
                if args.len() > 1 {
                    evaluator.evaluate(&args[1], context)
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }
}

/// Value access operator (val) with context level support
pub struct ValOperator;

impl Operator for ValOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            // No args means current context
            return Ok(context.current().data.clone());
        }

        // Check if we have two arguments: [level_array, path]
        // This is the case for {"val": [[1], "index"]}
        if args.len() == 2 {
            // First arg should be an array with level, second is the path
            if let Value::Array(level_arr) = &args[0]
                && let Some(Value::Number(level_num)) = level_arr.first()
                && let Some(level) = level_num.as_i64()
            {
                // Access path in the request
                let path = args[1].as_str().unwrap_or("");

                // Special handling for metadata keys like "index" and "key"
                // These are always in the current frame's metadata, regardless of level
                if (path == "index" || path == "key")
                    && let Some(metadata) = &context.current().metadata
                    && let Some(value) = metadata.get(path)
                {
                    return Ok(value.clone());
                }

                // Get frame at relative level for normal data access
                // Both [1] and [-1] go up 1 level to parent
                // Both [2] and [-2] go up 2 levels to grandparent
                let frame = context
                    .get_at_level(level as isize)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;

                // Normal path access in the target frame
                return Ok(access_path(&frame.data, path).unwrap_or(Value::Null));
            }
        }

        // Single argument - evaluate it
        let path_value = evaluator.evaluate(&args[0], context)?;

        // Handle array notation for context levels: [[level], "path"]
        // Level indicates how many levels to go up from current
        // Sign doesn't matter: [1] and [-1] both mean parent
        // [2] and [-2] both mean grandparent, etc.
        if let Value::Array(arr) = &path_value
            && arr.len() == 2
            && let Value::Array(level_arr) = &arr[0]
            && let Some(Value::Number(level_num)) = level_arr.first()
            && let Some(level) = level_num.as_i64()
        {
            // Access path in the request
            let path = arr[1].as_str().unwrap_or("");

            // Special handling for metadata keys like "index" and "key"
            // These are always in the current frame's metadata, regardless of level
            if (path == "index" || path == "key")
                && let Some(metadata) = &context.current().metadata
                && let Some(value) = metadata.get(path)
            {
                return Ok(value.clone());
            }

            // Get frame at relative level for normal data access
            // Both [1] and [-1] go up 1 level to parent
            // Both [2] and [-2] go up 2 levels to grandparent
            let frame = context
                .get_at_level(level as isize)
                .ok_or(Error::InvalidContextLevel(level as isize))?;

            // Normal path access in the target frame
            return Ok(access_path(&frame.data, path).unwrap_or(Value::Null));
        }

        // Standard path access in current context
        let path = path_value.as_str().unwrap_or("");
        Ok(access_path(&context.current().data, path).unwrap_or(Value::Null))
    }
}
