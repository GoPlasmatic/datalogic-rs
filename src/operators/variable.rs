use serde_json::{Value, json};

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
use crate::value_helpers::{access_path, access_path_ref};
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Helper to apply a single path element (string or number) to a value (reference variant).
/// Returns None if the path element is an invalid type (not string/number) or path doesn't exist.
#[inline]
fn apply_path_element_ref<'a>(current: &'a Value, path_elem: &Value) -> Option<&'a Value> {
    match path_elem {
        Value::String(path_str) => {
            if let Value::Object(obj) = current {
                obj.get(path_str)
            } else {
                access_path_ref(current, path_str)
            }
        }
        Value::Number(n) => {
            let index = n.as_u64()?;
            if let Value::Array(arr) = current {
                arr.get(index as usize)
            } else {
                // Try as object key
                if let Value::Object(obj) = current {
                    obj.get(&n.to_string())
                } else {
                    None
                }
            }
        }
        _ => None,
    }
}

/// Variable access operator function (var)
#[inline]
pub fn evaluate_var(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(context.current().data().clone());
    }

    // Fast path: first arg is a literal string or number (most common case).
    // Avoids cloning the Value through evaluate_node just to extract a &str path.
    let path_arg;
    let path_str;
    let path = match &args[0] {
        CompiledNode::Value {
            value: Value::String(s),
            ..
        } => s.as_str(),
        CompiledNode::Value {
            value: Value::Number(n),
            ..
        } => {
            path_str = n.to_string();
            path_str.as_str()
        }
        // Dynamic path: must evaluate to get the value
        other => {
            path_arg = engine.evaluate_node(other, context)?;
            match &path_arg {
                Value::String(s) => s.as_str(),
                Value::Number(n) => {
                    path_str = n.to_string();
                    path_str.as_str()
                }
                _ => "",
            }
        }
    };

    // Access the variable in current context
    match access_path_ref(context.current().data(), path) {
        Some(result) => Ok(result.clone()),
        None => {
            // If not found and there's a default value, use it
            if args.len() > 1 {
                engine.evaluate_node(&args[1], context)
            } else {
                Ok(Value::Null)
            }
        }
    }
}
/// Value access operator function (val) with context level support
#[inline]
pub fn evaluate_val(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        // No args means current context
        return Ok(context.current().data().clone());
    }

    // Check if we have level access: [[level], path...]
    // This handles both {"val": [[1], "index"]} and {"val": [[2], "", "", "/"]}
    if args.len() >= 2 {
        // First check if it's level access - evaluate first arg to check
        let first_arg = engine.evaluate_node(&args[0], context)?;
        if let Value::Array(level_arr) = &first_arg
            && let Some(Value::Number(level_num)) = level_arr.first()
            && let Some(level) = level_num.as_i64()
        {
            // For metadata keys, only check if we have exactly 2 args
            if args.len() == 2 {
                let path_val = engine.evaluate_node(&args[1], context)?;
                let path = path_val.as_str().unwrap_or("");

                // Special handling for metadata keys like "index" and "key"
                // These are always in the current frame's metadata, regardless of level
                if path == "index" {
                    // Fast path: use get_index() to avoid HashMap lookup
                    if let Some(idx) = context.current().get_index() {
                        return Ok(json!(idx));
                    }
                    // Fallback to metadata HashMap
                    if let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
                } else if path == "key"
                    && let Some(metadata) = context.current().metadata()
                    && let Some(value) = metadata.get(path)
                {
                    return Ok(value.clone());
                }
            }

            // For simple two-arg case [[level], path], just access the path
            if args.len() == 2 {
                let path_val = engine.evaluate_node(&args[1], context)?;
                let path = match &path_val {
                    Value::String(s) => s.clone(),
                    Value::Number(n) if n.is_i64() => n.as_i64().unwrap().to_string(),
                    Value::Number(n) if n.is_u64() => n.as_u64().unwrap().to_string(),
                    _ => path_val.as_str().unwrap_or("").to_string(),
                };

                // Get frame at relative level for normal data access
                let frame = context
                    .get_at_level(level as isize)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;

                return Ok(access_path_ref(frame.data(), &path)
                    .cloned()
                    .unwrap_or(Value::Null));
            }

            // For multi-arg case, chain path access
            // First evaluate all path arguments
            let mut paths = Vec::new();
            for item in args.iter().skip(1) {
                let path_val = engine.evaluate_node(item, context)?;
                let path = match &path_val {
                    Value::String(s) => s.clone(),
                    Value::Number(n) if n.is_i64() => n.as_i64().unwrap().to_string(),
                    Value::Number(n) if n.is_u64() => n.as_u64().unwrap().to_string(),
                    _ => path_val.as_str().unwrap_or("").to_string(),
                };
                paths.push(path);
            }

            // Now get the frame and apply paths
            let frame = context
                .get_at_level(level as isize)
                .ok_or(Error::InvalidContextLevel(level as isize))?;

            // Start with a reference and only clone at the end or when needed
            let mut current_ref = frame.data();
            let mut owned_value = None;

            for path in &paths {
                if let Some(owned) = owned_value.as_ref() {
                    // If we already have an owned value, use access_path on it
                    owned_value = Some(access_path(owned, path).unwrap_or(Value::Null));
                } else {
                    // Still working with references
                    if let Some(next_ref) = access_path_ref(current_ref, path) {
                        current_ref = next_ref;
                    } else {
                        // Path not found, return null
                        return Ok(Value::Null);
                    }
                }
            }

            return Ok(owned_value.unwrap_or_else(|| current_ref.clone()));
        } else if args.len() == 2 {
            // Two arguments - check for datetime/duration property access first
            let first = engine.evaluate_node(&args[0], context)?;
            let second_val = engine.evaluate_node(&args[1], context)?;
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
            // Pre-evaluate args, then use reference-based traversal, clone only at the end
            let evaluated_args: Vec<Value> = args
                .iter()
                .map(|arg| engine.evaluate_node(arg, context))
                .collect::<Result<Vec<_>>>()?;
            let current_frame = context.current();
            let mut current = current_frame.data();
            for evaluated in &evaluated_args {
                match apply_path_element_ref(current, evaluated) {
                    Some(v) => current = v,
                    None => return Ok(Value::Null),
                }
            }
            return Ok(current.clone());
        }
    }

    // Handle multiple arguments (>2) as path chain
    // Pre-evaluate args, then use reference-based traversal, clone only at the end
    if args.len() > 2 {
        let evaluated_args: Vec<Value> = args
            .iter()
            .map(|arg| engine.evaluate_node(arg, context))
            .collect::<Result<Vec<_>>>()?;
        let current_frame = context.current();
        let mut current = current_frame.data();
        for evaluated in &evaluated_args {
            match apply_path_element_ref(current, evaluated) {
                Some(v) => current = v,
                None => return Ok(Value::Null),
            }
        }
        return Ok(current.clone());
    }

    // Single argument - evaluate it
    let path_value = engine.evaluate_node(&args[0], context)?;

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
                if path == "index" {
                    // Fast path: use get_index() to avoid HashMap lookup
                    if let Some(idx) = context.current().get_index() {
                        return Ok(json!(idx));
                    }
                    // Fallback to metadata HashMap
                    if let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
                } else if path == "key"
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

            // Chain path access through remaining elements using references
            let mut current = frame.data();
            for item in arr.iter().skip(1) {
                if let Some(path) = item.as_str() {
                    if let Some(next) = access_path_ref(current, path) {
                        current = next;
                    } else {
                        return Ok(Value::Null);
                    }
                } else {
                    return Ok(Value::Null);
                }
            }
            return Ok(current.clone());
        } else {
            // Array of paths like ["user", "admin"] or [1, 1] - chain access
            // Use reference-based traversal, clone only at the end
            let current_frame = context.current();
            let mut current = current_frame.data();
            for path_elem in arr {
                match apply_path_element_ref(current, path_elem) {
                    Some(v) => current = v,
                    None => return Ok(Value::Null),
                }
            }
            return Ok(current.clone());
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
            Ok(access_path_ref(context.current().data(), s)
                .cloned()
                .unwrap_or(Value::Null))
        }
        Value::Number(n) => {
            // Handle numeric index for array access
            if let Some(index) = n.as_u64() {
                if let Value::Array(arr) = context.current().data() {
                    Ok(arr.get(index as usize).cloned().unwrap_or(Value::Null))
                } else {
                    // Try converting to string for object key access
                    let key = n.to_string();
                    Ok(access_path_ref(context.current().data(), &key)
                        .cloned()
                        .unwrap_or(Value::Null))
                }
            } else {
                Ok(Value::Null)
            }
        }
        _ => Ok(Value::Null),
    }
}
/// Exists operator function - checks if a key exists in the data
#[inline]
pub fn evaluate_exists(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Bool(false));
    }

    // If we have a single argument, evaluate it
    if args.len() == 1 {
        let path_arg = engine.evaluate_node(&args[0], context)?;

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
            let path_val = engine.evaluate_node(arg, context)?;
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
/// Helper function to check if a key exists in an object
#[inline]
fn key_exists(value: &Value, key: &str) -> bool {
    if let Value::Object(obj) = value {
        obj.contains_key(key)
    } else {
        false
    }
}
