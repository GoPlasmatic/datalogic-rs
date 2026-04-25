//! Type inspection operator for runtime type checking.
//!
//! The `type` operator returns a string indicating the type of a value,
//! useful for conditional logic based on data types.
//!
//! # Return Values
//!
//! | Value | Returns |
//! |-------|---------|
//! | `null` | `"null"` |
//! | `true`/`false` | `"boolean"` |
//! | `123`, `1.5` | `"number"` |
//! | `"hello"` | `"string"` |
//! | `[1, 2, 3]` | `"array"` |
//! | `{"key": "val"}` | `"object"` |
//! | ISO datetime string | `"datetime"` |
//! | Duration string | `"duration"` |
//!
//! # Special Type Detection
//!
//! The operator performs heuristic detection for datetime and duration strings:
//! - Datetime: Contains `T`, `:`, and either `Z` or `+` (ISO 8601 format)
//! - Duration: Contains time unit letters (`d`, `h`, `m`, `s`) with digits
//!
//! # Examples
//!
//! ```json
//! {"type": 42}                          // Returns: "number"
//! {"type": "hello"}                     // Returns: "string"
//! {"type": [1, 2, 3]}                   // Returns: "array"
//! {"type": "2024-01-15T10:30:00Z"}      // Returns: "datetime"
//! {"type": "2h30m"}                     // Returns: "duration"
//! ```

use serde_json::Value;

#[cfg(feature = "datetime")]
use crate::datetime::{is_datetime_object, is_duration_object};
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Type operator function - returns the type of a value as a string
#[inline]
pub fn evaluate_type(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
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
        engine.evaluate_node(&args[0], context)?
    } else {
        // Multiple arguments or no arguments - reconstruct the array
        let mut arr = Vec::new();
        for arg in args {
            arr.push(engine.evaluate_node(arg, context)?);
        }
        Value::Array(arr)
    };

    // Check for datetime/duration objects first
    #[cfg(feature = "datetime")]
    {
        if is_datetime_object(&value) {
            return Ok(Value::String("datetime".to_string()));
        }
        if is_duration_object(&value) {
            return Ok(Value::String("duration".to_string()));
        }
    }

    let type_str = match &value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(s) => {
            #[cfg(feature = "datetime")]
            {
                // Check for special datetime/duration formats in strings
                // Simple heuristic: if it looks like an ISO datetime or duration
                if s.contains('T') && s.contains(':') && (s.contains('Z') || s.contains('+')) {
                    "datetime"
                } else if s.chars().any(|c| matches!(c, 'd' | 'h' | 'm' | 's'))
                    && s.chars().any(|c| c.is_ascii_digit())
                    && !s.contains(' ')
                {
                    "duration"
                } else {
                    "string"
                }
            }
            #[cfg(not(feature = "datetime"))]
            {
                let _ = s;
                "string"
            }
        }
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };

    Ok(Value::String(type_str.to_string()))
}

// =============================================================================
// Arena-mode type operator
// =============================================================================
//
// Evaluates the arg via arena dispatch and returns a `&'static str` from a
// small set of type names. The string is allocated once into the arena.

use crate::arena::{ArenaContextStack, ArenaValue};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_type_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::String("null")));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, context, arena)?;

    // Datetime/duration object detection (e.g. {"datetime": "..."}).
    #[cfg(feature = "datetime")]
    {
        if let ArenaValue::InputRef(v) = av {
            if is_datetime_object(v) {
                return Ok(arena.alloc(ArenaValue::String("datetime")));
            }
            if is_duration_object(v) {
                return Ok(arena.alloc(ArenaValue::String("duration")));
            }
        }
    }

    let type_str: &'static str = match av {
        ArenaValue::Null => "null",
        ArenaValue::Bool(_) => "boolean",
        ArenaValue::Number(_) => "number",
        ArenaValue::String(s) => classify_string(s),
        ArenaValue::Array(_) => "array",
        ArenaValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(_) => "duration",
        ArenaValue::InputRef(v) => match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(s) => classify_string(s),
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        },
    };
    Ok(arena.alloc(ArenaValue::String(type_str)))
}

/// Classify a string into "datetime" / "duration" / "string" using the same
/// heuristic as value-mode `evaluate_type`.
#[inline]
fn classify_string(s: &str) -> &'static str {
    #[cfg(feature = "datetime")]
    {
        if s.contains('T') && s.contains(':') && (s.contains('Z') || s.contains('+')) {
            return "datetime";
        }
        if s.chars().any(|c| matches!(c, 'd' | 'h' | 'm' | 's'))
            && s.chars().any(|c| c.is_ascii_digit())
            && !s.contains(' ')
        {
            return "duration";
        }
        "string"
    }
    #[cfg(not(feature = "datetime"))]
    {
        let _ = s;
        "string"
    }
}
