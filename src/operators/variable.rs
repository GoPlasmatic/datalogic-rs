use serde_json::{Value, json};

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
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
        match access_path(context.current().data(), path) {
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
            return Ok(context.current().data().clone());
        }

        // Check if we have level access: [[level], path...]
        // This handles both {"val": [[1], "index"]} and {"val": [[2], "", "", "/"]}
        if args.len() >= 2 {
            // First check if it's level access
            if let Value::Array(level_arr) = &args[0]
                && let Some(Value::Number(level_num)) = level_arr.first()
                && let Some(level) = level_num.as_i64()
            {
                // For metadata keys, only check if we have exactly 2 args
                if args.len() == 2 {
                    let path = args[1].as_str().unwrap_or("");

                    // Special handling for metadata keys like "index" and "key"
                    // These are always in the current frame's metadata, regardless of level
                    if (path == "index" || path == "key")
                        && let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
                }

                // Get frame at relative level for normal data access
                // Both [1] and [-1] go up 1 level to parent
                // Both [2] and [-2] go up 2 levels to grandparent
                let frame = context
                    .get_at_level(level as isize)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;

                // For simple two-arg case [[level], path], just access the path
                if args.len() == 2 {
                    let path = args[1].as_str().unwrap_or("");
                    return Ok(access_path(frame.data(), path).unwrap_or(Value::Null));
                }

                // For multi-arg case, chain path access
                let mut result = frame.data().clone();
                for item in args.iter().skip(1) {
                    let path = item.as_str().unwrap_or("");
                    result = access_path(&result, path).unwrap_or(Value::Null);
                }
                return Ok(result);
            } else if args.len() == 2 {
                // Two arguments - check for datetime/duration property access first
                let first = evaluator.evaluate(&args[0], context)?;
                let second_val = evaluator.evaluate(&args[1], context)?;
                let second_str = second_val.as_str();

                if let Some(prop) = second_str {
                    // Check for datetime property access (both objects and strings)
                    let dt = if is_datetime_object(&first) {
                        extract_datetime(&first)
                    } else if let Value::String(s) = &first {
                        crate::datetime::DataDateTime::parse(s)
                    } else {
                        None
                    };

                    if let Some(datetime) = dt {
                        return Ok(match prop {
                            "year" => json!(datetime.year()),
                            "month" => json!(datetime.month()),
                            "day" => json!(datetime.day()),
                            "hour" => json!(datetime.hour()),
                            "minute" => json!(datetime.minute()),
                            "second" => json!(datetime.second()),
                            "timestamp" => json!(datetime.timestamp()),
                            "iso" => Value::String(datetime.to_iso_string()),
                            _ => Value::Null,
                        });
                    }

                    // Check for duration property access (both objects and strings)
                    let dur = if is_duration_object(&first) {
                        extract_duration(&first)
                    } else if let Value::String(s) = &first {
                        crate::datetime::DataDuration::parse(s)
                    } else {
                        None
                    };

                    if let Some(duration) = dur {
                        return Ok(match prop {
                            "days" => json!(duration.days()),
                            "hours" => json!(duration.hours()),
                            "minutes" => json!(duration.minutes()),
                            "seconds" => json!(duration.seconds()),
                            "total_seconds" => json!(duration.total_seconds()),
                            _ => Value::Null,
                        });
                    }
                }

                // Two arguments - chain access like ["user", "admin"] or [1, 1]
                let mut result = context.current().data().clone();
                for arg in args {
                    // Evaluate the argument if needed
                    let evaluated = if arg.is_string() || arg.is_number() {
                        arg.clone()
                    } else {
                        evaluator.evaluate(arg, context)?
                    };

                    match &evaluated {
                        Value::String(path_str) => {
                            // Try direct object key access first for simple keys
                            if let Value::Object(obj) = &result {
                                if let Some(val) = obj.get(path_str) {
                                    result = val.clone();
                                } else {
                                    result = Value::Null;
                                }
                            } else {
                                result = access_path(&result, path_str).unwrap_or(Value::Null);
                            }
                        }
                        Value::Number(n) => {
                            // Handle numeric index for array access
                            if let Some(index) = n.as_u64() {
                                if let Value::Array(arr) = result {
                                    result =
                                        arr.get(index as usize).cloned().unwrap_or(Value::Null);
                                } else {
                                    // Try as string key for object
                                    let key = n.to_string();
                                    result = access_path(&result, &key).unwrap_or(Value::Null);
                                }
                            } else {
                                return Ok(Value::Null);
                            }
                        }
                        _ => return Ok(Value::Null),
                    }
                }
                return Ok(result);
            }
        }

        // Handle multiple arguments (>2) as path chain
        if args.len() > 2 {
            let mut result = context.current().data().clone();
            for arg in args {
                // Evaluate the argument first
                let evaluated = if arg.is_string() || arg.is_number() {
                    arg.clone()
                } else {
                    evaluator.evaluate(arg, context)?
                };

                match &evaluated {
                    Value::String(path_str) => {
                        // Try direct object key access first for simple keys
                        if let Value::Object(obj) = &result {
                            if let Some(val) = obj.get(path_str) {
                                result = val.clone();
                            } else {
                                result = Value::Null;
                            }
                        } else {
                            result = access_path(&result, path_str).unwrap_or(Value::Null);
                        }
                    }
                    Value::Number(n) => {
                        // Handle numeric index for array access
                        if let Some(index) = n.as_u64() {
                            if let Value::Array(arr) = result {
                                result = arr.get(index as usize).cloned().unwrap_or(Value::Null);
                            } else {
                                // Try as string key for object
                                let key = n.to_string();
                                result = access_path(&result, &key).unwrap_or(Value::Null);
                            }
                        } else {
                            return Ok(Value::Null);
                        }
                    }
                    _ => return Ok(Value::Null),
                }
            }
            return Ok(result);
        }

        // Single argument - evaluate it
        let path_value = evaluator.evaluate(&args[0], context)?;

        // Handle array notation for context levels: [[level], "path", ...]
        // Level indicates how many levels to go up from current
        // Sign doesn't matter: [1] and [-1] both mean parent
        // [2] and [-2] both mean grandparent, etc.
        if let Value::Array(arr) = &path_value {
            // Check if first element is a level access array: [[level], ...]
            if arr.len() >= 2
                && let Value::Array(level_arr) = &arr[0]
                && let Some(Value::Number(level_num)) = level_arr.first()
                && let Some(level) = level_num.as_i64()
            {
                // Special case for metadata keys with exactly 2 elements
                if arr.len() == 2 {
                    let path = arr[1].as_str().unwrap_or("");

                    // Special handling for metadata keys like "index" and "key"
                    // These are always in the current frame's metadata, regardless of level
                    if (path == "index" || path == "key")
                        && let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
                }

                // Get frame at relative level for normal data access
                // Both [1] and [-1] go up 1 level to parent
                // Both [2] and [-2] go up 2 levels to grandparent
                let frame = context
                    .get_at_level(level as isize)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;

                // Chain path access through remaining elements
                let mut result = frame.data().clone();
                for item in arr.iter().skip(1) {
                    if let Some(path) = item.as_str() {
                        result = access_path(&result, path).unwrap_or(Value::Null);
                    } else {
                        return Ok(Value::Null);
                    }
                }
                return Ok(result);
            } else {
                // Array of paths like ["user", "admin"] or [1, 1] - chain access
                let mut result = context.current().data().clone();
                for path_elem in arr {
                    match path_elem {
                        Value::String(path_str) => {
                            // Try direct object key access first for simple keys
                            if let Value::Object(obj) = &result {
                                if let Some(val) = obj.get(path_str) {
                                    result = val.clone();
                                } else {
                                    result = Value::Null;
                                }
                            } else {
                                result = access_path(&result, path_str).unwrap_or(Value::Null);
                            }
                        }
                        Value::Number(n) => {
                            // Handle numeric index for array access
                            if let Some(index) = n.as_u64() {
                                if let Value::Array(arr_val) = result {
                                    result =
                                        arr_val.get(index as usize).cloned().unwrap_or(Value::Null);
                                } else {
                                    // Try as string key for object
                                    let key = n.to_string();
                                    result = access_path(&result, &key).unwrap_or(Value::Null);
                                }
                            } else {
                                return Ok(Value::Null);
                            }
                        }
                        _ => {
                            // Non-string/number element, can't use as path
                            return Ok(Value::Null);
                        }
                    }
                }
                return Ok(result);
            }
        }

        // Standard path access in current context
        match &path_value {
            Value::String(s) => {
                // For single string arguments, try direct object key access first
                // This handles empty string keys and keys with dots correctly
                if let Value::Object(obj) = context.current().data()
                    && let Some(val) = obj.get(s)
                {
                    return Ok(val.clone());
                }
                // Fall back to access_path for complex paths
                Ok(access_path(context.current().data(), s).unwrap_or(Value::Null))
            }
            Value::Number(n) => {
                // Handle numeric index for array access
                if let Some(index) = n.as_u64() {
                    if let Value::Array(arr) = context.current().data() {
                        Ok(arr.get(index as usize).cloned().unwrap_or(Value::Null))
                    } else {
                        // Try converting to string for object key access
                        let key = n.to_string();
                        Ok(access_path(context.current().data(), &key).unwrap_or(Value::Null))
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            _ => Ok(Value::Null),
        }
    }
}

/// Exists operator - checks if a key exists in the data
pub struct ExistsOperator;

impl Operator for ExistsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Ok(Value::Bool(false));
        }

        // If we have a single argument, evaluate it
        if args.len() == 1 {
            let path_arg = evaluator.evaluate(&args[0], context)?;

            // Handle different path formats
            match path_arg {
                Value::String(path) => {
                    // Simple string path
                    Ok(Value::Bool(key_exists(context.current().data(), &path)))
                }
                Value::Array(paths) => {
                    // Array of path segments for nested access
                    if paths.is_empty() {
                        return Ok(Value::Bool(false));
                    }

                    let current_frame = context.current();
                    let mut current = current_frame.data();

                    for (i, path_val) in paths.iter().enumerate() {
                        if let Value::String(path) = path_val {
                            if let Value::Object(obj) = current {
                                // For the last path segment, just check if key exists
                                if i == paths.len() - 1 {
                                    return Ok(Value::Bool(obj.contains_key(path)));
                                }
                                // For intermediate segments, navigate deeper
                                if let Some(next) = obj.get(path) {
                                    current = next;
                                } else {
                                    return Ok(Value::Bool(false));
                                }
                            } else {
                                return Ok(Value::Bool(false));
                            }
                        } else {
                            return Ok(Value::Bool(false));
                        }
                    }

                    // Should not reach here if paths is non-empty
                    Ok(Value::Bool(true))
                }
                _ => Ok(Value::Bool(false)),
            }
        } else {
            // Multiple arguments - treat as path segments for nested access
            // First evaluate all args to get the path segments
            let mut paths = Vec::new();
            for arg in args {
                let path_val = evaluator.evaluate(arg, context)?;
                if let Value::String(path) = path_val {
                    paths.push(path);
                } else {
                    return Ok(Value::Bool(false));
                }
            }

            // Now navigate through the paths
            let current_frame = context.current();
            let mut current = current_frame.data();

            for (i, path) in paths.iter().enumerate() {
                if let Value::Object(obj) = current {
                    // For the last path segment, just check if key exists
                    if i == paths.len() - 1 {
                        return Ok(Value::Bool(obj.contains_key(path)));
                    }
                    // For intermediate segments, navigate deeper
                    if let Some(next) = obj.get(path) {
                        current = next;
                    } else {
                        return Ok(Value::Bool(false));
                    }
                } else {
                    return Ok(Value::Bool(false));
                }
            }

            // Should not reach here if paths is non-empty
            Ok(Value::Bool(true))
        }
    }
}

/// Helper function to check if a key exists in an object
fn key_exists(value: &Value, key: &str) -> bool {
    if let Value::Object(obj) = value {
        obj.contains_key(key)
    } else {
        false
    }
}
