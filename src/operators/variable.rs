use serde_json::{Value, json};

use crate::compiled::{MetadataHint, PathSegment, ReduceHint};
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

    // Fast path for reduce context fields — avoids BTreeMap lookup entirely
    {
        let frame = context.current();
        if path == "current" {
            if let Some(v) = frame.get_reduce_current() {
                return Ok(v.clone());
            }
        } else if path == "accumulator" {
            if let Some(v) = frame.get_reduce_accumulator() {
                return Ok(v.clone());
            }
        } else if let Some(rest) = path.strip_prefix("current.") {
            if let Some(current) = frame.get_reduce_current() {
                return Ok(access_path_ref(current, rest)
                    .cloned()
                    .unwrap_or(Value::Null));
            }
        } else if let Some(rest) = path.strip_prefix("accumulator.")
            && let Some(acc) = frame.get_reduce_accumulator()
        {
            return Ok(access_path_ref(acc, rest).cloned().unwrap_or(Value::Null));
        }
    }

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
                } else if path == "key" {
                    // Fast path: use get_key() to avoid HashMap lookup
                    if let Some(key) = context.current().get_key() {
                        return Ok(Value::String(key.to_string()));
                    }
                    // Fallback to metadata HashMap
                    if let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
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
            // Two arguments - chain access like ["user", "admin"] or [1, 1]
            // Pre-evaluate args, then use reference-based traversal, clone only at the end
            let evaluated_args: Vec<Value> = args
                .iter()
                .map(|arg| engine.evaluate_node(arg, context))
                .collect::<Result<Vec<_>>>()?;
            let current_frame = context.current();

            // Fast path: resolve reduce context fields for first path element
            let resolve_start = if let Some(Value::String(s)) = evaluated_args.first() {
                if s == "current" {
                    current_frame.get_reduce_current()
                } else if s == "accumulator" {
                    current_frame.get_reduce_accumulator()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(start) = resolve_start {
                // Chain remaining path elements from the reduce field
                let mut current = start;
                for evaluated in &evaluated_args[1..] {
                    match apply_path_element_ref(current, evaluated) {
                        Some(v) => current = v,
                        None => return Ok(Value::Null),
                    }
                }
                return Ok(current.clone());
            }

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

        // Fast path: resolve reduce context fields for first path element
        let resolve_start = if let Some(Value::String(s)) = evaluated_args.first() {
            if s == "current" {
                current_frame.get_reduce_current()
            } else if s == "accumulator" {
                current_frame.get_reduce_accumulator()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(start) = resolve_start {
            let mut current = start;
            for evaluated in &evaluated_args[1..] {
                match apply_path_element_ref(current, evaluated) {
                    Some(v) => current = v,
                    None => return Ok(Value::Null),
                }
            }
            return Ok(current.clone());
        }

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
                } else if path == "key" {
                    // Fast path: use get_key() to avoid HashMap lookup
                    if let Some(key) = context.current().get_key() {
                        return Ok(Value::String(key.to_string()));
                    }
                    // Fallback to metadata HashMap
                    if let Some(metadata) = context.current().metadata()
                        && let Some(value) = metadata.get(path)
                    {
                        return Ok(value.clone());
                    }
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

    // Fast path for reduce context fields in val operator
    if let Value::String(s) = &path_value {
        let frame = context.current();
        if s == "current" {
            if let Some(v) = frame.get_reduce_current() {
                return Ok(v.clone());
            }
        } else if s == "accumulator" {
            if let Some(v) = frame.get_reduce_accumulator() {
                return Ok(v.clone());
            }
        } else if let Some(rest) = s.strip_prefix("current.") {
            if let Some(current) = frame.get_reduce_current() {
                return Ok(access_path_ref(current, rest)
                    .cloned()
                    .unwrap_or(Value::Null));
            }
        } else if let Some(rest) = s.strip_prefix("accumulator.")
            && let Some(acc) = frame.get_reduce_accumulator()
        {
            return Ok(access_path_ref(acc, rest).cloned().unwrap_or(Value::Null));
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

/// Traverse pre-parsed path segments on a value (reference only, no context needed).
#[inline]
fn try_traverse_segments<'a>(data: &'a Value, segments: &[PathSegment]) -> Option<&'a Value> {
    let mut current = data;
    for seg in segments {
        match seg {
            PathSegment::Field(key) => match current {
                Value::Object(obj) => match obj.get(key.as_ref()) {
                    Some(v) => current = v,
                    None => return None,
                },
                _ => return None,
            },
            PathSegment::Index(idx) => match current {
                Value::Array(arr) => match arr.get(*idx) {
                    Some(v) => current = v,
                    None => return None,
                },
                _ => return None,
            },
            PathSegment::FieldOrIndex(key, idx) => match current {
                Value::Object(obj) => match obj.get(key.as_ref()) {
                    Some(v) => current = v,
                    None => return None,
                },
                Value::Array(arr) => match arr.get(*idx) {
                    Some(v) => current = v,
                    None => return None,
                },
                _ => return None,
            },
        }
    }
    Some(current)
}

/// Return the default value if provided, otherwise null.
#[inline]
fn default_or_null(
    default_value: Option<&CompiledNode>,
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    match default_value {
        Some(node) => engine.evaluate_node(node, context),
        None => Ok(Value::Null),
    }
}

/// Evaluate a pre-compiled variable access (unified var/val).
///
/// This function handles both var (scope_level=0) and val (scope_level=N) access
/// with pre-parsed path segments, avoiding runtime string splitting and parsing.
#[inline]
pub fn evaluate_compiled_var(
    scope_level: u32,
    segments: &[PathSegment],
    reduce_hint: ReduceHint,
    metadata_hint: MetadataHint,
    default_value: Option<&CompiledNode>,
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // 1. Metadata fast path (index/key)
    match metadata_hint {
        MetadataHint::Index => {
            if let Some(idx) = context.current().get_index() {
                return Ok(json!(idx));
            }
            if let Some(metadata) = context.current().metadata()
                && let Some(value) = metadata.get("index")
            {
                return Ok(value.clone());
            }
        }
        MetadataHint::Key => {
            if let Some(key) = context.current().get_key() {
                return Ok(Value::String(key.to_string()));
            }
            if let Some(metadata) = context.current().metadata()
                && let Some(value) = metadata.get("key")
            {
                return Ok(value.clone());
            }
        }
        MetadataHint::None => {}
    }

    // 2. Reduce context fast path
    // Use Option<Option<Value>>: None = no reduce, Some(Some(v)) = found, Some(None) = not found
    let reduce_result = {
        let frame = context.current();
        match reduce_hint {
            ReduceHint::Current => frame.get_reduce_current().map(|v| Some(v.clone())),
            ReduceHint::Accumulator => frame.get_reduce_accumulator().map(|v| Some(v.clone())),
            ReduceHint::CurrentPath => frame
                .get_reduce_current()
                .map(|current| try_traverse_segments(current, &segments[1..]).cloned()),
            ReduceHint::AccumulatorPath => frame
                .get_reduce_accumulator()
                .map(|acc| try_traverse_segments(acc, &segments[1..]).cloned()),
            ReduceHint::None => None,
        }
    }; // frame borrow ends here

    match reduce_result {
        Some(Some(v)) => return Ok(v),
        Some(None) => return default_or_null(default_value, context, engine),
        None => {} // fall through to normal access
    }

    // 3. Get data at scope level and traverse
    let data_result = {
        let frame = if scope_level == 0 {
            context.current()
        } else {
            context
                .get_at_level(scope_level as isize)
                .ok_or(Error::InvalidContextLevel(scope_level as isize))?
        };

        if segments.is_empty() {
            Some(frame.data().clone())
        } else {
            try_traverse_segments(frame.data(), segments).cloned()
        }
    }; // frame borrow ends here

    match data_result {
        Some(v) => Ok(v),
        None => default_or_null(default_value, context, engine),
    }
}

/// Evaluate a pre-compiled exists check.
#[inline]
pub fn evaluate_compiled_exists(
    scope_level: u32,
    segments: &[PathSegment],
    context: &mut ContextStack,
) -> Result<Value> {
    let frame = if scope_level == 0 {
        context.current()
    } else {
        match context.get_at_level(scope_level as isize) {
            Some(f) => f,
            None => return Ok(Value::Bool(false)),
        }
    };

    if segments.is_empty() {
        return Ok(Value::Bool(true));
    }

    let mut current = frame.data();
    let last = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        match seg {
            PathSegment::Field(key) => {
                if let Value::Object(obj) = current {
                    if i == last {
                        return Ok(Value::Bool(obj.contains_key(key.as_ref())));
                    }
                    match obj.get(key.as_ref()) {
                        Some(v) => current = v,
                        None => return Ok(Value::Bool(false)),
                    }
                } else {
                    return Ok(Value::Bool(false));
                }
            }
            PathSegment::Index(idx) => {
                if let Value::Array(arr) = current {
                    if i == last {
                        return Ok(Value::Bool(arr.get(*idx).is_some()));
                    }
                    match arr.get(*idx) {
                        Some(v) => current = v,
                        None => return Ok(Value::Bool(false)),
                    }
                } else {
                    return Ok(Value::Bool(false));
                }
            }
            PathSegment::FieldOrIndex(key, idx) => match current {
                Value::Object(obj) => {
                    if i == last {
                        return Ok(Value::Bool(obj.contains_key(key.as_ref())));
                    }
                    match obj.get(key.as_ref()) {
                        Some(v) => current = v,
                        None => return Ok(Value::Bool(false)),
                    }
                }
                Value::Array(arr) => {
                    if i == last {
                        return Ok(Value::Bool(arr.get(*idx).is_some()));
                    }
                    match arr.get(*idx) {
                        Some(v) => current = v,
                        None => return Ok(Value::Bool(false)),
                    }
                }
                _ => return Ok(Value::Bool(false)),
            },
        }
    }
    Ok(Value::Bool(true))
}
