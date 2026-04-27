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

use crate::{CompiledNode, DataLogic, Result};

// =============================================================================
// Arena-mode type operator
// =============================================================================
//
// Evaluates the arg via arena dispatch and returns a `&'static str` from a
// small set of type names. The string is allocated once into the arena.

use crate::arena::{DataContextStack, DataValue};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_type_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(DataValue::String("null")));
    }
    let av = engine.evaluate_node(&args[0], actx, arena)?;

    // Datetime/duration object detection (e.g. {"datetime": "..."}).
    #[cfg(feature = "datetime")]
    {
        if let DataValue::Object(pairs) = av {
            if pairs.iter().any(|(k, _)| *k == "datetime") {
                return Ok(arena.alloc(DataValue::String("datetime")));
            }
            if pairs.iter().any(|(k, _)| *k == "timestamp") {
                return Ok(arena.alloc(DataValue::String("duration")));
            }
        }
    }

    let type_str: &'static str = match av {
        DataValue::Null => "null",
        DataValue::Bool(_) => "boolean",
        DataValue::Number(_) => "number",
        DataValue::String(s) => classify_string(s),
        DataValue::Array(_) => "array",
        DataValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        DataValue::Duration(_) => "duration",
    };
    Ok(arena.alloc(DataValue::String(type_str)))
}

/// Classify a string into "datetime" / "duration" / "string" using the
/// `type` operator's string heuristic.
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
