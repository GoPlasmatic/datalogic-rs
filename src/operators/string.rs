#[cfg(feature = "ext-string")]
use regex::Regex;
use serde_json::Value;
#[cfg(feature = "ext-string")]
use serde_json::json;

use super::helpers::to_string_cow;
#[cfg(feature = "ext-string")]
use super::variable;
#[cfg(feature = "ext-string")]
use crate::constants::INVALID_ARGS;
#[cfg(feature = "ext-string")]
use crate::error::Error;
#[cfg(feature = "ext-string")]
use crate::node::{MetadataHint, ReduceHint};
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// String concatenation operator function (cat) - variadic
#[inline]
pub fn evaluate_cat(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut result = String::with_capacity(args.len() * 16);

    for arg in args {
        let value = engine.evaluate_node_cow(arg, context)?;
        // If the value is an array, concatenate its elements
        if let Value::Array(arr) = value.as_ref() {
            for item in arr {
                result.push_str(&to_string_cow(item));
            }
        } else {
            result.push_str(&to_string_cow(value.as_ref()));
        }
    }

    Ok(Value::String(result))
}

/// Substring operator function (substr)
#[inline]
pub fn evaluate_substr(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::String(String::new()));
    }

    let string_val = engine.evaluate_node(&args[0], context)?;
    let string: std::borrow::Cow<str> = match &string_val {
        Value::String(s) => std::borrow::Cow::Borrowed(s.as_str()),
        _ => std::borrow::Cow::Owned(string_val.to_string()),
    };

    // Get character count for proper bounds checking
    let char_count = string.chars().count();

    // Fast path: read literal integer args directly without evaluate_node dispatch
    let start = if args.len() > 1 {
        if let CompiledNode::Value { value, .. } = &args[1] {
            value.as_i64().unwrap_or(0)
        } else {
            let start_val = engine.evaluate_node(&args[1], context)?;
            start_val.as_i64().unwrap_or(0)
        }
    } else {
        0
    };

    let length = if args.len() > 2 {
        if let CompiledNode::Value { value, .. } = &args[2] {
            value.as_i64()
        } else {
            let length_val = engine.evaluate_node(&args[2], context)?;
            length_val.as_i64()
        }
    } else {
        None
    };

    // Safe bounds checking with overflow protection
    let actual_start = if start < 0 {
        // Safely handle negative indices
        let abs_start = start.saturating_abs() as usize;
        char_count.saturating_sub(abs_start)
    } else {
        // Safely handle positive indices
        (start as usize).min(char_count)
    };

    let result = if let Some(len) = length {
        if len < 0 {
            // Special case: negative length means use it as end position (like slice)
            // This mimics JSONLogic's behavior which differs from JavaScript's substr
            let end_pos = if len < 0 {
                // Negative end position counts from end of string
                let abs_end = len.saturating_abs() as usize;
                char_count.saturating_sub(abs_end)
            } else {
                0
            };

            // Take characters from actual_start to end_pos
            if end_pos > actual_start {
                string
                    .chars()
                    .skip(actual_start)
                    .take(end_pos - actual_start)
                    .collect()
            } else {
                String::new()
            }
        } else if len == 0 {
            // Zero length returns empty string
            String::new()
        } else {
            // Positive length - take from start position
            let take_count = (len as usize).min(char_count.saturating_sub(actual_start));
            string.chars().skip(actual_start).take(take_count).collect()
        }
    } else {
        // No length specified - take rest of string
        string.chars().skip(actual_start).collect()
    };

    Ok(Value::String(result))
}

/// In operator function - checks if a value is in a string or array
#[inline]
pub fn evaluate_in(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Bool(false));
    }

    let needle = engine.evaluate_node_cow(&args[0], context)?;
    let haystack = engine.evaluate_node_cow(&args[1], context)?;

    let result = match haystack.as_ref() {
        Value::String(s) => match needle.as_ref() {
            Value::String(n) => s.contains(n.as_str()),
            _ => false,
        },
        Value::Array(arr) => arr.iter().any(|v| v == needle.as_ref()),
        _ => false,
    };

    Ok(Value::Bool(result))
}

/// Length operator function - returns the length of a string or array
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_length(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() || args.len() > 1 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Fast path: CompiledVar with scope_level 0 — navigate directly, skip clone
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        ..
    } = &args[0]
    {
        if let Some(val) = variable::try_traverse_segments(context.current().data(), segments) {
            return match val {
                Value::String(s) => Ok(Value::Number(serde_json::Number::from(
                    s.chars().count() as i64
                ))),
                Value::Array(arr) => Ok(Value::Number(serde_json::Number::from(arr.len() as i64))),
                _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
            };
        }
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Use cow to avoid cloning strings/arrays just to get their length
    let value = engine.evaluate_node_cow(&args[0], context)?;

    match value.as_ref() {
        Value::String(s) => {
            let char_count = s.chars().count();
            if char_count > i64::MAX as usize {
                return Err(Error::InvalidArguments("String too long".to_string()));
            }
            Ok(Value::Number(serde_json::Number::from(char_count as i64)))
        }
        Value::Array(arr) => {
            if arr.len() > i64::MAX as usize {
                return Err(Error::InvalidArguments("Array too long".to_string()));
            }
            Ok(Value::Number(serde_json::Number::from(arr.len() as i64)))
        }
        Value::Null | Value::Number(_) | Value::Bool(_) | Value::Object(_) => {
            Err(Error::InvalidArguments(INVALID_ARGS.into()))
        }
    }
}

/// StartsWithOperator function - checks if a string starts with a prefix
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_starts_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: pattern is a literal string (most common case) — avoid clone
    if let CompiledNode::Value {
        value: Value::String(p),
        ..
    } = &args[1]
    {
        return Ok(Value::Bool(text_str.starts_with(p.as_str())));
    }

    let pattern = engine.evaluate_node(&args[1], context)?;
    let pattern_str = pattern.as_str().unwrap_or("");
    Ok(Value::Bool(text_str.starts_with(pattern_str)))
}

/// EndsWithOperator function - checks if a string ends with a suffix
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_ends_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: pattern is a literal string (most common case) — avoid clone
    if let CompiledNode::Value {
        value: Value::String(p),
        ..
    } = &args[1]
    {
        return Ok(Value::Bool(text_str.ends_with(p.as_str())));
    }

    let pattern = engine.evaluate_node(&args[1], context)?;
    let pattern_str = pattern.as_str().unwrap_or("");
    Ok(Value::Bool(text_str.ends_with(pattern_str)))
}

/// UpperOperator function - converts a string to uppercase
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_upper(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: if ASCII and already uppercase, return original value (no allocation)
    let already_upper = value
        .as_str()
        .is_some_and(|s| s.is_ascii() && !s.bytes().any(|b| b.is_ascii_lowercase()));
    if already_upper {
        return Ok(value);
    }
    let text = value.as_str().unwrap_or("");
    Ok(Value::String(text.to_uppercase()))
}

/// LowerOperator function - converts a string to lowercase
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_lower(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: if ASCII and already lowercase, return original value (no allocation)
    let already_lower = value
        .as_str()
        .is_some_and(|s| s.is_ascii() && !s.bytes().any(|b| b.is_ascii_uppercase()));
    if already_lower {
        return Ok(value);
    }
    let text = value.as_str().unwrap_or("");
    Ok(Value::String(text.to_lowercase()))
}

/// TrimOperator function - removes leading and trailing whitespace from a string
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_trim(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: check if trimming is needed before allocating
    let needs_trim = value.as_str().is_some_and(|s| {
        !s.is_empty() && {
            // chars().next() and next_back() are O(1) for valid UTF-8
            s.starts_with(|c: char| c.is_whitespace()) || s.ends_with(|c: char| c.is_whitespace())
        }
    });
    if !needs_trim {
        return match &value {
            Value::String(_) => Ok(value),
            _ => Ok(Value::String(String::new())),
        };
    }
    let text = value.as_str().unwrap_or("");
    Ok(Value::String(text.trim().to_string()))
}

/// SplitOperator function - splits a string by delimiter or extracts regex groups
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_split(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: delimiter is a literal string — skip regex check entirely.
    // Valid regex patterns are already handled at compile-time via CompiledSplitRegex,
    // so any remaining literal delimiter is guaranteed to be a plain string split.
    if let CompiledNode::Value {
        value: Value::String(delim),
        ..
    } = &args[1]
    {
        return split_normal(text_str, delim.as_str());
    }

    let delimiter = engine.evaluate_node(&args[1], context)?;
    let delimiter_str = delimiter.as_str().unwrap_or("");

    // Check if delimiter is a regex pattern with named groups (dynamic delimiter case)
    if delimiter_str.contains("(?P<") {
        // Try to parse as regex
        match Regex::new(delimiter_str) {
            Ok(re) => {
                // Check if regex has named groups
                let capture_names: Vec<_> = re.capture_names().flatten().collect();

                if !capture_names.is_empty() {
                    // Extract named groups
                    if let Some(captures) = re.captures(text_str) {
                        let mut result = serde_json::Map::new();

                        for name in capture_names {
                            if let Some(m) = captures.name(name) {
                                result.insert(
                                    name.to_string(),
                                    Value::String(m.as_str().to_string()),
                                );
                            }
                        }

                        return Ok(Value::Object(result));
                    } else {
                        // No match, return empty object
                        return Ok(Value::Object(serde_json::Map::new()));
                    }
                }
            }
            Err(_) => {
                // Invalid regex, fall back to normal split
            }
        }
    }

    // Normal string split
    split_normal(text_str, delimiter_str)
}

/// Split with a pre-compiled regex (used when regex is known at compile time)
#[cfg(feature = "ext-string")]
#[inline]
pub fn evaluate_split_with_regex(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    regex: &Regex,
    capture_names: &[Box<str>],
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    if let Some(captures) = regex.captures(text_str) {
        let mut result = serde_json::Map::new();
        for name in capture_names {
            if let Some(m) = captures.name(name) {
                result.insert(name.to_string(), Value::String(m.as_str().to_string()));
            }
        }
        Ok(Value::Object(result))
    } else {
        Ok(Value::Object(serde_json::Map::new()))
    }
}

#[cfg(feature = "ext-string")]
#[inline]
fn split_normal(text_str: &str, delimiter_str: &str) -> Result<Value> {
    if text_str.is_empty() {
        Ok(json!([""]))
    } else if delimiter_str.is_empty() {
        let chars: Vec<Value> = text_str
            .chars()
            .map(|c| Value::String(c.to_string()))
            .collect();
        Ok(Value::Array(chars))
    } else {
        let parts: Vec<Value> = text_str
            .split(delimiter_str)
            .map(|s| Value::String(s.to_string()))
            .collect();
        Ok(Value::Array(parts))
    }
}

// =============================================================================
// Arena-mode string operators
// =============================================================================
//
// Pre-evaluate args via `evaluate_arena_node` (so var lookups borrow), then
// build the result. For string-producing ops, the result is allocated as
// `&'a str` in the arena via `arena.alloc_str` — no heap `String`.

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value_cow, to_string_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_cat_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    // Build the concatenated string using a bumpalo String to avoid heap alloc.
    let mut buf = bumpalo::collections::String::new_in(arena);
    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, context, arena, root)?;
        match av {
            // For arrays, concat each item's string form.
            ArenaValue::Array(items) => {
                for it in *items {
                    buf.push_str(to_string_arena(it, arena));
                }
            }
            ArenaValue::InputRef(Value::Array(arr)) => {
                for it in arr {
                    let part = match it {
                        Value::String(s) => s.as_str(),
                        Value::Null => "",
                        _ => arena.alloc_str(&it.to_string()),
                    };
                    buf.push_str(part);
                }
            }
            _ => buf.push_str(to_string_arena(av, arena)),
        }
    }
    Ok(arena.alloc(ArenaValue::String(buf.into_bump_str())))
}

#[inline]
pub(crate) fn evaluate_substr_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::String("")));
    }
    // Pre-evaluate args into Cow<Value> and bridge to value-mode for the
    // index math. The result string lands in the arena.
    let s_av = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let s_cow = arena_to_value_cow(s_av);
    let s_owned = serde_json::Value::String(s_cow.as_str().map(|x| x.to_string()).unwrap_or_else(|| s_cow.to_string()));
    // Easier: just bridge via existing op.
    drop(s_owned);
    let _ = s_cow;
    let v = evaluate_substr(args, context, engine)?;
    Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)))
}

#[inline]
pub(crate) fn evaluate_in_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    // Pre-eval args via arena, but use existing value-mode logic for the
    // contains check (arrays/strings/objects all need different handling).
    if args.len() != 2 {
        let v = evaluate_in(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let _ = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let _ = engine.evaluate_arena_node(&args[1], actx, context, arena, root)?;
    let v = evaluate_in(args, context, engine)?;
    Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_starts_with_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        let v = evaluate_starts_with(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let s = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let p = engine.evaluate_arena_node(&args[1], actx, context, arena, root)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.starts_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_ends_with_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        let v = evaluate_ends_with(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let s = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let p = engine.evaluate_arena_node(&args[1], actx, context, arena, root)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.ends_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_upper_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        let v = evaluate_upper(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&s.to_uppercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_lower_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        let v = evaluate_lower(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&s.to_lowercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_trim_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        let v = evaluate_trim(args, context, engine)?;
        return Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(s.trim()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    // Pre-eval args; bridge for the array result construction.
    for arg in args {
        let _ = engine.evaluate_arena_node(arg, actx, context, arena, root)?;
    }
    let v = evaluate_split(args, context, engine)?;
    Ok(arena.alloc(crate::arena::value_to_arena(&v, arena)))
}

/// Arena variant of split-with-regex (compiled regex form).
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split_with_regex_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    regex: &Regex,
    capture_names: &[Box<str>],
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let text_av = engine.evaluate_arena_node(&args[0], actx, context, arena, root)?;
    let text_str = match text_av {
        ArenaValue::String(s) => *s,
        ArenaValue::InputRef(Value::String(s)) => s.as_str(),
        _ => to_string_arena(text_av, arena),
    };

    if let Some(captures) = regex.captures(text_str) {
        let mut pairs: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
            bumpalo::collections::Vec::with_capacity_in(capture_names.len(), arena);
        for name in capture_names {
            if let Some(m) = captures.name(name) {
                let k: &'a str = arena.alloc_str(name);
                let v: &'a str = arena.alloc_str(m.as_str());
                pairs.push((k, ArenaValue::String(v)));
            }
        }
        Ok(arena.alloc(ArenaValue::Object(pairs.into_bump_slice())))
    } else {
        Ok(arena.alloc(ArenaValue::Object(&[])))
    }
}
