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

use crate::arena::{ArenaContextStack, ArenaValue};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_type_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::String("null")));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;

    // Datetime/duration object detection (e.g. {"datetime": "..."}).
    #[cfg(feature = "datetime")]
    {
        if let ArenaValue::Object(pairs) = av {
            if pairs.iter().any(|(k, _)| *k == "datetime") {
                return Ok(arena.alloc(ArenaValue::String("datetime")));
            }
            if pairs.iter().any(|(k, _)| *k == "timestamp") {
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
    };
    Ok(arena.alloc(ArenaValue::String(type_str)))
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
